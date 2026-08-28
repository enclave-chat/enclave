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

use chacha20poly1305::{aead::KeyInit, ChaCha20Poly1305, Key};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, SampleFormat, SizedSample,
};
use ringbuf::{
    storage::Heap,
    traits::{Consumer, Observer, Producer, Split},
    CachingCons, CachingProd, HeapRb, SharedRb,
};
use tauri::State;

use crate::{commands::config::ConfigState, crypto::SessionCipher};

const TARGET_SAMPLE_RATE: u32 = 48_000;
const PACKET_SAMPLES: usize = 960;
const MAX_PACKET_SIZE: usize = 4096;

const INITIAL_PACKET_CUSHION: usize = 3;

const VAD_THRESHOLD: f32 = 0.01;
const VAD_HANGOVER_FRAMES: usize = 10;

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
    pub input_stream: Mutex<Option<cpal::Stream>>,
    pub output_stream: Arc<Mutex<Option<cpal::Stream>>>,
    pub socket: Arc<UdpSocket>,
    pub pin: u64,
    pub shutdown: Arc<AtomicBool>,

    pub current_input_device: Option<String>,
    pub current_output_device: Option<String>,

    pub producer_in: AudioProducer,
    pub consumer_out: AudioConsumer,
}

fn resolve_input_device(device_name: &Option<String>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    match device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| format!("Failed to enumerate input devices: {e}"))?
            .find(|d| {
                d.description()
                    .ok()
                    .map(|x| x.name() == *name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("Input device not found: {name}")),
        None => host.default_input_device().ok_or_else(|| {
            format!("No default input device")
        }),
    }
}

