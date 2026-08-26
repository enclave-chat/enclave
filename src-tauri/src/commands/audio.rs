use std::{
    collections::{BTreeMap, VecDeque},
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::State;

use crate::commands::config::ConfigState;

// ============================================================================
// AUDIO FORMAT
// ============================================================================

const NETWORK_SAMPLE_RATE: u32 = 44_100;

// 20ms packets.
//
// 44100 * 0.020 = 882 samples
const PACKET_DURATION_MS: usize = 20;
const NETWORK_PACKET_SAMPLES: usize = NETWORK_SAMPLE_RATE as usize * PACKET_DURATION_MS / 1000;

// PCM is i16 => 2 bytes/sample.
const NETWORK_PACKET_BYTES: usize = NETWORK_PACKET_SAMPLES * 2;

// Sequence number is a u32.
const AUDIO_HEADER_BYTES: usize = 4;

// Maximum UDP packet we accept.
const MAX_UDP_PACKET_SIZE: usize = 4096;

// ============================================================================
// JITTER BUFFER
// ============================================================================
//
// We intentionally keep this relatively small.
//
// Increasing these values reduces underruns but increases latency.
//
// Current target:
//
//     startup:       40ms
//     normal target: 40ms
//     maximum:      100ms
//
// This is appropriate for low-latency voice.
//

const JITTER_TARGET_MS: usize = 40;
const JITTER_MAX_MS: usize = 100;

// If a packet is missing, wait this long before considering it lost.
//
// Since packets are 20ms, 30ms gives us enough room for modest
// out-of-order delivery without making latency enormous.
const PACKET_LOSS_WAIT_MS: u64 = 30;

// ============================================================================
// VOICE STATE
// ============================================================================

pub struct VoiceState {
    pub session: Mutex<Option<VoiceSession>>,
}

pub struct VoiceSession {
    pub input_stream: cpal::Stream,

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

// ============================================================================
// DEVICE LISTING
// ============================================================================

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

// ============================================================================
// DISCONNECT
// ============================================================================

#[tauri::command]
pub fn disconnect_from_vc(voice_state: State<VoiceState>) -> Result<(), String> {
    if let Some(session) = voice_state.session.lock().unwrap().take() {
        session.shutdown.store(true, Ordering::SeqCst);
    }

    Ok(())
}

// ============================================================================
// CONNECT
// ============================================================================

#[tauri::command]
pub fn connect_to_vc(
    hostname: String,
    pin: u64,
    config_state: State<ConfigState>,
    voice_state: State<VoiceState>,
) -> Result<(), String> {
    disconnect_from_vc(voice_state.clone())?;

    let config = config_state.0.lock().unwrap().clone();

    // ========================================================================
    // UDP
    // ========================================================================

    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind UDP socket: {e}"))?;

    socket
        .connect(&hostname)
        .map_err(|e| format!("Failed to connect UDP socket: {e}"))?;

    // Required so disconnect can terminate the receiver thread.
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| format!("Failed to set UDP read timeout: {e}"))?;

    let socket = Arc::new(socket);

    // ========================================================================
    // AUTHENTICATION
    // ========================================================================

    socket
        .send(&pin.to_be_bytes())
        .map_err(|e| format!("Failed to send pincode: {e}"))?;

    // ========================================================================
    // DEVICES
    // ========================================================================

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

    // ========================================================================
    // SHUTDOWN
    // ========================================================================

    let shutdown = Arc::new(AtomicBool::new(false));

    // ========================================================================
    // PLAYBACK BUFFER
    // ========================================================================
    //
    // This contains samples already converted to the output device's
    // channel layout.
    //

    let playback_buffer = Arc::new(Mutex::new(VecDeque::<f32>::new()));

    let playback_started = Arc::new(AtomicBool::new(false));

    // ========================================================================
    // OUTPUT CONFIG
    // ========================================================================

    let output_config = build_output_config(&output_device)?;

    eprintln!(
        "[vc] output: {} Hz / {} channels",
        output_config.sample_rate, output_config.channels
    );

    let output_config_cell = Arc::new(Mutex::new(output_config.clone()));

    let needs_output_rebuild = Arc::new(AtomicBool::new(false));

    // ========================================================================
    // INPUT
    // ========================================================================

    let input_socket = socket.clone();

    let mut input_config: cpal::StreamConfig = input_device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {e}"))?
        .into();

    // Network audio is mono.
    input_config.channels = 1;

    let input_sample_rate = input_config.sample_rate;

    eprintln!(
        "[vc] input: {} Hz -> {} Hz",
        input_sample_rate, NETWORK_SAMPLE_RATE
    );

    let mut input_resampler = resampler::ResamplerFft::new(
        1,
        input_sample_rate.try_into().map_err(|e| {
            format!(
                "Invalid input sample rate: \
                         {e:?}"
            )
        })?,
        resampler::SampleRate::Hz44100,
    );

    let mut input_buffer = Vec::<f32>::new();

    // Resampled samples waiting to form 20ms packets.
    let mut packet_samples = Vec::<f32>::with_capacity(NETWORK_PACKET_SAMPLES * 2);

    // Sequence number for every audio packet.
    let mut sequence: u32 = 0;

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
                        eprintln!(
                            "[vc] input resample \
                             failed: {e}"
                        );

                        continue;
                    }

                    packet_samples.extend(output.into_iter().map(|sample| sample.clamp(-1.0, 1.0)));

                    // ========================================================
                    // CREATE EXACT 20ms PACKETS
                    // ========================================================

                    while packet_samples.len() >= NETWORK_PACKET_SAMPLES {
                        let samples: Vec<f32> =
                            packet_samples.drain(..NETWORK_PACKET_SAMPLES).collect();

                        // Header:
                        //
                        // [u32 sequence]
                        //
                        // Then:
                        //
                        // [i16 PCM...]
                        let mut packet =
                            Vec::with_capacity(AUDIO_HEADER_BYTES + NETWORK_PACKET_BYTES);

                        packet.extend_from_slice(&sequence.to_be_bytes());

                        for sample in samples {
                            let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;

                            packet.extend_from_slice(&pcm.to_be_bytes());
                        }

                        if let Err(e) = input_socket.send(&packet) {
                            eprintln!(
                                "[vc] UDP send failed: \
                                 {e}"
                            );
                        }

                        sequence = sequence.wrapping_add(1);
                    }
                }
            },
            |err| {
                eprintln!("[vc] input stream error: {err}");
            },
            None,
        )
        .map_err(|e| e.to_string())?;

    // ========================================================================
    // OUTPUT STREAM
    // ========================================================================

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

    // ========================================================================
    // OUTPUT DEVICE WATCHER
    // ========================================================================

    {
        let output_device = output_device.clone();

        let needs_output_rebuild = needs_output_rebuild.clone();

        let output_stream = output_stream.clone();

        let output_config_cell = output_config_cell.clone();

        let playback_buffer = playback_buffer.clone();

        let playback_started = playback_started.clone();

        let shutdown = shutdown.clone();

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
                                "[vc] failed to start \
                                 rebuilt output: {e}"
                            );

                            needs_output_rebuild.store(true, Ordering::SeqCst);

                            continue;
                        }

                        // The old output format is no longer valid.
                        //
                        // Throw away queued samples rather than playing
                        // them using the new device timing/layout.
                        playback_buffer.lock().unwrap().clear();

                        playback_started.store(false, Ordering::SeqCst);

                        *output_config_cell.lock().unwrap() = new_config.clone();

                        *output_stream.lock().unwrap() = new_stream;

                        eprintln!(
                            "[vc] output rebuilt: \
                             {} Hz / {} channels",
                            new_config.sample_rate, new_config.channels
                        );
                    }

                    Err(e) => {
                        eprintln!(
                            "[vc] failed to rebuild \
                             output: {e}"
                        );

                        needs_output_rebuild.store(true, Ordering::SeqCst);
                    }
                }
            }
        });
    }

    // ========================================================================
    // UDP RECEIVE + JITTER BUFFER
    // ========================================================================

    {
        let recv_socket = socket.clone();

        let output_config_cell = output_config_cell.clone();

        let playback_buffer = playback_buffer.clone();

        let playback_started = playback_started.clone();

        let shutdown = shutdown.clone();

        thread::spawn(move || {
            // ================================================================
            // PACKET REORDER BUFFER
            // ================================================================

            let mut packets: BTreeMap<u32, Vec<f32>> = BTreeMap::new();

            let mut expected_sequence: Option<u32> = None;

            // When we first notice a missing packet, remember when.
            let mut missing_since: Option<Instant> = None;

            // ================================================================
            // RESAMPLER
            // ================================================================

            let initial_config = output_config_cell.lock().unwrap().clone();

            let mut last_sample_rate = initial_config.sample_rate;

            let mut output_resampler = match create_output_resampler(initial_config.sample_rate) {
                Ok(resampler) => resampler,

                Err(e) => {
                    eprintln!(
                        "[vc] failed to create \
                             output resampler: {e}"
                    );

                    return;
                }
            };

            let mut resample_input = Vec::<f32>::new();

            // ================================================================
            // UDP BUFFER
            // ================================================================

            let mut udp_buffer = [0u8; MAX_UDP_PACKET_SIZE];

            // ================================================================
            // MAIN LOOP
            // ================================================================

            while !shutdown.load(Ordering::SeqCst) {
                // ============================================================
                // RECEIVE PACKET
                // ============================================================

                match recv_socket.recv(&mut udp_buffer) {
                    Ok(len) => {
                        if len <= AUDIO_HEADER_BYTES {
                            continue;
                        }

                        // ----------------------------------------------------
                        // Read sequence number.
                        // ----------------------------------------------------

                        let sequence = u32::from_be_bytes([
                            udp_buffer[0],
                            udp_buffer[1],
                            udp_buffer[2],
                            udp_buffer[3],
                        ]);

                        let pcm_bytes = &udp_buffer[AUDIO_HEADER_BYTES..len];

                        // Must contain complete i16 samples.
                        let pcm_len = pcm_bytes.len() & !1;

                        if pcm_len == 0 {
                            continue;
                        }

                        let mut samples = Vec::with_capacity(pcm_len / 2);

                        for chunk in pcm_bytes[..pcm_len].chunks_exact(2) {
                            let pcm = i16::from_be_bytes([chunk[0], chunk[1]]);

                            samples.push(pcm as f32 / i16::MAX as f32);
                        }

                        // ----------------------------------------------------
                        // Ignore packets that are already too old.
                        // ----------------------------------------------------

                        if let Some(expected) = expected_sequence {
                            if sequence_before(sequence, expected) {
                                continue;
                            }
                        }

                        // ----------------------------------------------------
                        // Insert into jitter buffer.
                        //
                        // BTreeMap automatically keeps sequence numbers
                        // ordered.
                        // ----------------------------------------------------

                        packets.entry(sequence).or_insert(samples);

                        // ----------------------------------------------------
                        // Don't allow the packet jitter buffer itself to
                        // become a source of latency.
                        // ----------------------------------------------------

                        while packets.len() > 8 {
                            if let Some((&oldest, _)) = packets.iter().next() {
                                if let Some(expected) = expected_sequence {
                                    if sequence_before(oldest, expected) {
                                        packets.remove(&oldest);
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                        }
                    }

                    Err(e)
                        if e.kind() == std::io::ErrorKind::TimedOut
                            || e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        // Timeout is expected. It gives us a chance to
                        // process jitter-buffer timeouts and shutdown.
                    }

                    Err(e) => {
                        if !shutdown.load(Ordering::SeqCst) {
                            eprintln!(
                                "[vc] UDP receive failed: \
                                 {e}"
                            );
                        }

                        continue;
                    }
                }

                // ============================================================
                // INITIALIZE EXPECTED SEQUENCE
                // ============================================================

                if expected_sequence.is_none() {
                    if let Some((&first_sequence, _)) = packets.iter().next() {
                        expected_sequence = Some(first_sequence);

                        eprintln!(
                            "[vc] jitter buffer \
                             synchronized at packet {}",
                            first_sequence
                        );
                    }
                }

                // ============================================================
                // CURRENT OUTPUT CONFIG
                // ============================================================

                let current_config = output_config_cell.lock().unwrap().clone();

                // ============================================================
                // OUTPUT DEVICE SAMPLE RATE CHANGE
                // ============================================================

                if current_config.sample_rate != last_sample_rate {
                    eprintln!(
                        "[vc] output rate changed: \
                         {} -> {}",
                        last_sample_rate, current_config.sample_rate
                    );

                    match create_output_resampler(current_config.sample_rate) {
                        Ok(new_resampler) => {
                            output_resampler = new_resampler;

                            last_sample_rate = current_config.sample_rate;

                            packets.clear();

                            resample_input.clear();

                            playback_buffer.lock().unwrap().clear();

                            playback_started.store(false, Ordering::SeqCst);

                            expected_sequence = None;

                            missing_since = None;
                        }

                        Err(e) => {
                            eprintln!(
                                "[vc] failed to create \
                                 output resampler: {e}"
                            );

                            packets.clear();
                            resample_input.clear();

                            continue;
                        }
                    }
                }

                // ============================================================
                // MOVE READY PACKETS INTO RESAMPLER
                // ============================================================

                loop {
                    let Some(expected) = expected_sequence else {
                        break;
                    };

                    // --------------------------------------------------------
                    // Expected packet exists.
                    // --------------------------------------------------------

                    if let Some(samples) = packets.remove(&expected) {
                        resample_input.extend_from_slice(&samples);

                        expected_sequence = Some(expected.wrapping_add(1));

                        missing_since = None;

                        continue;
                    }

                    // --------------------------------------------------------
                    // Expected packet doesn't exist.
                    //
                    // If we don't have anything newer, there is nothing
                    // to do yet.
                    // --------------------------------------------------------

                    let has_newer_packet = packets.keys().any(|&seq| sequence_after(seq, expected));

                    if !has_newer_packet {
                        break;
                    }

                    // --------------------------------------------------------
                    // We have a later packet.
                    //
                    // Therefore the expected packet is either delayed or
                    // lost.
                    //
                    // Wait a short period before declaring it lost.
                    // --------------------------------------------------------

                    let now = Instant::now();

                    let since = missing_since.get_or_insert(now);

                    if since.elapsed() < Duration::from_millis(PACKET_LOSS_WAIT_MS) {
                        break;
                    }

                    // --------------------------------------------------------
                    // Packet is considered lost.
                    //
                    // We insert a zero packet here.
                    //
                    // Because the missing packet is exactly 20ms, this
                    // results in a controlled 20ms gap rather than the
                    // playback clock getting permanently stuck.
                    // --------------------------------------------------------

                    resample_input.extend(std::iter::repeat(0.0f32).take(NETWORK_PACKET_SAMPLES));

                    expected_sequence = Some(expected.wrapping_add(1));

                    missing_since = None;

                    eprintln!("[vc] lost UDP packet {}", expected);
                }

                // ============================================================
                // RESAMPLE
                // ============================================================

                let input_frame_size = output_resampler.chunk_size_input();

                while resample_input.len() >= input_frame_size {
                    let input: Vec<f32> = resample_input.drain(..input_frame_size).collect();

                    let output_frame_size = output_resampler.chunk_size_output();

                    let mut resampled = vec![0.0f32; output_frame_size];

                    if let Err(e) = output_resampler.resample(&input, &mut resampled) {
                        eprintln!(
                            "[vc] output resample \
                             failed: {e}"
                        );

                        continue;
                    }

                    // ========================================================
                    // MONO -> DEVICE CHANNELS
                    // ========================================================

                    let output = mono_to_output_channels(&resampled, current_config.channels);

                    // ========================================================
                    // PLAYBACK QUEUE
                    // ========================================================

                    let channels = current_config.channels.max(1) as usize;

                    let mut queue = playback_buffer.lock().unwrap();

                    queue.extend(output);

                    // --------------------------------------------------------
                    // Maximum playback queue.
                    //
                    // If this gets exceeded, THROW AWAY OLD AUDIO.
                    //
                    // This prevents latency from continuously growing.
                    // --------------------------------------------------------

                    let max_frames = current_config.sample_rate as usize * JITTER_MAX_MS / 1000;

                    let max_samples = max_frames * channels;

                    while queue.len() > max_samples {
                        queue.pop_front();
                    }

                    // --------------------------------------------------------
                    // Start playback once enough audio is available.
                    // --------------------------------------------------------

                    if !playback_started.load(Ordering::Acquire) {
                        let target_frames =
                            current_config.sample_rate as usize * JITTER_TARGET_MS / 1000;

                        let target_samples = target_frames * channels;

                        if queue.len() >= target_samples {
                            playback_started.store(true, Ordering::Release);

                            eprintln!(
                                "[vc] playback started \
                                 with ~{}ms buffered",
                                JITTER_TARGET_MS
                            );
                        }
                    }
                }

                // ============================================================
                // RECOVER FROM PLAYBACK UNDERRUN
                // ============================================================
                //
                // If CPAL consumes everything, it will output silence.
                //
                // Once enough audio has accumulated again, playback can
                // resume.
                //
                // We intentionally don't constantly toggle this state.
                // That was one of the sources of the previous flicker.
                // ============================================================

                if playback_started.load(Ordering::Acquire) {
                    let channels = current_config.channels.max(1) as usize;

                    let queue_len = playback_buffer.lock().unwrap().len();

                    let low_frames = current_config.sample_rate as usize * 10 / 1000;

                    let low_samples = low_frames * channels;

                    // We don't stop playback at 0ms.
                    //
                    // Only stop if the queue has actually become empty.
                    if queue_len == 0 {
                        playback_started.store(false, Ordering::Release);
                    }

                    let _ = low_samples;
                }
            }
        });
    }

    // ========================================================================
    // START INPUT
    // ========================================================================

    input_stream
        .play()
        .map_err(|e| format!("Failed to start input stream: {e}"))?;

    // ========================================================================
    // SAVE SESSION
    // ========================================================================

    *voice_state.session.lock().unwrap() = Some(VoiceSession {
        input_stream,
        output_stream,
        socket,
        pin,
        shutdown,
    });

    Ok(())
}

// ============================================================================
// OUTPUT CONFIG
// ============================================================================

fn build_output_config(output_device: &cpal::Device) -> Result<cpal::StreamConfig, String> {
    output_device
        .default_output_config()
        .map_err(|e| e.to_string())
        .map(Into::into)
}

// ============================================================================
// OUTPUT RESAMPLER
// ============================================================================

fn create_output_resampler(
    output_sample_rate: cpal::SampleRate,
) -> Result<resampler::ResamplerFft, String> {
    let rate = output_sample_rate.try_into().map_err(|e| {
        format!(
            "Invalid output sample rate: \
                     {e:?}"
        )
    })?;

    Ok(resampler::ResamplerFft::new(
        1,
        resampler::SampleRate::Hz44100,
        rate,
    ))
}

// ============================================================================
// OUTPUT STREAM
// ============================================================================

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
            // =================================================================
            // AUDIO CALLBACK
            // =================================================================
            move |data: &mut [f32], _| {
                // -------------------------------------------------------------
                // Don't consume the queue until the jitter buffer has enough
                // audio.
                // -------------------------------------------------------------

                if !playback_started.load(Ordering::Acquire) {
                    data.fill(0.0);
                    return;
                }

                let mut queue = playback_buffer.lock().unwrap();

                // -------------------------------------------------------------
                // CRITICAL:
                //
                // VecDeque::pop_front() is O(1).
                //
                // NEVER use Vec::remove(0) here.
                // -------------------------------------------------------------

                for output in data.iter_mut() {
                    *output = queue.pop_front().unwrap_or(0.0);
                }

                // If the callback consumed the entire queue, the receiver
                // thread will refill it. We don't modify playback_started
                // here because the CPAL callback should stay extremely cheap.
            },
            // =================================================================
            // ERROR CALLBACK
            // =================================================================
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

// ============================================================================
// CHANNEL CONVERSION
// ============================================================================

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

// ============================================================================
// SEQUENCE NUMBER HELPERS
// ============================================================================
//
// UDP sequence numbers eventually wrap around u32::MAX.
//
// These helpers make comparisons work correctly across the wrap.
//

fn sequence_after(a: u32, b: u32) -> bool {
    let diff = a.wrapping_sub(b);

    diff != 0 && diff < 0x8000_0000
}

fn sequence_before(a: u32, b: u32) -> bool {
    let diff = a.wrapping_sub(b);

    diff != 0 && diff >= 0x8000_0000
}

// ============================================================================
// OPTIONAL UTILITY
// ============================================================================

fn stereo_to_mono(stereo_data: &[f32]) -> Vec<f32> {
    let mut mono = Vec::with_capacity(stereo_data.len() / 2);

    for chunk in stereo_data.chunks_exact(2) {
        mono.push((chunk[0] + chunk[1]) * 0.5);
    }

    mono
}
