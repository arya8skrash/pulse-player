//! Audio engine.
//!
//! rodio's `OutputStream` is not `Send`, so it lives on one dedicated thread.

use std::{
    fs::File,
    io::BufReader,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

use rodio::{Decoder, Sink};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub enum AudioCmd {
    Play { track_id: i64, path: String, duration_ms: u64 },
    Pause,
    Resume,
    Stop,
    Seek(u64),
    SetVolume(f32),
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Tick {
    track_id: i64,
    position_ms: u64,
    duration_ms: u64,
    playing: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Ended {
    track_id: i64,
}

struct Current {
    sink: Sink,
    track_id: i64,
    duration_ms: u64,
}

impl Current {
    fn tick(&self) -> Tick {
        Tick {
            track_id: self.track_id,
            position_ms: self.sink.get_pos().as_millis() as u64,
            duration_ms: self.duration_ms,
            playing: !self.sink.is_paused(),
        }
    }
}

#[derive(Clone)]
pub struct AudioEngine {
    tx: Sender<AudioCmd>,
}

impl AudioEngine {
    pub fn spawn(app: AppHandle) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("pulse-audio".into())
            .spawn(move || run(app, rx))
            .expect("failed to spawn audio thread");
        AudioEngine { tx }
    }

    pub fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }
}

fn open_sink(mixer: &rodio::Mixer, path: &str, volume: f32) -> Result<Sink, String> {
    let file = File::open(path).map_err(|e| format!("cannot open {path}: {e}"))?;
    let source = Decoder::new(BufReader::new(file)).map_err(|e| format!("cannot decode {path}: {e}"))?;
    let sink = Sink::try_new(mixer).map_err(|e| format!("audio device error: {e}"))?;
    sink.set_volume(volume);
    sink.append(source);
    Ok(sink)
}

fn run(app: AppHandle, rx: Receiver<AudioCmd>) {
    let host = rodio::cpal::default_host();
    let device = match host.default_output_device() {
        Some(d) => d,
        None => {
            log::error!("no audio output device found");
            let _ = app.emit("player:error", "No audio output device found");
            return;
        }
    };
    
    let device_name = device.name().unwrap_or_default();
    let is_razer = device_name.to_lowercase().contains("razer") || device_name.to_lowercase().contains("7.1");

    let mut builder = rodio::OutputStreamBuilder::new();
    
    if is_razer {
        log::info!("Razer 7.1 detected! Forcing 48000Hz, 16-bit to prevent glitches.");
        builder = builder
            .sample_rate(48000)
            .sample_format(rodio::cpal::SampleFormat::I16)
            .channels(2);
    }

    let stream = match builder.open_stream_or_default(&device) {
        Ok(s) => s,
        Err(e) => {
            log::error!("cannot open audio stream: {e}");
            let _ = app.emit("player:error", format!("Cannot open audio stream: {e}"));
            return;
        }
    };
    
    let handle = stream.mixer();

    let mut volume: f32 = 1.0;
    let mut current: Option<Current> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(cmd) => {
                match cmd {
                    AudioCmd::Play { track_id, path, duration_ms } => {
                        if let Some(old) = current.take() {
                            old.sink.stop();
                        }
                        match open_sink(handle, &path, volume) {
                            Ok(sink) => current = Some(Current { sink, track_id, duration_ms }),
                            Err(e) => {
                                log::error!("{e}");
                                let _ = app.emit("player:error", e);
                            }
                        }
                    }
                    AudioCmd::Pause => {
                        if let Some(c) = &current {
                            c.sink.pause();
                        }
                    }
                    AudioCmd::Resume => {
                        if let Some(c) = &current {
                            c.sink.play();
                        }
                    }
                    AudioCmd::Stop => {
                        if let Some(c) = current.take() {
                            c.sink.stop();
                        }
                        let _ = app.emit("player:stopped", ());
                    }
                    AudioCmd::Seek(ms) => {
                        if let Some(c) = &current {
                            if let Err(e) = c.sink.try_seek(Duration::from_millis(ms)) {
                                log::warn!("seek failed: {e}");
                            }
                        }
                    }
                    AudioCmd::SetVolume(v) => {
                        volume = v.clamp(0.0, 1.0);
                        if let Some(c) = &current {
                            c.sink.set_volume(volume);
                        }
                    }
                }
                if let Some(c) = &current {
                    let _ = app.emit("player:tick", c.tick());
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let status = current.as_ref().map(|c| (c.sink.empty(), c.track_id));
        match status {
            Some((true, track_id)) => {
                current = None;
                let _ = app.emit("player:ended", Ended { track_id });
            }
            Some((false, _)) => {
                if let Some(c) = &current {
                    let _ = app.emit("player:tick", c.tick());
                }
            }
            None => {}
        }
    }
}