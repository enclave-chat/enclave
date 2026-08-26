use std::{
    collections::VecDeque,
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::State;

use crate::commands::config::ConfigState;

// ============================================================
// AUDIO CONSTANTS
// ============================================================

/// Network audio format.
///
/// Everything sent over UDP is:
///     44100 Hz
///     mono
///     signed i16
///     big endian
///
/// 20 ms @ 44100 Hz = 882 samples.
/// 882 * 2 = 1764 bytes.
const NETWORK_SAMPLE_RATE: u32 = 44_100;
const PACKET_DURATION_MS: usize = 20;
const NETWORK_PACKET_SAMPLES: usize = NETWORK_SAMPLE_RATE as usize * PACKET_DURATION_MS / 1000;

/// Amount of audio that must be queued before playback begins.
const PREBUFFER_MS: usize = 40;

/// If the playback queue gets below this amount, we allow it to
/// continue normally but the callback will output silence if it
/// actually runs dry.
const LOW_WATERMARK_MS: usize = 20;

/// Maximum amount of queued audio.
///
/// If this is exceeded, OLD audio is discarded. This is intentional:
/// for voice chat, dropping old audio is much better than accumulating
/// hundreds of milliseconds/seconds of latency.
const MAX_BUFFER_MS: usize = 100;

/// Maximum UDP packet size we expect.
const MAX_UDP_PACKET_SIZE: usize = 4096;

// ============================================================
// STATE
// ============================================================

pub struct VoiceState {
    pub session: Mutex<Option<VoiceSession>>,
}

pub struct VoiceSession {
    pub input_stream: cpal::Stream,

    // Wrapped so the watcher thread can replace the stream after
    // the output device changes configuration.
    pub output_stream: Arc<Mutex<cpal::Stream>>,

    pub socket: Arc<UdpSocket>,

    pub pin: u64,

    pub shutdown: Arc<AtomicBool>,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

// ============================================================
// DEVICE ENUMERATION
// ============================================================

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();

    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate input devices: {e}"))?;

    Ok(devices
        .filter_map(|device| Some(device.description().ok()?.name().to_string()))
        .collect())
}

#[tauri::command]
pub fn list_output_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();

    let devices = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;

    Ok(devices
        .filter_map(|device| Some(device.description().ok()?.name().to_string()))
        .collect())
}

// ============================================================
// DISCONNECT
// ============================================================

#[tauri::command]
pub fn disconnect_from_vc(voice_state: State<VoiceState>) -> Result<(), String> {
    if let Some(session) = voice_state.session.lock().unwrap().take() {
        session.shutdown.store(true, Ordering::SeqCst);

        // Streams are dropped here.
        //
        // The UDP receiver has a 100ms read timeout, so it will notice
        // shutdown shortly instead of remaining blocked forever.
    }

    Ok(())
}

// ============================================================
// CONNECT
// ============================================================

