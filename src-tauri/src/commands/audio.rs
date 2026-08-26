use std::{
    net::UdpSocket,
    sync::{Arc, Mutex},
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
    pub output_stream: cpal::Stream,
    pub socket: Arc<UdpSocket>,
    pub pin: u64,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
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

    let input_socket = socket.clone();
    let mut input_config: cpal::StreamConfig = input_device
        .default_input_config()
        .map_err(|e| e.to_string())?
        .into();

    input_config.channels = 1;

    let mut input_resampler = resampler::ResamplerFft::new(
        1,
        input_config
            .sample_rate
            .try_into()
            .map_err(|e| format!("{e:?}"))?,
        resampler::SampleRate::Hz44100,
    );

    let mut input_buffer = Vec::<f32>::new();

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                input_buffer.extend_from_slice(data);

                while input_buffer.len() >= 1024 {
                    let input: Vec<f32> = input_buffer.drain(..1024).collect();

                    let output_len =
                        ((1024.0 * 44100.0 / input_config.sample_rate as f32).ceil()) as usize;

                    let mut output = vec![0.0f32; output_len];

                    if let Err(e) = input_resampler.resample(&input, &mut output) {
                        eprintln!("[vc] failed to resample input: {e}");
                        continue;
                    }

                    let mut packet = Vec::with_capacity(output.len() * 4);

                    for sample in output {
                        packet.extend_from_slice(&sample.to_be_bytes());
                    }

                    let _ = input_socket.send(&packet);
                }
            },
            |err| eprintln!("[vc] input stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    let output_socket = socket.clone();
    let output_config: cpal::StreamConfig = output_device
        .default_output_config()
        .map_err(|e| e.to_string())?
        .into();
    let mut output_resampler = resampler::ResamplerFft::new(
        1,
        resampler::SampleRate::Hz44100,
        output_config
            .sample_rate
            .try_into()
            .map_err(|e| format!("{e:?}"))?,
    );

    // shared ring buffer between the UDP-receiving task and the playback callback
    let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<f32>>();

    let output_stream = output_device
        .build_output_stream(
            output_config.clone(),
            move |data: &mut [f32], _| {
                if let Ok(pcm) = audio_rx.try_recv() {
                    for (out, sample) in data.iter_mut().zip(pcm.iter()) {
                        *out = *sample;
                    }

                    // If CPAL asks for more samples than we received,
                    // fill the remainder with silence.
                    if pcm.len() < data.len() {
                        data[pcm.len()..].fill(0.0);
                    }
                } else {
                    data.fill(0.0);
                }
            },
            |err| eprintln!("[vc] output stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    // background thread reading incoming UDP audio, feeding the playback channel
    let recv_socket = output_socket.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        let mut output_buffer = Vec::<f32>::new();

        loop {
            if let Ok(len) = recv_socket.recv(&mut buf) {
                let pcm = buf[..len]
                    .chunks_exact(4)
                    .map(|b| f32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                    .collect::<Vec<_>>();

                output_buffer.extend_from_slice(&pcm);

                while output_buffer.len() >= 1024 {
                    let input: Vec<f32> = output_buffer.drain(..1024).collect();

                    let output_len =
                        ((1024.0 * output_config.sample_rate as f32 / 44100.0).ceil()) as usize;

                    let mut pcm_out = vec![0.0f32; output_len];

                    if let Err(e) = output_resampler.resample(&input, &mut pcm_out) {
                        eprintln!("[vc] failed to resample output: {e}");
                        continue;
                    }

                    if audio_tx
                        .send(if output_config.channels > 1 {
                            stereo_to_mono(&pcm_out)
                        } else {
                            pcm_out
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }
    });

    input_stream.play().map_err(|e| e.to_string())?;
    output_stream.play().map_err(|e| e.to_string())?;

    *voice_state.session.lock().unwrap() = Some(VoiceSession {
        input_stream,
        output_stream,
        socket,
        pin,
    });

    Ok(())
}

#[tauri::command]
pub fn disconnect_from_vc(voice_state: State<VoiceState>) -> Result<(), String> {
    *voice_state.session.lock().unwrap() = None; // dropping stops both cpal streams
    Ok(())
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
