use std::{
    collections::BTreeMap,
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};
use tauri::State;

use crate::commands::config::ConfigState;

const SAMPLE_RATE: u32 = 48000;
const PACKET_SAMPLES: usize = 960; // 20ms @ 48kHz
const HEADER_SIZE: usize = 4;
const MAX_PACKET_SIZE: usize = 4096;

const INITIAL_PACKET_CUSHION: usize = 3; // ~60ms cushion

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
        .set_read_timeout(Some(Duration::from_millis(5)))
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

    // Lock-Free Ring Buffer setup (19.2k samples ~ 400ms buffer capacity at 48kHz)
    let rb_out = HeapRb::<f32>::new(19200);
    let (mut producer_out, consumer_out) = rb_out.split();

    let output_config = cpal::StreamConfig {
        channels: 2,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Default,
    };

    let output_stream = build_output_stream(&output_device, output_config, consumer_out)?;
    output_stream
        .play()
        .map_err(|e| format!("Failed to start output: {e}"))?;
    let output_stream = Arc::new(Mutex::new(output_stream));

    // Configure Input Stream with dedicated thread ring-buffer
    let input_config = cpal::StreamConfig {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        buffer_size: cpal::BufferSize::Default,
    };

    let rb_in = HeapRb::<f32>::new(19200);
    let (mut producer_in, mut consumer_in) = rb_in.split();

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                let _ = producer_in.push_slice(data);
            },
            |err| eprintln!("[vc] input error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    // UDP Sender Thread - Pumps exact 20ms frames smoothly
    {
        let input_socket = socket.clone();
        let shutdown = shutdown.clone();

        thread::spawn(move || {
            let mut sequence = 0u32;
            let mut net_packet = vec![0u8; HEADER_SIZE + PACKET_SAMPLES * 2];
            let mut frame_buf = vec![0.0f32; PACKET_SAMPLES];

            while !shutdown.load(Ordering::Relaxed) {
                if consumer_in.occupied_len() >= PACKET_SAMPLES {
                    let _ = consumer_in.pop_slice(&mut frame_buf);

                    net_packet[0..4].copy_from_slice(&sequence.to_be_bytes());
                    for (i, sample) in frame_buf.iter().enumerate() {
                        let pcm = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                        let offset = HEADER_SIZE + i * 2;
                        net_packet[offset..offset + 2].copy_from_slice(&pcm.to_be_bytes());
                    }

                    let _ = input_socket.send(&net_packet);
                    sequence = sequence.wrapping_add(1);
                } else {
                    thread::sleep(Duration::from_millis(2));
                }
            }
        });
    }

    // UDP Receiver Thread - Paced Jitter Buffer
    {
        let socket = socket.clone();
        let shutdown = shutdown.clone();

        thread::spawn(move || {
            let mut packets: BTreeMap<u32, Vec<f32>> = BTreeMap::new();
            let mut expected: Option<u32> = None;
            let mut is_prebuffering = true;
            let mut last_good_frame = vec![0.0f32; PACKET_SAMPLES];
            let mut udp_buffer = [0u8; MAX_PACKET_SIZE];
            let mut next_frame_time = Instant::now();

            while !shutdown.load(Ordering::Relaxed) {
                // Non-blocking UDP receive
                if let Ok(len) = socket.recv(&mut udp_buffer) {
                    if len > HEADER_SIZE {
                        let seq = u32::from_be_bytes(udp_buffer[0..4].try_into().unwrap());
                        let pcm = &udp_buffer[HEADER_SIZE..len];
                        let samples: Vec<f32> = pcm
                            .chunks_exact(2)
                            .map(|c| i16::from_be_bytes([c[0], c[1]]) as f32 / 32768.0)
                            .collect();

                        packets.insert(seq, samples);
                    }
                }

                if is_prebuffering {
                    if packets.len() >= INITIAL_PACKET_CUSHION {
                        expected = packets.keys().next().copied();
                        is_prebuffering = false;
                        next_frame_time = Instant::now();
                    } else {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                }

                // Paced playback frame-by-frame (20ms target interval)
                if Instant::now() >= next_frame_time {
                    if let Some(seq) = expected {
                        if let Some(samples) = packets.remove(&seq) {
                            if samples.len() == PACKET_SAMPLES {
                                last_good_frame.copy_from_slice(&samples);
                            } else {
                                last_good_frame.clear();
                                last_good_frame
                                    .extend(samples.iter().take(PACKET_SAMPLES).copied());
                                if last_good_frame.len() < PACKET_SAMPLES {
                                    last_good_frame.resize(PACKET_SAMPLES, 0.0);
                                }
                            }

                            let _ = producer_out.push_slice(&last_good_frame);
                            expected = Some(seq.wrapping_add(1));
                        } else if packets.keys().any(|&x| x > seq) {
                            // PLC (Packet Loss Concealment)
                            for sample in last_good_frame.iter_mut() {
                                *sample *= 0.65;
                            }
                            let _ = producer_out.push_slice(&last_good_frame);
                            expected = Some(seq.wrapping_add(1));
                        } else {
                            // Re-buffer if packet stream dropped
                            if packets.is_empty() {
                                is_prebuffering = true;
                            }
                        }
                    }
                    next_frame_time += Duration::from_millis(20);
                }

                thread::sleep(Duration::from_millis(1));
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

// ============================================================================
// AUDIO CALLBACK
// ============================================================================

fn build_output_stream<C>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut consumer: C,
) -> Result<cpal::Stream, String>
where
    C: Consumer<Item = f32> + Send + 'static,
{
    let channels = config.channels as usize;
    let mut last_sample = 0.0f32;

    device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| {
                let mut idx = 0;
                while idx < data.len() {
                    if let Some(mono_sample) = consumer.try_pop() {
                        last_sample = mono_sample;
                        for ch in 0..channels {
                            data[idx + ch] = mono_sample;
                        }
                        idx += channels;
                    } else {
                        // Smooth fade out to avoid clicks on underruns
                        while idx < data.len() {
                            last_sample *= 0.92;
                            for ch in 0..channels {
                                data[idx + ch] = last_sample;
                            }
                            idx += channels;
                        }
                        break;
                    }
                }
            },
            |err| eprintln!("[vc] output error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}