#[tauri::command]
pub fn connect_to_vc(
    hostname: String,
    pin: u64,
    config_state: State<ConfigState>,
    voice_state: State<VoiceState>,
) -> Result<(), String> {
    disconnect_from_vc(voice_state.clone())?;

    let config = config_state.0.lock().unwrap().clone();

    // ========================================================
    // UDP SOCKET
    // ========================================================

    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind UDP socket: {e}"))?;

    socket
        .connect(&hostname)
        .map_err(|e| format!("Failed to connect UDP socket: {e}"))?;

    // IMPORTANT:
    //
    // Without a timeout, recv() can remain blocked forever and the
    // receiver thread can survive after disconnect.
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Failed to configure UDP timeout: {e}"))?;

    let socket = Arc::new(socket);

    // PIN is always the first packet.
    socket
        .send(&pin.to_be_bytes())
        .map_err(|e| format!("Failed to send pincode: {e}"))?;

    // ========================================================
    // AUDIO DEVICES
    // ========================================================

    let host = cpal::default_host();

    let input_device = match &config.input_device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|device| {
                device
                    .description()
                    .ok()
                    .map(|description| description.name() == name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| "Input device not found".to_string())?,

        None => host
            .default_input_device()
            .ok_or_else(|| "No default input device".to_string())?,
    };

    let output_device = match &config.output_device_name {
        Some(name) => host
            .output_devices()
            .map_err(|e| e.to_string())?
            .find(|device| {
                device
                    .description()
                    .ok()
                    .map(|description| description.name() == name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| "Output device not found".to_string())?,

        None => host
            .default_output_device()
            .ok_or_else(|| "No default output device".to_string())?,
    };

    // ========================================================
    // SHARED PLAYBACK STATE
    // ========================================================

    //
    // IMPORTANT:
    //
    // This queue contains samples already converted to the
    // CURRENT output device's channel layout.
    //
    // Therefore:
    //
    // stereo device:
    //     L R L R L R ...
    //
    // mono device:
    //     M M M M ...
    //
    let playback_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));

    let needs_output_rebuild = Arc::new(AtomicBool::new(false));

    let shutdown = Arc::new(AtomicBool::new(false));

    let playback_started = Arc::new(AtomicBool::new(false));

    // ========================================================
    // INPUT: MICROPHONE -> UDP
    // ========================================================

    let input_socket = socket.clone();

    let mut input_config: cpal::StreamConfig = input_device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {e}"))?
        .into();

    // We explicitly capture mono.
    input_config.channels = 1;

    let input_sample_rate = input_config.sample_rate;

    eprintln!(
        "[vc] input: {} Hz -> {} Hz",
        input_sample_rate, NETWORK_SAMPLE_RATE
    );

    let mut input_resampler = resampler::ResamplerFft::new(
        1,
        input_sample_rate
            .try_into()
            .map_err(|e| format!("Invalid input sample rate: {e:?}"))?,
        resampler::SampleRate::Hz44100,
    );

    let mut input_buffer = Vec::<f32>::new();

    // Resampled samples waiting to form a network packet.
    let mut packet_buffer = Vec::<f32>::with_capacity(NETWORK_PACKET_SAMPLES * 2);

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                input_buffer.extend_from_slice(data);

                let frame_size = input_resampler.chunk_size_input();

                while input_buffer.len() >= frame_size {
                    let input: Vec<f32> = input_buffer.drain(..frame_size).collect();

                    let output_size = input_resampler.chunk_size_output();

                    let mut output = vec![0.0f32; output_size];

                    if let Err(e) = input_resampler.resample(&input, &mut output) {
                        eprintln!("[vc] failed to resample input: {e}");
                        continue;
                    }

                    packet_buffer.extend(output.into_iter().map(|sample| sample.clamp(-1.0, 1.0)));

                    // ------------------------------------------------
                    // Form exact 20ms network packets.
                    // ------------------------------------------------

                    while packet_buffer.len() >= NETWORK_PACKET_SAMPLES {
                        let packet_samples: Vec<f32> =
                            packet_buffer.drain(..NETWORK_PACKET_SAMPLES).collect();

                        let mut packet = Vec::with_capacity(NETWORK_PACKET_SAMPLES * 2);

                        for sample in packet_samples {
                            let pcm = (sample * i16::MAX as f32) as i16;

                            packet.extend_from_slice(&pcm.to_be_bytes());
                        }

                        match input_socket.send(&packet) {
                            Ok(_) => {}

                            Err(e) => {
                                eprintln!("[vc] UDP send failed: {e}");
                            }
                        }
                    }
                }
            },
            |err| {
                eprintln!("[vc] input stream error: {err}");
            },
            None,
        )
        .map_err(|e| e.to_string())?;

    // ========================================================
    // OUTPUT CONFIG
    // ========================================================

    let output_config = build_output_config(&output_device)?;

    eprintln!(
        "[vc] output: {} Hz / {} channels",
        output_config.sample_rate, output_config.channels
    );

    let output_config_cell = Arc::new(Mutex::new(output_config.clone()));

    // ========================================================
    // OUTPUT STREAM
    // ========================================================

    let initial_output_stream = build_output_stream(
        &output_device,
        &output_config,
        playback_buffer.clone(),
        needs_output_rebuild.clone(),
        playback_started.clone(),
    )?;

    initial_output_stream
        .play()
        .map_err(|e| format!("Failed to start output stream: {e}"))?;

    let output_stream = Arc::new(Mutex::new(initial_output_stream));

    // ========================================================
    // OUTPUT DEVICE WATCHER
    // ========================================================

    {
        let output_device = output_device.clone();

        let needs_output_rebuild = needs_output_rebuild.clone();

        let output_stream = output_stream.clone();

        let output_config_cell = output_config_cell.clone();

        let shutdown = shutdown.clone();

        let playback_buffer = playback_buffer.clone();

        let playback_started = playback_started.clone();

        thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));

                if shutdown.load(Ordering::SeqCst) {
                    return;
                }

                if !needs_output_rebuild.swap(false, Ordering::SeqCst) {
                    continue;
                }

                eprintln!("[vc] rebuilding output stream...");

                let result = build_output_config(&output_device).and_then(|new_config| {
                    build_output_stream(
                        &output_device,
                        &new_config,
                        playback_buffer.clone(),
                        needs_output_rebuild.clone(),
                        playback_started.clone(),
                    )
                    .map(|stream| (new_config, stream))
                });

                match result {
                    Ok((new_config, new_stream)) => {
                        if let Err(e) = new_stream.play() {
                            eprintln!(
                                "[vc] failed to start rebuilt \
                                 output stream: {e}"
                            );

                            needs_output_rebuild.store(true, Ordering::SeqCst);

                            continue;
                        }

                        // Clear stale audio because it was generated
                        // for the old output timing/channel layout.
                        playback_buffer.lock().unwrap().clear();

                        playback_started.store(false, Ordering::SeqCst);

                        *output_config_cell.lock().unwrap() = new_config.clone();

                        *output_stream.lock().unwrap() = new_stream;

                        eprintln!(
                            "[vc] output rebuilt: {} Hz / {} channels",
                            new_config.sample_rate, new_config.channels
                        );
                    }

                    Err(e) => {
                        eprintln!("[vc] failed to rebuild output: {e}");

                        needs_output_rebuild.store(true, Ordering::SeqCst);
                    }
                }
            }
        });
    }

    // ========================================================
    // UDP RECEIVE -> RESAMPLE -> PLAYBACK QUEUE
    // ========================================================

    {
        let recv_socket = socket.clone();

        let output_config_cell = output_config_cell.clone();

        let playback_buffer = playback_buffer.clone();

        let playback_started = playback_started.clone();

        let shutdown = shutdown.clone();

        thread::spawn(move || {
            let mut buf = [0u8; MAX_UDP_PACKET_SIZE];

            let initial_config = output_config_cell.lock().unwrap().clone();

            let mut last_sample_rate = initial_config.sample_rate;

            let mut output_resampler = match create_output_resampler(initial_config.sample_rate) {
                Ok(resampler) => resampler,

                Err(e) => {
                    eprintln!(
                        "[vc] failed to create output \
                             resampler: {e}"
                    );

                    return;
                }
            };

            // Audio waiting to be fed into the resampler.
            let mut resample_input = Vec::<f32>::new();

            loop {
                if shutdown.load(Ordering::SeqCst) {
                    return;
                }

                // ----------------------------------------------------
                // Receive UDP packet.
                // ----------------------------------------------------

                let len = match recv_socket.recv(&mut buf) {
                    Ok(len) => len,

                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        continue;
                    }

                    Err(e) => {
                        if !shutdown.load(Ordering::SeqCst) {
                            eprintln!("[vc] UDP receive failed: {e}");
                        }

                        continue;
                    }
                };

                // ----------------------------------------------------
                // Ignore malformed packets.
                //
                // Every audio packet must contain complete i16
                // samples.
                // ----------------------------------------------------

                if len < 2 {
                    continue;
                }

                let usable_len = len - (len % 2);

                // ----------------------------------------------------
                // BIG-ENDIAN i16 -> f32
                // ----------------------------------------------------

                for chunk in buf[..usable_len].chunks_exact(2) {
                    let pcm = i16::from_be_bytes([chunk[0], chunk[1]]);

                    resample_input.push(pcm as f32 / i16::MAX as f32);
                }

                // ----------------------------------------------------
                // Check current output device configuration.
                // ----------------------------------------------------

                let current_config = output_config_cell.lock().unwrap().clone();

                // ----------------------------------------------------
                // Output sample rate changed.
                //
                // Recreate the resampler and discard samples from the
                // old timing domain.
                // ----------------------------------------------------

                if current_config.sample_rate != last_sample_rate {
                    eprintln!(
                        "[vc] output rate changed: {} -> {}",
                        last_sample_rate, current_config.sample_rate
                    );

                    match create_output_resampler(current_config.sample_rate) {
                        Ok(new_resampler) => {
                            output_resampler = new_resampler;

                            last_sample_rate = current_config.sample_rate;

                            resample_input.clear();

                            playback_buffer.lock().unwrap().clear();

                            playback_started.store(false, Ordering::SeqCst);
                        }

                        Err(e) => {
                            eprintln!(
                                "[vc] unsupported output \
                                 sample rate {}: {e}",
                                current_config.sample_rate
                            );

                            resample_input.clear();

                            continue;
                        }
                    }
                }

                // ----------------------------------------------------
                // Resample incoming 44.1kHz mono audio into the
                // output device's sample rate.
                // ----------------------------------------------------

                let frame_size = output_resampler.chunk_size_input();

                while resample_input.len() >= frame_size {
                    let input: Vec<f32> = resample_input.drain(..frame_size).collect();

                    let output_size = output_resampler.chunk_size_output();

                    let mut resampled = vec![0.0f32; output_size];

                    if let Err(e) = output_resampler.resample(&input, &mut resampled) {
                        eprintln!(
                            "[vc] failed to resample \
                             output: {e}"
                        );

                        continue;
                    }

                    // ------------------------------------------------
                    // Mono -> device channels.
                    // ------------------------------------------------

                    let output = mono_to_output_channels(&resampled, current_config.channels);

                    // ------------------------------------------------
                    // Push into bounded playback queue.
                    // ------------------------------------------------

                    let mut queue = playback_buffer.lock().unwrap();

                    queue.extend(output);

                    let channels = current_config.channels.max(1) as usize;

                    let max_frames = current_config.sample_rate as usize * MAX_BUFFER_MS / 1000;

                    let max_samples = max_frames * channels;

                    // ------------------------------------------------
                    // If we have accumulated too much audio, discard
                    // OLD audio.
                    //
                    // This is critical for voice chat latency.
                    // ------------------------------------------------

                    while queue.len() > max_samples {
                        queue.pop_front();
                    }

                    // ------------------------------------------------
                    // Start playback only after we have a small
                    // amount of audio buffered.
                    // ------------------------------------------------

                    if !playback_started.load(Ordering::SeqCst) {
                        let prebuffer_frames =
                            current_config.sample_rate as usize * PREBUFFER_MS / 1000;

                        let prebuffer_samples = prebuffer_frames * channels;

                        if queue.len() >= prebuffer_samples {
                            playback_started.store(true, Ordering::SeqCst);

                            eprintln!(
                                "[vc] playback started \
                                 with ~{}ms buffered",
                                PREBUFFER_MS
                            );
                        }
                    }
                }
            }
        });
    }

    // ========================================================
    // START INPUT
    // ========================================================

    input_stream
        .play()
        .map_err(|e| format!("Failed to start input stream: {e}"))?;

    // ========================================================
    // STORE SESSION
    // ========================================================

    *voice_state.session.lock().unwrap() = Some(VoiceSession {
        input_stream,
        output_stream,
        socket,
        pin,
        shutdown,
    });

    Ok(())
}

