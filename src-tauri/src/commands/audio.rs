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
    let input_config = input_device
        .default_input_config()
        .map_err(|e| e.to_string())?
        .into();

    let input_stream = input_device
        .build_input_stream(
            input_config,
            move |data: &[f32], _| {
                let pcm: Vec<i16> = data
                    .iter()
                    .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                    .collect();

                let mut packet = Vec::with_capacity(pcm.len() * 2);
                for sample in &pcm {
                    packet.extend_from_slice(&sample.to_be_bytes());
                }

                eprintln!(
                    "[vc] mic captured {} samples ({} bytes)",
                    pcm.len(),
                    packet.len()
                );
                let _ = input_socket.send(&packet);
            },
            |err| eprintln!("[vc] input stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;

    let output_socket = socket.clone();
    let output_config = output_device
        .default_output_config()
        .map_err(|e| e.to_string())?
        .into();

    // shared ring buffer between the UDP-receiving task and the playback callback
    let (audio_tx, audio_rx) = std::sync::mpsc::channel::<Vec<i16>>();

    let output_stream = output_device
        .build_output_stream(
            output_config,
            move |data: &mut [f32], _| {
                if let Ok(pcm) = audio_rx.try_recv() {
                    eprintln!("[vc] playing {} samples", pcm.len());
                    for (out, sample) in data.iter_mut().zip(pcm.iter()) {
                        *out = *sample as f32 / i16::MAX as f32;
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
        loop {
            if let Ok(len) = recv_socket.recv(&mut buf) {
                let pcm: Vec<i16> = buf[..len]
                    .chunks_exact(2)
                    .map(|b| i16::from_be_bytes([b[0], b[1]]))
                    .collect();
                let _ = audio_tx.send(pcm);
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
