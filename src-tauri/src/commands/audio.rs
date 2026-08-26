use std::{
    collections::BTreeMap,
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};
use tauri::State;

use crate::commands::config::ConfigState;

const PACKET_SAMPLES: usize = 882; // 20ms @ 44.1kHz
const HEADER_SIZE: usize = 4;
const MAX_PACKET_SIZE: usize = 4096;

// Jitter buffer settings
const JITTER_BUFFER_MS: usize = 60; // Cushion size before playback starts
const PACKET_DURATION_MS: usize = 20;
const INITIAL_PACKET_CUSHION: usize = JITTER_BUFFER_MS / PACKET_DURATION_MS; // 3 packets

// ============================================================================
// STATE
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
// DEVICES
// ============================================================================

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();

    Ok(host
        .input_devices()
        .map_err(|e| e.to_string())?
        .filter_map(|d| d.description().ok().map(|x| x.name().to_string()))
        .collect())
}

#[tauri::command]
pub fn list_output_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();

    Ok(host
        .output_devices()
        .map_err(|e| e.to_string())?
        .filter_map(|d| d.description().ok().map(|x| x.name().to_string()))
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

    // ------------------------------------------------------------------------
    // UDP
    // ------------------------------------------------------------------------

    let socket = Arc::new(
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind UDP socket: {e}"))?,
    );

    socket
        .connect(&hostname)
        .map_err(|e| format!("Failed to connect UDP socket: {e}"))?;

    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|e| e.to_string())?;

    socket
        .send(&pin.to_be_bytes())
        .map_err(|e| format!("Failed to send pin: {e}"))?;

    // ------------------------------------------------------------------------
    // DEVICES
    // ------------------------------------------------------------------------

    let host = cpal::default_host();

    let input_device = match &config.input_device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| {
                d.description()
                    .ok()
                    .map(|x| x.name() == *name)
                    .unwrap_or(false)
            })
            .ok_or("Input device not found")?,

        None => host
            .default_input_device()
            .ok_or("No default input device")?,
    };

    let output_device = match &config.output_device_name {
        Some(name) => host
            .output_devices()
            .map_err(|e| e.to_string())?
            .find(|d| {
                d.description()
                    .ok()
                    .map(|x| x.name() == *name)
                    .unwrap_or(false)
            })
            .ok_or("Output device not found")?,

        None => host
            .default_output_device()
            .ok_or("No default output device")?,
    };

    let shutdown = Arc::new(AtomicBool::new(false));

    // ------------------------------------------------------------------------
    // PLAYBACK BUFFER (RingBuffer)
    // ------------------------------------------------------------------------

    let max_capacity = 48000;
    let rb = HeapRb::<f32>::new(max_capacity);
    let (producer, consumer) = rb.split();

    let producer_lock = Arc::new(Mutex::new(producer));
    let consumer_lock = Arc::new(Mutex::new(consumer));

    // ------------------------------------------------------------------------
    // OUTPUT
    // ------------------------------------------------------------------------

    let output_config = build_output_config(&output_device)?;
    let output_config = Arc::new(Mutex::new(output_config));

    let needs_output_rebuild = Arc::new(AtomicBool::new(false));

    let output_stream = build_output_stream(
        &output_device,
        &output_config.lock().unwrap(),
        consumer_lock.clone(),
        needs_output_rebuild.clone(),
    )?;

    output_stream
        .play()
        .map_err(|e| format!("Failed to start output: {e}"))?;

    let output_stream = Arc::new(Mutex::new(output_stream));

    // ------------------------------------------------------------------------
    // INPUT
    // ------------------------------------------------------------------------

    let mut input_config: cpal::StreamConfig = input_device
        .default_input_config()
        .map_err(|e| e.to_string())?
        .into();

    input_config.channels = 1;

    let input_sample_rate = input_config.sample_rate;

    let input_socket = socket.clone();

    let mut input_resampler = resampler::ResamplerFft::new(
        1,
        input_sample_rate
            .try_into()
            .map_err(|e| format!("Invalid input sample rate: {e:?}"))?,
        resampler::SampleRate::Hz44100,
    );

    let mut input_buffer = Vec::<f32>::new();
    let mut packet_buffer = Vec::<f32>::new();
    let mut sequence = 0u32;

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                input_buffer.extend_from_slice(data);

                let input_size = input_resampler.chunk_size_input();
                let output_size = input_resampler.chunk_size_output();

                while input_buffer.len() >= input_size {
                    let input: Vec<f32> = input_buffer.drain(..input_size).collect();

                    let mut output = vec![0.0; output_size];

                    if input_resampler.resample(&input, &mut output).is_err() {
                        continue;
                    }

                    packet_buffer.extend(output);

                    while packet_buffer.len() >= PACKET_SAMPLES {
                        let samples: Vec<f32> = packet_buffer.drain(..PACKET_SAMPLES).collect();

                        let mut packet = Vec::with_capacity(HEADER_SIZE + PACKET_SAMPLES * 2);

                        packet.extend_from_slice(&sequence.to_be_bytes());

                        for sample in samples {
                            let pcm = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;

                            packet.extend_from_slice(&pcm.to_be_bytes());
                        }

                        let _ = input_socket.send(&packet);

                        sequence = sequence.wrapping_add(1);
                    }
                }
            },
            |err| {
                eprintln!("[vc] input error: {err}");
            },
            None,
        )
        .map_err(|e| e.to_string())?;

    // ------------------------------------------------------------------------
    // OUTPUT DEVICE REBUILD WATCHER
    // ------------------------------------------------------------------------

    {
        let output_device = output_device.clone();
        let output_stream = output_stream.clone();
        let output_config = output_config.clone();
        let consumer_lock = consumer_lock.clone();
        let needs_rebuild = needs_output_rebuild.clone();
        let shutdown = shutdown.clone();

        thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));

                if !needs_rebuild.swap(false, Ordering::SeqCst) {
                    continue;
                }

                let Ok(new_config) = build_output_config(&output_device) else {
                    needs_rebuild.store(true, Ordering::SeqCst);
                    continue;
                };

                let Ok(new_stream) = build_output_stream(
                    &output_device,
                    &new_config,
                    consumer_lock.clone(),
                    needs_rebuild.clone(),
                ) else {
                    needs_rebuild.store(true, Ordering::SeqCst);
                    continue;
                };

                if new_stream.play().is_err() {
                    needs_rebuild.store(true, Ordering::SeqCst);
                    continue;
                }

                *output_config.lock().unwrap() = new_config;
                *output_stream.lock().unwrap() = new_stream;
            }
        });
    }

    // ------------------------------------------------------------------------
    // UDP RECEIVE (JITTER BUFFER INCLUDED)
    // ------------------------------------------------------------------------

    {
        let socket = socket.clone();
        let output_config = output_config.clone();
        let shutdown = shutdown.clone();

        thread::spawn(move || {
            let initial_config = output_config.lock().unwrap().clone();

            let mut output_resampler = match create_output_resampler(initial_config.sample_rate) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[vc] output resampler: {e}");
                    return;
                }
            };

            let mut last_sample_rate = initial_config.sample_rate;

            let mut packets: BTreeMap<u32, Vec<f32>> = BTreeMap::new();
            let mut expected: Option<u32> = None;
            let mut is_prebuffering = true;

            let mut resample_buffer = Vec::<f32>::new();
            let mut udp_buffer = [0u8; MAX_PACKET_SIZE];

            while !shutdown.load(Ordering::SeqCst) {
                match socket.recv(&mut udp_buffer) {
                    Ok(len) => {
                        if len <= HEADER_SIZE {
                            continue;
                        }

                        let sequence = u32::from_be_bytes([
                            udp_buffer[0],
                            udp_buffer[1],
                            udp_buffer[2],
                            udp_buffer[3],
                        ]);

                        let pcm = &udp_buffer[HEADER_SIZE..len];

                        let mut samples = Vec::with_capacity(pcm.len() / 2);

                        for chunk in pcm.chunks_exact(2) {
                            let value = i16::from_be_bytes([chunk[0], chunk[1]]);

                            samples.push(value as f32 / i16::MAX as f32);
                        }

                        packets.entry(sequence).or_insert(samples);
                    }

                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                        // Reset jitter state if connection completely drops
                        if packets.is_empty() {
                            expected = None;
                            is_prebuffering = true;
                        }
                        continue;
                    }

                    Err(e) => {
                        if !shutdown.load(Ordering::SeqCst) {
                            eprintln!("[vc] UDP receive error: {e}");
                        }
                    }
                }

                // ----------------------------------------------------------------
                // OUTPUT CONFIG CHANGE
                // ----------------------------------------------------------------

                let config = output_config.lock().unwrap().clone();

                if config.sample_rate != last_sample_rate {
                    match create_output_resampler(config.sample_rate) {
                        Ok(new_resampler) => {
                            output_resampler = new_resampler;
                            last_sample_rate = config.sample_rate;

                            packets.clear();
                            resample_buffer.clear();
                            expected = None;
                            is_prebuffering = true;
                        }
                        Err(_) => continue,
                    }
                }

                // ----------------------------------------------------------------
                // JITTER BUFFER STATE MACHINE
                // ----------------------------------------------------------------

                if is_prebuffering {
                    // Accumulate target packet cushion before popping
                    if packets.len() >= INITIAL_PACKET_CUSHION {
                        expected = packets.keys().next().copied();
                        is_prebuffering = false;
                    } else {
                        continue;
                    }
                }

                // Initialize sequence if unset
                if expected.is_none() {
                    expected = packets.keys().next().copied();
                }

                // ----------------------------------------------------------------
                // DRAIN PACKETS IN SEQUENTIAL ORDER
                // ----------------------------------------------------------------

                while let Some(seq) = expected {
                    if let Some(samples) = packets.remove(&seq) {
                        resample_buffer.extend(samples);
                        expected = Some(seq.wrapping_add(1));
                    } else {
                        // Missing frame strategy:
                        // If future sequence numbers exist, insert Concealment (Silence)
                        if packets.keys().any(|&x| x > seq) {
                            resample_buffer.extend(std::iter::repeat(0.0).take(PACKET_SAMPLES));
                            expected = Some(seq.wrapping_add(1));
                        } else {
                            // Waiting on late packets
                            break;
                        }
                    }
                }

                // Trim jitter buffer cache to avoid memory leak spikes on extreme latency drops
                if packets.len() > 50 {
                    packets.clear();
                    expected = None;
                    is_prebuffering = true;
                }

                // ----------------------------------------------------------------
                // RESAMPLE & POPULATE RINGBUFFER
                // ----------------------------------------------------------------

                let input_size = output_resampler.chunk_size_input();
                let output_size = output_resampler.chunk_size_output();

                while resample_buffer.len() >= input_size {
                    let input: Vec<f32> = resample_buffer.drain(..input_size).collect();

                    let mut output = vec![0.0; output_size];

                    if output_resampler.resample(&input, &mut output).is_err() {
                        continue;
                    }

                    let mut prod = producer_lock.lock().unwrap();

                    // Latency Drift Protection: prevent total queue build-up past ~60ms
                    let target_sample_rate = config.sample_rate as usize;
                    let max_ring_buffer_samples = (target_sample_rate / 1000) * 60;

                    // Fix occupied_len check and overflow protection
                    // Note: We avoid calling try_pop() on a Producer. If the buffer is full,
                    // try_push will naturally fail and drop the overflow sample.
                    for sample in output {
                        if (*prod).occupied_len() < max_ring_buffer_samples {
                            let _ = (*prod).try_push(sample);
                        }
                    }
                }
            }
        });
    }

    // ------------------------------------------------------------------------
    // START INPUT
    // ------------------------------------------------------------------------

    input_stream
        .play()
        .map_err(|e| format!("Failed to start input: {e}"))?;

    // ------------------------------------------------------------------------
    // SAVE
    // ------------------------------------------------------------------------

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

