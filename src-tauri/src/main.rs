#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod db;
mod error;
mod lyrics;
mod queue;
mod scanner;
mod shell;
mod state;
mod tags;
mod online;

use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use crate::{audio::AudioEngine, db::Db, queue::Queue, state::AppState};

fn resolve_data_dir(app: &AppHandle) -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let portable = exe
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase().ends_with("-portable"))
            .unwrap_or(false);
        if portable {
            if let Some(dir) = exe.parent() {
                return dir.join("pulse-data");
            }
        }
    }
    app.path().app_data_dir().expect("no app data directory available")
}

fn close_to_tray_enabled(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|s| s.db.all_settings().ok())
        .and_then(|m| m.get("general.closeToTray").cloned())
        .map(|v| v == "true")
        .unwrap_or(false)
}

fn apply_installer_seed(app: &AppHandle) {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let seed_path = exe_dir.join("install-seed.json");
            if seed_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&seed_path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        let state = app.state::<AppState>();
                        if let Some(scheme) = json.get("scheme").and_then(|s| s.as_str()) {
                            let _ = state.db.set_setting("ui.scheme", &serde_json::to_string(scheme).unwrap_or_default());
                        }
                        if let Some(mode) = json.get("mode").and_then(|m| m.as_str()) {
                            let _ = state.db.set_setting("ui.mode", &serde_json::to_string(mode).unwrap_or_default());
                        }
                    }
                }
                let _ = std::fs::remove_file(&seed_path);
            }
        }
    }
}

fn extract_audio_file(args: &[String]) -> Option<String> {
    let extensions = [".mp3", ".flac", ".wav", ".ogg", ".m4a", ".aac"];
    for arg in args.iter().skip(1) { 
        let lower = arg.to_lowercase();
        if extensions.iter().any(|ext| lower.ends_with(ext)) {
            if std::path::Path::new(arg).is_file() {
                return Some(arg.clone());
            }
        }
    }
    None
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            shell::show_main(app);
            if let Some(path) = extract_audio_file(&args) {
                let _ = app.emit("play-file", path);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Debug).build())
        .setup(|app| {
            let data_dir = resolve_data_dir(app.handle());
            let art_dir = data_dir.join("art");
            std::fs::create_dir_all(&art_dir)?;

            app.asset_protocol_scope().allow_directory(&art_dir, true)?;

            let db = match Db::open(&data_dir.join("pulse.db")) {
                Ok(db) => Arc::new(db),
                Err(e) => {
                    log::error!("Failed to open database: {}", e);
                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Database error: {}", e))));
                }
            };
            
            let queue = Arc::new(Mutex::new(Queue::new()));
            let audio = AudioEngine::spawn(app.handle().clone(), db.clone(), queue.clone());
            
            app.manage(AppState {
                db,
                queue,
                audio,
                art_dir,
                mini: AtomicBool::new(false),
                saved_size: Mutex::new(None),
            });

            app.manage(commands::DiscordState {
                client: Mutex::new(None),
            });

            apply_installer_seed(app.handle());

            commands::apply_saved_settings(app.handle());
            commands::startup_scan(app.handle());
            shell::setup(app)?;

            let args: Vec<String> = std::env::args().collect();
            if let Some(path) = extract_audio_file(&args) {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    let _ = app_handle.emit("play-file", path);
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && close_to_tray_enabled(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.emit("ui:tray-state", true);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_tracks,
            commands::list_artists,
            commands::list_albums,
            commands::list_genres,
            commands::list_folders,
            commands::add_folder,
            commands::remove_folder,
            commands::set_folder_hidden,
            commands::rescan_all,
            commands::list_playlists,
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::set_rating,
            commands::set_favorite,
            commands::update_metadata,
            commands::get_lyrics,
            commands::play_queue,
            commands::toggle_playback,
            commands::pause_playback,
            commands::resume_playback,
            commands::stop_playback,
            commands::next_track,
            commands::prev_track,
            commands::seek_to,
            commands::set_volume,
            commands::set_shuffle,
            commands::set_repeat,
            commands::get_queue,
            commands::get_player_state,
            commands::list_audio_devices,
            commands::set_audio_device,
            commands::set_eq,
            commands::set_crossfade,
            commands::set_gapless,
            commands::get_settings,
            commands::set_setting,
            commands::set_mini_player,
            commands::set_always_on_top,
            commands::play_file_by_path,
            commands::init_rpc,
            commands::update_rpc,
            commands::clear_rpc,
            commands::set_virtualizer,
            commands::set_crystalizer,
            commands::set_enhancer,
            commands::set_replaygain,
            commands::analyze_library,
            online::online_search,
            online::online_resolve,
            online::online_play,
            online::online_set_proxy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Pulse");
}