fn resolve_output_device(device_name: &Option<String>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    match device_name {
        Some(name) => host
            .output_devices()
            .map_err(|e| format!("Failed to enumerate output devices: {e}"))?
            .find(|d| {
                d.description()
                    .ok()
                    .map(|x| x.name() == *name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("Output device not found: {name}")),
        None => host.default_output_device().ok_or_else(|| {
            format!("No default output device")
        }),
    }
}

// Simple Linear Resampler for real-time audio conversion
struct LinearResampler {
    phase: f64,
}

impl LinearResampler {
    fn new() -> Self {
        Self { phase: 0.0 }
    }

    /// Resamples dynamic buffers from `src_rate` to `dst_rate`
    fn process(&mut self, input: &[f32], src_rate: u32, dst_rate: u32, output: &mut Vec<f32>) {
        if src_rate == dst_rate {
            output.extend_from_slice(input);
            return;
        }

        let ratio = src_rate as f64 / dst_rate as f64;
        while self.phase < input.len() as f64 {
            let idx = self.phase as usize;
            let frac = (self.phase - idx as f64) as f32;
            let next_idx = (idx + 1).min(input.len() - 1);

            let sample = input[idx] * (1.0 - frac) + input[next_idx] * frac;
            output.push(sample);

            self.phase += ratio;
        }

        self.phase -= input.len() as f64;
    }
}

impl VoiceSession {
    // Single code path for (re)initializing the input stream. Used both for the
    // initial connect and for changing/recovering the device.
    pub fn setup_input(
        &mut self,
        device_name: Option<String>,
        state_inner: Arc<VoiceStateInner>,
    ) -> Result<(), String> {
        let device = resolve_input_device(&device_name)?;

        let input_config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {e}"))?
            .config();

        eprintln!(
            "[vc] Input device {:?} config: {} Hz, {} ch",
            device.description().map(|d| d.name().to_string()),
            input_config.sample_rate,
            input_config.channels
        );

        let producer = Arc::clone(&self.producer_in);
        let new_stream = build_input_stream(&device, input_config, producer, state_inner)?;
        new_stream
            .play()
            .map_err(|e| format!("Failed to play input stream: {e}"))?;

        if let Ok(mut slot) = self.input_stream.lock() {
            *slot = Some(new_stream);
        }
        self.current_input_device = device_name;
        Ok(())
    }

    // Single code path for (re)initializing the output stream. Used both for the
    // initial connect and for changing/recovering the device.
    pub fn setup_output(
        &mut self,
        device_name: Option<String>,
        state_inner: Arc<VoiceStateInner>,
    ) -> Result<(), String> {
        let device = resolve_output_device(&device_name)?;

        let output_config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get output config: {e}"))?
            .config();

        eprintln!(
            "[vc] Output device {:?} config: {} Hz, {} ch",
            device.description().map(|d| d.name().to_string()),
            output_config.sample_rate,
            output_config.channels
        );

        let consumer = Arc::clone(&self.consumer_out);
        let new_stream = build_output_stream(&device, output_config, consumer, state_inner)?;
        new_stream
            .play()
            .map_err(|e| format!("Failed to play output stream: {e}"))?;

        if let Ok(mut slot) = self.output_stream.lock() {
            *slot = Some(new_stream);
        }
        self.current_output_device = device_name;
        Ok(())
    }
}

// Helper to build normalized Input Stream (Resampled & Downmixed to 48kHz Mono)
fn build_input_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    producer: AudioProducer,
    state_inner: Arc<VoiceStateInner>,
) -> Result<cpal::Stream, String> {
    let sample_format = device
        .default_input_config()
        .map_err(|e| format!("Failed to get input config: {e}"))?
        .sample_format();

    match sample_format {
        SampleFormat::F32 => build_input_stream_with::<f32>(device, config, producer, state_inner),
        SampleFormat::I16 => build_input_stream_with::<i16>(device, config, producer, state_inner),
        SampleFormat::I32 => build_input_stream_with::<i32>(device, config, producer, state_inner),
        SampleFormat::I64 => build_input_stream_with::<i64>(device, config, producer, state_inner),
        SampleFormat::U8 => build_input_stream_with::<u8>(device, config, producer, state_inner),
        SampleFormat::U16 => build_input_stream_with::<u16>(device, config, producer, state_inner),
        SampleFormat::U32 => build_input_stream_with::<u32>(device, config, producer, state_inner),
        SampleFormat::U64 => build_input_stream_with::<u64>(device, config, producer, state_inner),
        format => Err(format!("Unsupported input sample format: {format}")),
    }
}

fn build_input_stream_with<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    producer: AudioProducer,
    state_inner: Arc<VoiceStateInner>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let native_sample_rate = config.sample_rate;
    let channels = config.channels as usize;
    let mut resampler = LinearResampler::new();
    let mut mono_buffer = Vec::with_capacity(2048);
    let mut resampled_buffer = Vec::with_capacity(2048);
    let mut callback_count: u64 = 0;

    let inner_input_err = Arc::clone(&state_inner);

    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                callback_count += 1;
                if callback_count % 200 == 0 {
                    let peak: f32 = data
                        .iter()
                        .map(|&s| s.to_sample::<f32>().abs())
                        .fold(0.0f32, f32::max);
                    eprintln!(
                        "[vc] input callback #{callback_count}: {} frames, {} ch, peak {:.4}",
                        data.len() / channels,
                        channels,
                        peak
                    );
                }

                mono_buffer.clear();
                resampled_buffer.clear();

                // Downmix channels to mono, converting to f32
                for chunk in data.chunks_exact(channels) {
                    let sum: f32 = chunk.iter().map(|&s| s.to_sample::<f32>()).sum();
                    mono_buffer.push(sum / channels as f32);
                }

                // Resample to 48kHz standard target
                resampler.process(
                    &mono_buffer,
                    native_sample_rate,
                    TARGET_SAMPLE_RATE,
                    &mut resampled_buffer,
                );

                if let Ok(mut prod) = producer.lock() {
                    let _ = prod.push_slice(&resampled_buffer);
                }
            },
            move |err| {
                eprintln!("[vc] Input error: {err}. Attempting recovery...");
                let rec = Arc::clone(&inner_input_err);
                thread::spawn(move || {
                    // Give the errored stream a moment to unwind before rebuilding.
                    thread::sleep(Duration::from_millis(10));
                    let Ok(mut lock) = rec.session.lock() else {
                        return;
                    };
                    if let Some(session) = lock.as_mut() {
                        if session.shutdown.load(Ordering::SeqCst) {
                            return;
                        }
                        let target_device = session.current_input_device.clone();
                        let _ = session.setup_input(target_device, Arc::clone(&rec));
                    }
                });
            },
            None,
        )
        .map_err(|e| e.to_string())
}

