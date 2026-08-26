use std::{
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::State;

use crate::commands::config::ConfigState;

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();

    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate input devices: {e}"))?;

    Ok(devices
        .filter_map(|d| Some(d.description().ok()?.name().to_string()))
        .collect())
}

#[tauri::command]
pub fn list_output_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();

    let devices = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate output devices: {e}"))?;

    Ok(devices
        .filter_map(|d| Some(d.description().ok()?.name().to_string()))
        .collect())
}

pub struct VoiceState {
    pub session: Mutex<Option<VoiceSession>>,
}

pub struct VoiceSession {
    pub input_stream: cpal::Stream,
    // Wrapped so the watcher thread can swap in a freshly rebuilt stream.
    pub output_stream: Arc<Mutex<cpal::Stream>>,
    pub socket: Arc<UdpSocket>,
    pub pin: u64,
    // Set to true when the watcher thread should stop (on disconnect).
    pub shutdown: Arc<AtomicBool>,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn disconnect_from_vc(voice_state: State<VoiceState>) -> Result<(), String> {
    if let Some(session) = voice_state.session.lock().unwrap().take() {
        // Tell the watcher thread to stop before dropping the streams.
        session.shutdown.store(true, Ordering::SeqCst);
    }
    // Dropping the session stops both cpal streams.
    Ok(())
}

#[tauri::command]
pub fn connect_to_vc(
    hostname: String,
    pin: u64,
    config_state: State<ConfigState>,
    voice_state: State<VoiceState>,
) -> Result<(), String> {
    disconnect_from_vc(voice_state.clone())?;

    let config = config_state.0.lock().unwrap().clone();

    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.connect(&hostname).map_err(|e| e.to_string())?;
    let socket = Arc::new(socket);

    // Send the pin as the very first packet — authenticates this UDP session
    socket
        .send(&pin.to_be_bytes())
        .map_err(|e| format!("Failed to send pincode: {e}"))?;

    let host = cpal::default_host();

    let input_device = match &config.input_device_name {
        Some(name) => host
            .input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| {
                d.description()
                    .ok()
                    .map(|d| d.name() == name)
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
                    .map(|d| d.name() == name)
                    .unwrap_or(false)
            })
            .ok_or("Output device not found")?,
        None => host
            .default_output_device()
            .ok_or("No default output device")?,
    };

    // ---------- INPUT (mic -> UDP) ----------

    let input_socket = socket.clone();
    let mut input_config: cpal::StreamConfig = input_device
        .default_input_config()
        .map_err(|e| e.to_string())?
        .into();

    input_config.channels = 1;
    let input_sample_rate = input_config.sample_rate;

    let mut input_resampler = resampler::ResamplerFft::new(
        1,
        input_sample_rate.try_into().map_err(|e| format!("{e:?}"))?,
        resampler::SampleRate::Hz44100,
    );

    let mut input_buffer = Vec::<f32>::new();

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                input_buffer.extend_from_slice(data);

                let frame_size = input_resampler.chunk_size_input();

