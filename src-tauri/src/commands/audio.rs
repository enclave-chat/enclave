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
    storage::Heap,
    traits::{Consumer, Observer, Producer, Split},
    CachingCons, CachingProd, HeapRb, SharedRb,
};
use tauri::State;

use crate::commands::config::ConfigState;

const PACKET_SAMPLES: usize = 960; // 20ms @ 48kHz
const HEADER_SIZE: usize = 4;
const MAX_PACKET_SIZE: usize = 4096;

const INITIAL_PACKET_CUSHION: usize = 3; // ~60ms cushion

// --- Voice Activity Detection (VAD) Settings ---
const VAD_THRESHOLD: f32 = 0.01;
const VAD_HANGOVER_FRAMES: usize = 10;

// ============================================================================
// STATE & TYPES
// ============================================================================

type AudioProducer = Arc<Mutex<CachingProd<Arc<SharedRb<Heap<f32>>>>>>;
type AudioConsumer = Arc<Mutex<CachingCons<Arc<SharedRb<Heap<f32>>>>>>;

#[derive(Clone, Default)]
pub struct VoiceState {
    pub inner: Arc<VoiceStateInner>,
}

#[derive(Default)]
pub struct VoiceStateInner {
    pub session: Mutex<Option<VoiceSession>>,
}

pub struct VoiceSession {
    pub input_stream: cpal::Stream,
    pub output_stream: Arc<Mutex<cpal::Stream>>,
    pub socket: Arc<UdpSocket>,
    pub pin: u64,
    pub shutdown: Arc<AtomicBool>,

    pub current_input_device: Option<String>,
    pub current_output_device: Option<String>,

    pub producer_in: AudioProducer,
    pub consumer_out: AudioConsumer,
}

impl VoiceSession {
    pub fn update_input_device(
        &mut self,
        device_name: Option<String>,
        state_inner: Arc<VoiceStateInner>,
    ) -> Result<(), String> {
        let host = cpal::default_host();
        let device = match &device_name {
            Some(name) => host
                .input_devices()
                .map_err(|e| e.to_string())?
                .find(|d| {
                    d.description()
                        .ok()
                        .map(|x| x.name() == *name)
                        .unwrap_or(false)
                })
                .ok_or_else(|| format!("Input device '{name}' not found"))?,
            None => host
                .default_input_device()
                .ok_or("No default input device")?,
        };

        let input_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {e}"))?
            .config();

        let producer = Arc::clone(&self.producer_in);
        let inner_clone = Arc::clone(&state_inner);

        let new_stream = device
            .build_input_stream(
                input_config,
                move |data: &[f32], _| {
                    if let Ok(mut prod) = producer.lock() {
                        let _ = prod.push_slice(data);
                    }
                },
                move |err| {
                    eprintln!("[vc] Input error: {err}. Attempting input stream recovery...");
                    if let Ok(mut lock) = inner_clone.session.lock() {
                        if let Some(session) = lock.as_mut() {
                            let target_device = session.current_input_device.clone();
                            if let Err(e) =
                                session.update_input_device(target_device, Arc::clone(&inner_clone))
                            {
                                eprintln!("[vc] Input recovery failed: {e}");
                            }
                        }
                    }
                },
                None,
            )
            .map_err(|e| e.to_string())?;

        new_stream
            .play()
            .map_err(|e| format!("Failed to play input stream: {e}"))?;

        self.input_stream = new_stream;
        self.current_input_device = device_name;
        Ok(())
    }