// Helper to build normalized Output Stream (48kHz Mono -> Device Native Channels & Rate)
fn build_output_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    consumer: AudioConsumer,
    state_inner: Arc<VoiceStateInner>,
) -> Result<cpal::Stream, String> {
    let sample_format = device
        .default_output_config()
        .map_err(|e| format!("Failed to get output config: {e}"))?
        .sample_format();

    match sample_format {
        SampleFormat::F32 => build_output_stream_with::<f32>(device, config, consumer, state_inner),
        SampleFormat::I16 => build_output_stream_with::<i16>(device, config, consumer, state_inner),
        SampleFormat::I32 => build_output_stream_with::<i32>(device, config, consumer, state_inner),
        SampleFormat::I64 => build_output_stream_with::<i64>(device, config, consumer, state_inner),
        SampleFormat::U8 => build_output_stream_with::<u8>(device, config, consumer, state_inner),
        SampleFormat::U16 => build_output_stream_with::<u16>(device, config, consumer, state_inner),
        SampleFormat::U32 => build_output_stream_with::<u32>(device, config, consumer, state_inner),
        SampleFormat::U64 => build_output_stream_with::<u64>(device, config, consumer, state_inner),
        format => Err(format!("Unsupported output sample format: {format}")),
    }
}

fn build_output_stream_with<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    consumer: AudioConsumer,
    state_inner: Arc<VoiceStateInner>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + Send + 'static,
    T: FromSample<f32>,
{
    let native_sample_rate = config.sample_rate;
    let channels = config.channels as usize;
    let mut resampler = LinearResampler::new();
    let mut raw_mono_samples = Vec::with_capacity(2048);
    let mut resampled_mono = Vec::with_capacity(2048);
    let mut last_sample = 0.0f32;
    let mut callback_count: u64 = 0;

    let inner_output_err = Arc::clone(&state_inner);

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                callback_count += 1;
                if callback_count % 200 == 0 {
                    eprintln!(
                        "[vc] output callback #{callback_count}: {} frames, {} ch",
                        data.len() / channels,
                        channels
                    );
                }

                let required_mono_samples = (data.len() / channels) * TARGET_SAMPLE_RATE as usize
                    / native_sample_rate as usize;

                raw_mono_samples.clear();
                resampled_mono.clear();

                let mut underflow_count = 0usize;
                if let Ok(mut cons) = consumer.lock() {
                    for _ in 0..required_mono_samples {
                        if let Some(s) = cons.try_pop() {
                            last_sample = s;
                            raw_mono_samples.push(s);
                        } else {
                            // Exponential decay to prevent clicking when underflowing
                            last_sample *= 0.92;
                            raw_mono_samples.push(last_sample);
                            underflow_count += 1;
                        }
                    }
                }
                if callback_count % 200 == 0 {
                    eprintln!(
                        "[vc] output: got {}/{} mono samples ({} underflow)",
                        required_mono_samples - underflow_count,
                        required_mono_samples,
                        underflow_count
                    );
                }

                // Resample from 48kHz mono to target native output rate
                resampler.process(
                    &raw_mono_samples,
                    TARGET_SAMPLE_RATE,
                    native_sample_rate,
                    &mut resampled_mono,
                );

                // Interleave mono into hardware channels
                let mut res_idx = 0;
                let mut out_idx = 0;
                while out_idx < data.len() && res_idx < resampled_mono.len() {
                    let mono_val = resampled_mono[res_idx];
                    for ch in 0..channels {
                        if out_idx + ch < data.len() {
                            data[out_idx + ch] = T::from_sample(mono_val);
                        }
                    }
                    out_idx += channels;
                    res_idx += 1;
                }
            },
            move |err| {
                eprintln!("[vc] Output error: {err}. Attempting recovery...");
                let rec = Arc::clone(&inner_output_err);
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(10));
                    let Ok(mut lock) = rec.session.lock() else {
                        return;
                    };
                    if let Some(session) = lock.as_mut() {
                        if session.shutdown.load(Ordering::SeqCst) {
                            return;
                        }
                        let target_device = session.current_output_device.clone();
                        let _ = session.setup_output(target_device, Arc::clone(&rec));
                    }
                });
            },
            None,
        )
        .map_err(|e| e.to_string())
}

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

#[tauri::command]
pub fn disconnect_from_vc(voice_state: State<'_, VoiceState>) -> Result<(), String> {
    let session = {
        let mut lock = voice_state
            .inner
            .session
            .lock()
            .map_err(|e| e.to_string())?;
        lock.take()
    };

    if let Some(session) = session {
        session.shutdown.store(true, Ordering::SeqCst);
        if let Ok(input) = session.input_stream.lock() {
            if let Some(stream) = input.as_ref() {
                let _ = stream.pause();
            }
        }
        if let Ok(output) = session.output_stream.lock() {
            if let Some(stream) = output.as_ref() {
                let _ = stream.pause();
            }
        }
        eprintln!("[vc] disconnected and paused streams");
    }

    Ok(())
}