                while input_buffer.len() >= frame_size {
                    let input: Vec<f32> = input_buffer.drain(..frame_size).collect();

                    let mut output = vec![0.0f32; input_resampler.chunk_size_output()];

                    if let Err(e) = input_resampler.resample(&input, &mut output) {
                        eprintln!("[vc] failed to resample input: {e}");
                        continue;
                    }

                    // f32 -> i16 PCM
                    let mut packet = Vec::with_capacity(output.len() * 2);

                    for sample in output {
                        let sample = sample.clamp(-1.0, 1.0);
                        let pcm = (sample * i16::MAX as f32) as i16;

                        packet.extend_from_slice(&pcm.to_be_bytes());
                    }

                    eprintln!(
                        "[vc] sending audio: input={} samples, packet={} bytes",
                        frame_size,
                        packet.len()
                    );

                    match input_socket.send(&packet) {
                        Ok(n) => eprintln!("[vc] UDP sent {n} bytes"),
                        Err(e) => eprintln!("[vc] UDP send failed: {e}"),
                    }
                }
            },
            |err| eprintln!("[vc] input stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    // ---------- OUTPUT (UDP -> speakers), rebuildable on device change ----------

    let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();
    let audio_rx = Arc::new(Mutex::new(audio_rx));

    let needs_output_rebuild = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::new(AtomicBool::new(false));

    // Output config is tracked in a shared cell so the UDP-receiving thread
    // (below) always resamples toward whatever the *current* live stream expects,
    // even after a rebuild changes the device's rate.
    let output_config = build_output_config(&output_device)?;
    let output_config_cell = Arc::new(Mutex::new(output_config.clone()));

    let initial_output_stream = build_output_stream(
        &output_device,
        &output_config,
        audio_rx.clone(),
        needs_output_rebuild.clone(),
    )?;
    initial_output_stream.play().map_err(|e| e.to_string())?;

    let output_stream = Arc::new(Mutex::new(initial_output_stream));

    // Watcher thread: rebuilds the output stream whenever the device signals
    // its config has changed (e.g. "Device sample rate changed"), since cpal
    // streams can't be reconfigured in place — only rebuilt from scratch.
    {
        let output_device = output_device.clone();
        let audio_rx = audio_rx.clone();
        let needs_output_rebuild = needs_output_rebuild.clone();
        let output_stream = output_stream.clone();
        let output_config_cell = output_config_cell.clone();
        let shutdown = shutdown.clone();

        std::thread::spawn(move || loop {
            if shutdown.load(Ordering::SeqCst) {
                return;
            }

            std::thread::sleep(std::time::Duration::from_millis(200));

            if needs_output_rebuild.swap(false, Ordering::SeqCst) {
                match build_output_config(&output_device).and_then(|new_config| {
                    build_output_stream(
                        &output_device,
                        &new_config,
                        audio_rx.clone(),
                        needs_output_rebuild.clone(),
                    )
                    .map(|stream| (new_config, stream))
                }) {
                    Ok((new_config, new_stream)) => {
                        if let Err(e) = new_stream.play() {
                            eprintln!("[vc] failed to start rebuilt output stream: {e}");
                            continue;
                        }
                        *output_config_cell.lock().unwrap() = new_config;
                        *output_stream.lock().unwrap() = new_stream;
                        eprintln!("[vc] output stream rebuilt after device change");
                    }
                    Err(e) => eprintln!("[vc] failed to rebuild output stream: {e}"),
                }
            }
        });
    }

    // ---------- UDP receive thread: incoming audio -> resample -> playback channel ----------

    let recv_socket = socket.clone();
    {
        let output_config_cell = output_config_cell.clone();
        let shutdown = shutdown.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut output_buffer = Vec::<f32>::new();
            let mut output_resampler = resampler::ResamplerFft::new(
                1,
                resampler::SampleRate::Hz44100,
                output_config_cell
                    .lock()
                    .unwrap()
                    .sample_rate
                    .try_into()
                    .unwrap(),
            );
            let mut last_rate = output_config_cell.lock().unwrap().sample_rate;

            loop {
                if shutdown.load(Ordering::SeqCst) {
                    return;
                }

                let Ok(len) = recv_socket.recv(&mut buf) else {
                    continue;
                };

                let pcm = buf[..len]
                    .chunks_exact(4)
                    .map(|b| f32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                    .collect::<Vec<_>>();

                output_buffer.extend_from_slice(&pcm);

                let current_config = output_config_cell.lock().unwrap().clone();

                // If the output device's rate changed underneath us (rebuild
                // happened), rebuild the resampler to target the new rate too.
                if current_config.sample_rate != last_rate {
                    output_resampler = resampler::ResamplerFft::new(
                        1,
                        resampler::SampleRate::Hz44100,
                        match current_config.sample_rate.try_into() {
                            Ok(rate) => rate,
                            Err(_) => {
                                eprintln!("[vc] unsupported output sample rate, skipping rebuild");
                                last_rate = current_config.sample_rate;
                                continue;
                            }
                        },
                    );
                    last_rate = current_config.sample_rate;
                }

                let frame_size = output_resampler.chunk_size_input();

                while output_buffer.len() >= frame_size {
                    let input: Vec<f32> = output_buffer.drain(..frame_size).collect();

                    let mut pcm_out = vec![0.0f32; output_resampler.chunk_size_output()];

                    if let Err(e) = output_resampler.resample(&input, &mut pcm_out) {
                        eprintln!("[vc] failed to resample output: {e}");
                        continue;
                    }

                    let mono = if current_config.channels > 1 {
                        stereo_to_mono(&pcm_out)
                    } else {
                        pcm_out
                    };

                    // audio_rx/tx were built together, so this send only fails
                    // if every receiver was dropped — i.e. shutdown in progress.
                    let tx_result = {
                        let _rx_guard = audio_rx.lock().unwrap(); // keep receiver alive
                        audio_tx.send(mono)
                    };

                    if tx_result.is_err() {
                        return;
                    }
                }
            }
        });
    }

    input_stream.play().map_err(|e| e.to_string())?;

    *voice_state.session.lock().unwrap() = Some(VoiceSession {
        input_stream,
        output_stream,
        socket,
        pin,
        shutdown,
    });

    Ok(())
}

fn build_output_config(output_device: &cpal::Device) -> Result<cpal::StreamConfig, String> {
    Ok(output_device
        .default_output_config()
        .map_err(|e| e.to_string())?
        .into())
}

fn build_output_stream(
    output_device: &cpal::Device,
    output_config: &cpal::StreamConfig,
    audio_rx: Arc<Mutex<std::sync::mpsc::Receiver<Vec<f32>>>>,
    needs_rebuild: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    output_device
        .build_output_stream(
            *output_config,
            move |data: &mut [f32], _| {
                let rx = audio_rx.lock().unwrap();
                if let Ok(pcm) = rx.try_recv() {
                    for (out, sample) in data.iter_mut().zip(pcm.iter()) {
                        *out = *sample;
                    }
                    if pcm.len() < data.len() {
                        data[pcm.len()..].fill(0.0);
                    }
                } else {
                    data.fill(0.0);
                }
            },
            {
                let needs_rebuild = needs_rebuild.clone();
                move |err| {
                    eprintln!("[vc] output stream error: {err}");
                    let msg = err.to_string();
                    if msg.contains("sample rate changed") || msg.contains("DeviceNotAvailable") {
                        needs_rebuild.store(true, Ordering::SeqCst);
                    }
                }
            },
            None,
        )
        .map_err(|e| e.to_string())
}

fn stereo_to_mono(stereo_data: &[f32]) -> Vec<f32> {
    let mut mono_data = Vec::with_capacity(stereo_data.len() / 2);

    for chunk in stereo_data.chunks_exact(2) {
        let left = chunk[0];
        let right = chunk[1];
        let mono_sample = (left + right) / 2.0;
        mono_data.push(mono_sample);
    }

    mono_data
}
