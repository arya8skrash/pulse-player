use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use tauri::PhysicalSize;

use crate::{audio::AudioEngine, db::Db, queue::Queue};

/// Shared application state, registered with `app.manage(...)`.
pub struct AppState {
    pub db: Arc<Db>,
    pub queue: Arc<Mutex<Queue>>,
    pub audio: AudioEngine,
    /// Extracted album-art images live here (served to the UI via the asset protocol).
    pub art_dir: PathBuf,
    /// Mini-player mode, and the window size to restore when leaving it.
    pub mini: AtomicBool,
    pub saved_size: Mutex<Option<PhysicalSize<u32>>>,
}