    pub fn update_output_device(
        &mut self,
        device_name: Option<String>,
        state_inner: Arc<VoiceStateInner>,
    ) -> Result<(), String> {
        let host = cpal::default_host();
        let device = match &device_name {
            Some(name) => host
                .output_devices()
                .map_err(|e| e.to_string())?
                .find(|d| {
                    d.description()
                        .ok()
                        .map(|x| x.name() == *name)
                        .unwrap_or(false)
                })
                .ok_or_else(|| format!("Output device '{name}' not found"))?,
            None => host
                .default_output_device()
                .ok_or("No default output device")?,
        };

        let output_config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get default output config: {e}"))?
            .config();

        let consumer = Arc::clone(&self.consumer_out);
        let new_stream = build_output_stream(&device, output_config, consumer, state_inner)?;
        new_stream
            .play()
            .map_err(|e| format!("Failed to play output stream: {e}"))?;

        if let Ok(mut active_stream) = self.output_stream.lock() {
            *active_stream = new_stream;
        }

        self.current_output_device = device_name;
        Ok(())
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
pub fn disconnect_from_vc(voice_state: State<'_, VoiceState>) -> Result<(), String> {
    let mut lock = voice_state
        .inner
        .session
        .lock()
        .map_err(|e| e.to_string())?;

    if let Some(session) = lock.take() {
        session.shutdown.store(true, Ordering::SeqCst);

        let _ = session.input_stream.pause();
        if let Ok(output) = session.output_stream.lock() {
            let _ = output.pause();
        }
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
    config_state: State<'_, ConfigState>,
    voice_state: State<'_, VoiceState>,
) -> Result<(), String> {
    disconnect_from_vc(voice_state.clone())?;

    let config = config_state.0.lock().unwrap().clone();
    let state_inner = Arc::clone(&voice_state.inner);

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

    // Output Ring Buffer setup
    let rb_out = HeapRb::<f32>::new(19200);
    let (producer_out, consumer_out) = rb_out.split();
    let mut producer_out = producer_out;
    let shared_consumer_out = Arc::new(Mutex::new(consumer_out));

    let output_config = output_device
        .default_output_config()
        .map_err(|e| format!("Failed to get default output config: {e}"))?
        .config();

    let output_stream = build_output_stream(
        &output_device,
        output_config,
        Arc::clone(&shared_consumer_out),
        Arc::clone(&state_inner),
    )?;
    output_stream
        .play()
        .map_err(|e| format!("Failed to start output: {e}"))?;
    let output_stream = Arc::new(Mutex::new(output_stream));

    // Input Ring Buffer setup
    let rb_in = HeapRb::<f32>::new(19200);
    let (producer_in, mut consumer_in) = rb_in.split();
    let shared_producer_in = Arc::new(Mutex::new(producer_in));

    let input_config = input_device
        .default_input_config()
        .map_err(|e| format!("Failed to get default input config: {e}"))?
        .config();

    let cb_producer = Arc::clone(&shared_producer_in);
    let inner_input_err = Arc::clone(&state_inner);

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                if let Ok(mut prod) = cb_producer.lock() {
                    let _ = prod.push_slice(data);
                }
            },
            move |err| {
                eprintln!("[vc] Input error: {err}. Attempting input stream recovery...");
                if let Ok(mut lock) = inner_input_err.session.lock() {
                    if let Some(session) = lock.as_mut() {
                        let target_device = session.current_input_device.clone();
                        if let Err(e) =
                            session.update_input_device(target_device, Arc::clone(&inner_input_err))
                        {
                            eprintln!("[vc] Input recovery failed: {e}");
                        }
                    }
                }
            },
            None,
        )
        .map_err(|e| e.to_string())?;

    // UDP Sender Thread
    {
        let input_socket = socket.clone();
        let shutdown = shutdown.clone();

        thread::spawn(move || {
            let mut sequence = 0u32;
            let mut net_packet = vec![0u8; HEADER_SIZE + PACKET_SAMPLES * 2];
            let mut frame_buf = vec![0.0f32; PACKET_SAMPLES];
            let mut hangover_counter = 0;

            while !shutdown.load(Ordering::Relaxed) {
                if consumer_in.occupied_len() >= PACKET_SAMPLES {
                    let _ = consumer_in.pop_slice(&mut frame_buf);

                    let sum_squares: f32 = frame_buf.iter().map(|&s| s * s).sum();
                    let rms = (sum_squares / PACKET_SAMPLES as f32).sqrt();

                    let is_speaking = if rms >= VAD_THRESHOLD {
                        hangover_counter = VAD_HANGOVER_FRAMES;
                        true
                    } else if hangover_counter > 0 {
                        hangover_counter -= 1;
                        true
                    } else {
                        false
                    };

                    if is_speaking {
                        net_packet[0..4].copy_from_slice(&sequence.to_be_bytes());
                        for (i, sample) in frame_buf.iter().enumerate() {
                            let pcm = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                            let offset = HEADER_SIZE + i * 2;
                            net_packet[offset..offset + 2].copy_from_slice(&pcm.to_be_bytes());
                        }

                        let _ = input_socket.send(&net_packet);
                        sequence = sequence.wrapping_add(1);
                    }
                } else {
                    thread::sleep(Duration::from_millis(2));
                }
            }
        });
    }

    // UDP Receiver Thread
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
                            for sample in last_good_frame.iter_mut() {
                                *sample *= 0.65;
                            }
                            let _ = producer_out.push_slice(&last_good_frame);
                            expected = Some(seq.wrapping_add(1));
                        } else if packets.is_empty() {
                            is_prebuffering = true;
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

    *state_inner.session.lock().unwrap() = Some(VoiceSession {
        input_stream,
        output_stream,
        socket,
        pin,
        shutdown,
        current_input_device: config.input_device_name,
        current_output_device: config.output_device_name,
        producer_in: shared_producer_in,
        consumer_out: shared_consumer_out,
    });

    Ok(())
}

// ============================================================================
// AUDIO CALLBACK
// ============================================================================

fn build_output_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    consumer: AudioConsumer,
    state_inner: Arc<VoiceStateInner>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let mut last_sample = 0.0f32;
    let inner_output_err = Arc::clone(&state_inner);

    device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| {
                let mut idx = 0;
                let mut cons_guard = consumer.lock().ok();

                while idx < data.len() {
                    let sample = cons_guard.as_mut().and_then(|c| c.try_pop());
                    if let Some(mono_sample) = sample {
                        last_sample = mono_sample;
                        for ch in 0..channels {
                            data[idx + ch] = mono_sample;
                        }
                        idx += channels;
                    } else {
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
            move |err| {
                eprintln!("[vc] Output error: {err}. Attempting output stream recovery...");
                if let Ok(mut lock) = inner_output_err.session.lock() {
                    if let Some(session) = lock.as_mut() {
                        let target_device = session.current_output_device.clone();
                        if let Err(e) = session
                            .update_output_device(target_device, Arc::clone(&inner_output_err))
                        {
                            eprintln!("[vc] Output recovery failed: {e}");
                        }
                    }
                }
            },
            None,
        )
        .map_err(|e| e.to_string())
}

impl Drop for VoiceSession {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);

        let _ = self.input_stream.pause();
        if let Ok(output) = self.output_stream.lock() {
            let _ = output.pause();
        }

        eprintln!("[vc] VoiceSession dropped and audio streams paused.");
    }
}