#[tauri::command]
pub fn connect_to_vc(
    hostname: String,
    pin: u64,
    shared_secret: Vec<u8>, // output of js `x25519.getSharedSecret`
    config_state: State<'_, ConfigState>,
    voice_state: State<'_, VoiceState>,
) -> Result<(), String> {
    eprintln!("[vc] connect_to_vc called hostname={hostname} pin={pin}");

    if let Err(e) = disconnect_from_vc(voice_state.clone()) {
        eprintln!("[vc] disconnect_from_vc failed: {e}");
    }

    let key = match Key::try_from(shared_secret.as_slice()) {
        Ok(k) => k,
        Err(v) => {
            let msg = v.to_string();
            eprintln!("[vc] invalid key length: {msg}");
            return Err(msg);
        }
    };
    let cipher = Arc::new(Mutex::new(SessionCipher::new(ChaCha20Poly1305::new(&key))));

    let config = config_state.0.lock().unwrap().clone();
    let state_inner = Arc::clone(&voice_state.inner);

    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("Failed to bind UDP socket: {e}");
            eprintln!("[vc] {msg}");
            return Err(msg);
        }
    };
    let socket = Arc::new(socket);
    if let Err(e) = socket.connect(&hostname) {
        let msg = format!("Failed to connect UDP socket: {e}");
        eprintln!("[vc] {msg}");
        return Err(msg);
    }
    if let Err(e) = socket.set_read_timeout(Some(Duration::from_millis(5))) {
        let msg = e.to_string();
        eprintln!("[vc] set_read_timeout failed: {msg}");
        return Err(msg);
    }
    if let Err(e) = socket.send(&pin.to_be_bytes()) {
        let msg = format!("Failed to send pin: {e}");
        eprintln!("[vc] {msg}");
        return Err(msg);
    }

    let shutdown = Arc::new(AtomicBool::new(false));

    // Ring Buffers (device-agnostic; shared between initial setup and any rebuild)
    let rb_out = HeapRb::<f32>::new(19200);
    let (mut producer_out, consumer_out) = rb_out.split();
    let shared_consumer_out = Arc::new(Mutex::new(consumer_out));

    let rb_in = HeapRb::<f32>::new(19200);
    let (producer_in, mut consumer_in) = rb_in.split();
    let shared_producer_in = Arc::new(Mutex::new(producer_in));

    // Build the session with empty stream slots, then (re)initialize the actual
    // device streams through the same setup path used by device changes.
    let mut session = VoiceSession {
        input_stream: Mutex::new(None),
        output_stream: Arc::new(Mutex::new(None)),
        socket: socket.clone(),
        pin,
        shutdown: shutdown.clone(),
        current_input_device: config.input_device_name.clone(),
        current_output_device: config.output_device_name.clone(),
        producer_in: Arc::clone(&shared_producer_in),
        consumer_out: Arc::clone(&shared_consumer_out),
    };

    session.setup_input(config.input_device_name.clone(), Arc::clone(&state_inner))?;
    session.setup_output(config.output_device_name.clone(), Arc::clone(&state_inner))?;

    // Sender Thread
    {
        let input_socket = socket.clone();
        let shutdown = shutdown.clone();
        let cipher = cipher.clone();

        thread::spawn(move || {
            let mut sequence = 0u32;
            let mut frame_buf = vec![0.0f32; PACKET_SAMPLES];
            let mut hangover_counter = 0;
            let mut loop_count: u64 = 0;
            // Adaptive VAD: track a slow-moving noise floor so quiet mics still trigger.
            let mut noise_floor: f32 = 0.0;

            while !shutdown.load(Ordering::Relaxed) {
                if consumer_in.occupied_len() >= PACKET_SAMPLES {
                    let _ = consumer_in.pop_slice(&mut frame_buf);

                    let sum_squares: f32 = frame_buf.iter().map(|&s| s * s).sum();
                    let rms = (sum_squares / PACKET_SAMPLES as f32).sqrt();

                    // Update noise floor (attack fast, release slow).
                    if noise_floor == 0.0 {
                        noise_floor = rms;
                    } else if rms < noise_floor {
                        noise_floor = noise_floor * 0.9 + rms * 0.1;
                    } else {
                        noise_floor = noise_floor * 0.999;
                    }
                    let threshold = (noise_floor * 4.0).max(VAD_THRESHOLD);

                    let is_speaking = if rms >= threshold {
                        hangover_counter = VAD_HANGOVER_FRAMES;
                        true
                    } else if hangover_counter > 0 {
                        hangover_counter -= 1;
                        true
                    } else {
                        false
                    };

                    loop_count += 1;
                    if loop_count % 200 == 0 {
                        eprintln!(
                            "[vc] sender: buffered {} samples, frame rms {rms:.4}, floor {noise_floor:.4}, thr {threshold:.4}, speaking {is_speaking}, seq {sequence}",
                            consumer_in.occupied_len()
                        );
                    }

                    if is_speaking {
                        // sequence goes INSIDE the plaintext now, prefixed before the PCM
                        let mut plaintext = Vec::with_capacity(4 + PACKET_SAMPLES * 2);
                        plaintext.extend_from_slice(&sequence.to_be_bytes());

                        for sample in frame_buf.iter() {
                            let pcm = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                            plaintext.extend_from_slice(&pcm.to_be_bytes());
                        }

                        let Ok(net_packet) = cipher.lock().unwrap().encrypt(&plaintext) else {
                            eprintln!("[vc] failed to encrypt outgoing packet");
                            sequence = sequence.wrapping_add(1);
                            continue;
                        };

                        let sent = input_socket.send(&net_packet);
                        if let Err(e) = sent {
                            eprintln!("[vc] sender: send failed: {e}");
                        } else if loop_count % 200 == 0 {
                            eprintln!(
                                "[vc] sender: sent packet seq {sequence} ({} bytes)",
                                net_packet.len()
                            );
                        }
                        sequence = sequence.wrapping_add(1);
                    }
                } else {
                    thread::sleep(Duration::from_millis(2));
                }
            }
        });
    }

    // Receiver Thread
    {
        let socket = socket.clone();
        let shutdown = shutdown.clone();
        let cipher = cipher.clone();

        thread::spawn(move || {
            let mut packets: BTreeMap<u32, Vec<f32>> = BTreeMap::new();
            let mut expected: Option<u32> = None;
            let mut is_prebuffering = true;
            let mut last_good_frame = vec![0.0f32; PACKET_SAMPLES];
            let mut udp_buffer = [0u8; MAX_PACKET_SIZE];
            let mut next_frame_time = Instant::now();
            let mut recv_count: u64 = 0;
            let mut loop_count: u64 = 0;

            while !shutdown.load(Ordering::Relaxed) {
                loop_count += 1;
                if let Ok(len) = socket.recv(&mut udp_buffer) {
                    recv_count += 1;
                    match cipher.lock().unwrap().decrypt(&udp_buffer[..len]) {
                        Ok(plaintext) => {
                            if plaintext.len() < 4 {
                                eprintln!("[vc] dropped packet: too short after decrypt");
                            } else {
                                let seq = u32::from_be_bytes(plaintext[..4].try_into().unwrap());
                                let pcm = &plaintext[4..];

                                let samples: Vec<f32> = pcm
                                    .chunks_exact(2)
                                    .map(|c| i16::from_be_bytes([c[0], c[1]]) as f32 / 32768.0)
                                    .collect();

                                packets.insert(seq, samples);
                            }
                        }
                        Err(_) => {
                            eprintln!("[vc] dropped packet: decryption failed");
                        }
                    }
                }

                if loop_count % 2000 == 0 {
                    eprintln!(
                        "[vc] receiver: received {recv_count} packets total, {} buffered, prebuffering {is_prebuffering}, expected {expected:?}",
                        packets.len()
                    );
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

    *state_inner.session.lock().unwrap() = Some(session);

    // Workaround for Linux devices that don't start capturing/playing until the
    // stream is (re)initialized. Rebuild both directions a moment after connect.
    {
        let state_for_rebuild = Arc::clone(&state_inner);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(1000));
            let Ok(mut lock) = state_for_rebuild.session.lock() else {
                return;
            };
            if let Some(session) = lock.as_mut() {
                if session.shutdown.load(Ordering::SeqCst) {
                    return;
                }
                let out = session.current_output_device.clone();
                let _ = session.setup_output(out, Arc::clone(&state_for_rebuild));
                let inp = session.current_input_device.clone();
                let _ = session.setup_input(inp, Arc::clone(&state_for_rebuild));
                eprintln!("[vc] Reinitialized input/output streams after connect");
            }
        });
    }

    Ok(())
}

impl Drop for VoiceSession {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);

        if let Ok(input) = self.input_stream.lock() {
            if let Some(stream) = input.as_ref() {
                let _ = stream.pause();
            }
        }
        if let Ok(output) = self.output_stream.lock() {
            if let Some(stream) = output.as_ref() {
                let _ = stream.pause();
            }
        }

        eprintln!("[vc] VoiceSession dropped and audio streams paused.");
    }
}