fn build_output_config(device: &cpal::Device) -> Result<cpal::StreamConfig, String> {
    device
        .default_output_config()
        .map(Into::into)
        .map_err(|e| e.to_string())
}

// ============================================================================
// RESAMPLER
// ============================================================================

fn create_output_resampler(
    sample_rate: cpal::SampleRate,
) -> Result<resampler::ResamplerFft, String> {
    let rate = sample_rate
        .try_into()
        .map_err(|e| format!("Invalid sample rate: {e:?}"))?;

    Ok(resampler::ResamplerFft::new(
        1,
        resampler::SampleRate::Hz44100,
        rate,
    ))
}

// ============================================================================
// OUTPUT STREAM
// ============================================================================

fn build_output_stream<C>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    consumer_lock: Arc<Mutex<C>>,
    needs_rebuild: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let channels = config.channels as usize;
    let sample_rate = config.sample_rate as usize;
    let jitter_cushion_samples = (sample_rate / 1000) * 40; // 40ms stream cushion

    let mut is_buffering = true;

    device
        .build_output_stream(
            config.clone(),
            move |data: &mut [f32], _| {
                let mut cons = consumer_lock.lock().unwrap();

                // 1. Initial/Recovering Cushioning
                if is_buffering {
                    if (*cons).occupied_len() >= jitter_cushion_samples {
                        is_buffering = false;
                    } else {
                        data.fill(0.0);
                        return;
                    }
                }

                // 2. Sample Extraction & Channel Interleaving
                let mut idx = 0;
                while idx < data.len() {
                    if let Some(mono_sample) = (*cons).try_pop() {
                        for ch in 0..channels {
                            data[idx + ch] = mono_sample;
                        }
                        idx += channels;
                    } else {
                        // 3. Underrun: pad rest with silence & re-enter buffering mode
                        data[idx..].fill(0.0);
                        is_buffering = true;
                        break;
                    }
                }
            },
            {
                let needs_rebuild = needs_rebuild.clone();
                move |err| {
                    eprintln!("[vc] output error: {err}");
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