// ============================================================
// OUTPUT CONFIG
// ============================================================

fn build_output_config(output_device: &cpal::Device) -> Result<cpal::StreamConfig, String> {
    output_device
        .default_output_config()
        .map_err(|e| e.to_string())
        .map(Into::into)
}

// ============================================================
// OUTPUT RESAMPLER
// ============================================================

fn create_output_resampler(
    output_sample_rate: cpal::SampleRate,
) -> Result<resampler::ResamplerFft, String> {
    let output_rate = output_sample_rate
        .try_into()
        .map_err(|e| format!("Invalid output sample rate: {e:?}"))?;

    Ok(resampler::ResamplerFft::new(
        1,
        resampler::SampleRate::Hz44100,
        output_rate,
    ))
}

// ============================================================
// BUILD OUTPUT STREAM
// ============================================================

fn build_output_stream(
    output_device: &cpal::Device,
    output_config: &cpal::StreamConfig,
    playback_buffer: Arc<Mutex<VecDeque<f32>>>,
    needs_rebuild: Arc<AtomicBool>,
    playback_started: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    output_device
        .build_output_stream(
            output_config.clone(),
            // ====================================================
            // CPAL OUTPUT CALLBACK
            // ====================================================
            move |data: &mut [f32], _| {
                let started = playback_started.load(Ordering::Acquire);

                if !started {
                    // Do NOT consume audio before the prebuffer is
                    // ready. Just output silence.
                    data.fill(0.0);
                    return;
                }

                let mut queue = playback_buffer.lock().unwrap();

                // VecDeque::pop_front() is O(1).
                //
                // This is massively better than:
                //
                //     Vec::remove(0)
                //
                // which shifts the entire vector every sample.

                for sample in data.iter_mut() {
                    *sample = queue.pop_front().unwrap_or(0.0);
                }
            },
            // ====================================================
            // OUTPUT ERROR CALLBACK
            // ====================================================
            {
                let needs_rebuild = needs_rebuild.clone();

                move |err| {
                    eprintln!("[vc] output stream error: {err}");

                    let message = err.to_string();

                    if message.contains("sample rate changed")
                        || message.contains("DeviceNotAvailable")
                        || message.contains("device not available")
                    {
                        needs_rebuild.store(true, Ordering::SeqCst);
                    }
                }
            },
            None,
        )
        .map_err(|e| e.to_string())
}

// ============================================================
// MONO -> OUTPUT CHANNELS
// ============================================================

fn mono_to_output_channels(mono: &[f32], channels: u16) -> Vec<f32> {
    let channels = channels.max(1) as usize;

    if channels == 1 {
        return mono.to_vec();
    }

    let mut output = Vec::with_capacity(mono.len() * channels);

    for &sample in mono {
        for _ in 0..channels {
            output.push(sample);
        }
    }

    output
}

// ============================================================
// OPTIONAL UTILITY
// ============================================================

fn stereo_to_mono(stereo_data: &[f32]) -> Vec<f32> {
    let mut mono = Vec::with_capacity(stereo_data.len() / 2);

    for chunk in stereo_data.chunks_exact(2) {
        mono.push((chunk[0] + chunk[1]) * 0.5);
    }

    mono
}
