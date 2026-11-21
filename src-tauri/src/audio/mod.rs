mod decoder;
mod eq;
mod output;
mod pipeline;
mod resample;
mod ring;
mod virtualizer; 
pub mod crystalizer; 
pub mod enhancer;
pub mod replaygain;
pub mod analyzer;

pub use output::list_devices;
pub(crate) use decoder::codec_registry;
//pub use virtualizer::{find_profile, HeadphoneProfile, Virtualizer}; 

use std::{
    sync::{
        mpsc::{self, Sender},
        Arc, Mutex,
    },
    thread,
};

use tauri::AppHandle;

use crate::{db::{Db, Track}, queue::Queue};

pub const EQ_BANDS: usize = eq::BANDS;

pub enum AudioCmd {
    PlayAt { cursor: usize, offset_ms: u64 },
    PlayDirect { path: String, offset_ms: u64 },
    PlayDirectTrack { track: Track, offset_ms: u64 },
    Pause,
    Resume,
    Toggle,
    Stop,
    Seek(u64),
    Next,
    Prev,
    SetVolume(f32),
    SetDevice(Option<String>),
    SetEq { enabled: bool, gains: [f32; EQ_BANDS] },
    SetCrossfade(u32),
    SetGapless(bool),
    SetVirtualizer { enabled: bool, profile_name: Option<String> },
    SetCrystalizer { mode: String, intensity: f32 },
    SetEnhancer { bass: f32, treble: f32, width: f32, surround: bool, booster: bool, },
    SetReplayGain { enabled: bool },
}

#[derive(Clone)]
pub struct AudioEngine {
    tx: Sender<AudioCmd>,
    device: Arc<Mutex<Option<String>>>,
}

impl AudioEngine {
    pub fn spawn(app: AppHandle, db: Arc<Db>, queue: Arc<Mutex<Queue>>) -> Self {
        let (tx, rx) = mpsc::channel();
        let device = Arc::new(Mutex::new(None));
        let dev = device.clone();
        thread::Builder::new()
            .name("pulse-audio".into())
            .spawn(move || pipeline::run(app, db, queue, rx, dev))
            .expect("failed to spawn audio thread");
        AudioEngine { tx, device }
    }

    pub fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn current_device(&self) -> Option<String> {
        self.device.lock().ok().and_then(|g| g.clone())
    }
}
