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

    let socket = Arc::new(
        UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Failed to bind UDP socket: {e}"))?,
    );
    socket
        .connect(&hostname)
        .map_err(|e| format!("Failed to connect UDP socket: {e}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|e| e.to_string())?;
    socket
        .send(&pin.to_be_bytes())
        .map_err(|e| format!("Failed to send pin: {e}"))?;

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

    // Lock-Free Ring Buffer setup (Hold 100ms max buffer)
    let rb = HeapRb::<f32>::new(8820);
    let (mut producer, consumer) = rb.split();

    let output_config = build_output_config(&output_device)?;
    let output_config = Arc::new(Mutex::new(output_config));
    let needs_output_rebuild = Arc::new(AtomicBool::new(false));

    // Consumer moved directly without Mutex wrapping
    let output_stream = build_output_stream(
        &output_device,
        &output_config.lock().unwrap(),
        consumer,
        needs_output_rebuild.clone(),
    )?;

    output_stream
        .play()
        .map_err(|e| format!("Failed to start output: {e}"))?;
    let output_stream = Arc::new(Mutex::new(output_stream));

    // Setup input stream with stack/pre-allocated buffers
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

    let mut input_buffer = Vec::with_capacity(4096);
    let mut packet_buffer = Vec::with_capacity(4096);
    let mut sequence = 0u32;
    let mut net_packet = vec![0u8; HEADER_SIZE + PACKET_SAMPLES * 2];

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                input_buffer.extend_from_slice(data);

                let input_size = input_resampler.chunk_size_input();
                let output_size = input_resampler.chunk_size_output();

                while input_buffer.len() >= input_size {
                    let input_chunk: Vec<f32> = input_buffer.drain(..input_size).collect();
                    let mut output = vec![0.0; output_size];

                    if input_resampler.resample(&input_chunk, &mut output).is_ok() {
                        packet_buffer.extend(output);

                        while packet_buffer.len() >= PACKET_SAMPLES {
                            let samples = packet_buffer.drain(..PACKET_SAMPLES);

                            net_packet[0..4].copy_from_slice(&sequence.to_be_bytes());
                            for (i, sample) in samples.enumerate() {
                                let pcm = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                                let offset = HEADER_SIZE + i * 2;
                                net_packet[offset..offset + 2].copy_from_slice(&pcm.to_be_bytes());
                            }

                            let _ = input_socket.send(&net_packet);
                            sequence = sequence.wrapping_add(1);
                        }
                    }
                }
            },
            |err| eprintln!("[vc] input error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    // UDP Receiver Thread
    {
        let socket = socket.clone();
        let output_config = output_config.clone();
        let shutdown = shutdown.clone();

        thread::spawn(move || {
            let initial_config = output_config.lock().unwrap().clone();
            let mut output_resampler = match create_output_resampler(initial_config.sample_rate) {
                Ok(r) => r,
                Err(e) => return eprintln!("[vc] output resampler: {e}"),
            };

            let mut last_sample_rate = initial_config.sample_rate;
            let mut packets: BTreeMap<u32, Vec<f32>> = BTreeMap::new();
            let mut expected: Option<u32> = None;
            let mut is_prebuffering = true;

            let mut resample_buffer = Vec::with_capacity(8192);
            let mut udp_buffer = [0u8; MAX_PACKET_SIZE];

            while !shutdown.load(Ordering::Relaxed) {
                if let Ok(len) = socket.recv(&mut udp_buffer) {
                    if len > HEADER_SIZE {
                        let sequence = u32::from_be_bytes(udp_buffer[0..4].try_into().unwrap());
                        let pcm = &udp_buffer[HEADER_SIZE..len];
                        let samples: Vec<f32> = pcm
                            .chunks_exact(2)
                            .map(|c| i16::from_be_bytes([c[0], c[1]]) as f32 / 32768.0)
                            .collect();

                        packets.entry(sequence).or_insert(samples);
                    }
                }

                let current_sr = output_config.lock().unwrap().sample_rate;
                if current_sr != last_sample_rate {
                    if let Ok(nr) = create_output_resampler(current_sr) {
                        output_resampler = nr;
                        last_sample_rate = current_sr;
                        packets.clear();
                        expected = None;
                        is_prebuffering = true;
                    }
                }

                if is_prebuffering {
                    if packets.len() >= INITIAL_PACKET_CUSHION {
                        expected = packets.keys().next().copied();
                        is_prebuffering = false;
                    } else {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                }

                while let Some(seq) = expected {
                    if let Some(samples) = packets.remove(&seq) {
                        resample_buffer.extend(samples);
                        expected = Some(seq.wrapping_add(1));
                    } else if packets.keys().any(|&x| x > seq) {
                        resample_buffer.extend(std::iter::repeat(0.0).take(PACKET_SAMPLES));
                        expected = Some(seq.wrapping_add(1));
                    } else {
                        break;
                    }
                }

                let input_size = output_resampler.chunk_size_input();
                let output_size = output_resampler.chunk_size_output();

                while resample_buffer.len() >= input_size {
                    let input_chunk: Vec<f32> = resample_buffer.drain(..input_size).collect();
                    let mut output = vec![0.0; output_size];

                    if output_resampler.resample(&input_chunk, &mut output).is_ok() {
                        // Push directly without Mutex lock!
                        let _ = producer.push_slice(&output);
                    }
                }

                thread::sleep(Duration::from_millis(2));
            }
        });
    }

    input_stream
        .play()
        .map_err(|e| format!("Failed to start input: {e}"))?;

    *voice_state.session.lock().unwrap() = Some(VoiceSession {
        input_stream,
        output_stream,
        socket,
        pin,
        shutdown,
    });

    Ok(())
}

fn build_output_config(device: &cpal::Device) -> Result<cpal::StreamConfig, String> {
    device
        .default_output_config()
        .map(Into::into)
        .map_err(|e| e.to_string())
}

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

fn build_output_stream<C>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut consumer: C,
    needs_rebuild: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let channels = config.channels as usize;

    device
        .build_output_stream(
            config.clone(),
            move |data: &mut [f32], _| {
                let mut idx = 0;
                while idx < data.len() {
                    if let Some(mono_sample) = consumer.try_pop() {
                        for ch in 0..channels {
                            data[idx + ch] = mono_sample;
                        }
                        idx += channels;
                    } else {
                        data[idx..].fill(0.0);
                        break;
                    }
                }
            },
            move |err| {
                eprintln!("[vc] output error: {err}");

                needs_rebuild.store(true, Ordering::SeqCst);
            },
            None,
        )
        .map_err(|e| e.to_string())
}
