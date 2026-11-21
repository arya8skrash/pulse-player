use std::{collections::HashMap, path::{Path, PathBuf}};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::audio::SampleBuffer;

use crate::{
    audio::{self, AudioCmd, EQ_BANDS, analyzer::ReplayGainAnalyzer},
    db::{AlbumRow, ArtistRow, Folder, GenreRow, Playlist, Track, TrackQuery},
    error::{AppError, AppResult},
    lyrics::{self, Lyrics},
    queue::{QueueState, RepeatMode},
    scanner, shell, tags,
    state::AppState,
};

// ================================================================== library

#[tauri::command(async)]
pub fn get_tracks(state: State<AppState>, q: TrackQuery) -> AppResult<Vec<Track>> {
    state.db.query_tracks(&q)
}

#[tauri::command(async)]
pub fn list_artists(state: State<AppState>) -> AppResult<Vec<ArtistRow>> {
    state.db.list_artists()
}

#[tauri::command(async)]
pub fn list_albums(state: State<AppState>) -> AppResult<Vec<AlbumRow>> {
    state.db.list_albums()
}

#[tauri::command(async)]
pub fn list_genres(state: State<AppState>) -> AppResult<Vec<GenreRow>> {
    state.db.list_genres()
}

#[tauri::command(async)]
pub fn list_folders(state: State<AppState>) -> AppResult<Vec<Folder>> {
    state.db.list_folders()
}

#[tauri::command(async)]
pub fn add_folder(app: AppHandle, state: State<AppState>, path: String) -> AppResult<Folder> {
    if !PathBuf::from(&path).is_dir() {
        return Err(AppError::Msg(format!("{path} is not a folder")));
    }
    let folder = state.db.add_folder(&path)?;
    spawn_scan(&app, folder.id, path);
    Ok(folder)
}

#[tauri::command(async)]
pub fn remove_folder(state: State<AppState>, id: i64) -> AppResult<()> {
    state.db.remove_folder(id)
}

#[tauri::command(async)]
pub fn set_folder_hidden(state: State<AppState>, id: i64, hidden: bool) -> AppResult<()> {
    state.db.set_folder_hidden(id, hidden)
}

#[tauri::command(async)]
pub fn rescan_all(app: AppHandle) -> AppResult<()> {
    startup_scan(&app);
    Ok(())
}

pub fn startup_scan(app: &AppHandle) {
    let state = app.state::<AppState>();
    match state.db.list_folders() {
        Ok(folders) => {
            for f in folders.into_iter().filter(|f| f.watch) {
                spawn_scan(app, f.id, f.path);
            }
        }
        Err(e) => log::error!("could not list folders for startup scan: {e}"),
    }
}

fn spawn_scan(app: &AppHandle, folder_id: i64, path: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        match scanner::scan_folder(&app, &state.db, &state.art_dir, folder_id, Path::new(&path)) {
            Ok(s) => log::info!("scan of {path} finished: {s:?}"),
            Err(e) => log::error!("scan of {path} failed: {e}"),
        }
    });
}

// ================================================================== playlists

#[tauri::command(async)]
pub fn list_playlists(state: State<AppState>) -> AppResult<Vec<Playlist>> {
    state.db.list_playlists()
}

#[tauri::command(async)]
pub fn create_playlist(state: State<AppState>, name: String) -> AppResult<Playlist> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Msg("Give the playlist a name".into()));
    }
    state.db.create_playlist(name)
}

#[tauri::command(async)]
pub fn rename_playlist(state: State<AppState>, id: i64, name: String) -> AppResult<()> {
    state.db.rename_playlist(id, name.trim())
}

#[tauri::command(async)]
pub fn delete_playlist(state: State<AppState>, id: i64) -> AppResult<()> {
    state.db.delete_playlist(id)
}

#[tauri::command(async)]
pub fn add_to_playlist(state: State<AppState>, playlist_id: i64, track_ids: Vec<i64>) -> AppResult<()> {
    state.db.add_to_playlist(playlist_id, &track_ids)
}

#[tauri::command(async)]
pub fn remove_from_playlist(state: State<AppState>, playlist_id: i64, track_id: i64) -> AppResult<()> {
    state.db.remove_from_playlist(playlist_id, track_id)
}

// ================================================================== track details

#[tauri::command(async)]
pub fn set_rating(state: State<AppState>, id: i64, rating: i64) -> AppResult<()> {
    state.db.set_rating(id, rating)
}

#[tauri::command(async)]
pub fn set_favorite(state: State<AppState>, id: i64, favorite: bool) -> AppResult<()> {
    state.db.set_favorite(id, favorite)
}

#[tauri::command(async)]
pub fn update_metadata(state: State<AppState>, id: i64, title: String, artist: String, album: String) -> AppResult<Track> {
    let track = state.db.get_track(id)?;
    let title = title.trim();
    let title = if title.is_empty() { track.title.as_str() } else { title };
    let (artist, album) = (artist.trim(), album.trim());
    let (size, mtime) = tags::write_basic_tags(Path::new(&track.path), title, artist, album)?;
    state.db.update_metadata(id, title, artist, album, size, mtime)?;
    state.db.get_track(id)
}

#[tauri::command(async)]
pub fn get_lyrics(state: State<AppState>, id: i64) -> AppResult<Option<Lyrics>> {
    let track = state.db.get_track(id)?;
    Ok(lyrics::load_for(Path::new(&track.path)))
}

// ================================================================== replaygain

#[tauri::command]
pub async fn analyze_library(app: AppHandle) -> AppResult<()> {
    let app_handle = app.clone();
    std::thread::spawn(move || {
        let state = app_handle.state::<AppState>();
        let tracks = match state.db.get_unanalyzed_tracks() {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to query tracks for analysis: {e}");
                return;
            }
        };

        let total = tracks.len();
        if total == 0 {
            let _ = app_handle.emit("replaygain:done", ());
            return;
        }

        let counter = AtomicUsize::new(0);

        tracks.par_iter().for_each(|track| {
            let path = Path::new(&track.path);
            if let Ok(file) = std::fs::File::open(path) {
                let mss = MediaSourceStream::new(Box::new(file), Default::default());
                let hint = Hint::new();
                
                let probed = symphonia::default::get_probe().format(
                    &hint,
                    mss,
                    &FormatOptions::default(),
                    &MetadataOptions::default(),
                );

                if let Ok(probed_result) = probed {
                    let mut format = probed_result.format;
                    if let Some(track_info) = format.default_track() {
                        let track_id = track_info.id;
                        if let Ok(mut decoder) = crate::audio::codec_registry().make(
                            &track_info.codec_params,
                            &DecoderOptions::default(),
                        ) {
                            let sample_rate = track_info.codec_params.sample_rate.unwrap_or(44100);
                            let channels = track_info.codec_params.channels.map(|c| c.count()).unwrap_or(2);
                            
                            let mut analyzer = ReplayGainAnalyzer::new(sample_rate as f32, channels);
                            let mut sample_buf = None;

                            loop {
                                match format.next_packet() {
                                    Ok(packet) => {
                                        if packet.track_id() != track_id {
                                            continue;
                                        }
                                        match decoder.decode(&packet) {
                                            Ok(decoded) => {
                                                let spec = *decoded.spec();
                                                let duration = decoded.capacity() as u64;
                                                
                                                if sample_buf.is_none() {
                                                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                                                }
                                                
                                                if let Some(buf) = &mut sample_buf {
                                                    buf.copy_interleaved_ref(decoded);
                                                    analyzer.push_frames(buf.samples());
                                                }
                                            }
                                            Err(SymphoniaError::DecodeError(_)) => continue,
                                            Err(_) => break,
                                        }
                                    }
                                    Err(SymphoniaError::IoError(_)) => break,
                                    Err(_) => break,
                                }
                            }

                            let gain = analyzer.get_gain();
                            let peak = analyzer.get_peak();
                            
                            if let Err(e) = state.db.set_track_replaygain(track.id, gain, peak) {
                                log::error!("Failed to save replaygain for {}: {}", track.title, e);
                            }
                        }
                    }
                }
            }

            let current = counter.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = app_handle.emit("replaygain:progress", serde_json::json!({
                "current": current,
                "total": total,
                "name": track.title
            }));
        });

        let _ = app_handle.emit("replaygain:done", ());
    });
    Ok(())
}

// ================================================================== playback

#[tauri::command(async)]
pub fn play_queue(state: State<AppState>, track_ids: Vec<i64>, start_index: usize) -> AppResult<()> {
    let cursor = {
        let mut q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.set(track_ids, start_index);
        q.cursor()
    };
    if let Some(c) = cursor {
        state.audio.send(AudioCmd::PlayAt { cursor: c, offset_ms: 0 });
    }
    Ok(())
}

#[tauri::command(async)]
pub fn play_file_by_path(state: State<AppState>, path: String) -> AppResult<()> {
    let tracks = state.db.query_tracks(&TrackQuery { limit: Some(20000), ..Default::default() })?;
    
    if let Some(track) = tracks.into_iter().find(|t| t.path == path) {
        let mut q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.set(vec![track.id], 0);
        if let Some(c) = q.cursor() {
            state.audio.send(AudioCmd::PlayAt { cursor: c, offset_ms: 0 });
        }
    } else {
        state.audio.send(AudioCmd::PlayDirect { path, offset_ms: 0 });
    }
    Ok(())
}

#[tauri::command(async)]
pub fn toggle_playback(state: State<AppState>) {
    state.audio.send(AudioCmd::Toggle);
}

#[tauri::command(async)]
pub fn pause_playback(state: State<AppState>) {
    state.audio.send(AudioCmd::Pause);
}

#[tauri::command(async)]
pub fn resume_playback(state: State<AppState>) {
    state.audio.send(AudioCmd::Resume);
}

#[tauri::command(async)]
pub fn stop_playback(state: State<AppState>) {
    state.audio.send(AudioCmd::Stop);
}

#[tauri::command(async)]
pub fn next_track(state: State<AppState>) {
    state.audio.send(AudioCmd::Next);
}

#[tauri::command(async)]
pub fn prev_track(state: State<AppState>) {
    state.audio.send(AudioCmd::Prev);
}

#[tauri::command(async)]
pub fn seek_to(state: State<AppState>, position_ms: u64) {
    state.audio.send(AudioCmd::Seek(position_ms));
}

#[tauri::command(async)]
pub fn set_volume(state: State<AppState>, volume: f32) {
    state.audio.send(AudioCmd::SetVolume(volume));
}

#[tauri::command(async)]
pub fn set_shuffle(state: State<AppState>, on: bool) -> AppResult<()> {
    state.queue.lock().unwrap_or_else(|e| e.into_inner()).set_shuffle(on);
    state.db.set_setting("player.shuffle", &on.to_string())
}

#[tauri::command(async)]
pub fn set_repeat(state: State<AppState>, mode: String) -> AppResult<()> {
    let mode = RepeatMode::parse(&mode);
    state.queue.lock().unwrap_or_else(|e| e.into_inner()).repeat = mode;
    state.db.set_setting("player.repeat", &Value::String(mode.as_str().into()).to_string())
}

#[tauri::command(async)]
pub fn get_queue(state: State<AppState>) -> QueueState {
    state.queue.lock().unwrap_or_else(|e| e.into_inner()).state()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    shuffle: bool,
    repeat: RepeatMode,
    volume: f64,
}

#[tauri::command(async)]
pub fn get_player_state(state: State<AppState>) -> AppResult<PlayerState> {
    let settings = state.db.all_settings()?;
    let volume = settings
        .get("player.volume")
        .and_then(|v| serde_json::from_str::<f64>(v).ok())
        .unwrap_or(0.8);
    let q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
    Ok(PlayerState { shuffle: q.shuffle, repeat: q.repeat, volume })
}

#[tauri::command(async)]
pub fn set_replaygain(state: State<AppState>, enabled: bool) -> AppResult<()> {
    state.audio.send(AudioCmd::SetReplayGain { enabled });
    state.db.set_setting("audio.replaygain", &enabled.to_string())
}

// ================================================================== audio settings

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevices {
    devices: Vec<String>,
    default: Option<String>,
    current: Option<String>,
}

#[tauri::command]
pub async fn list_audio_devices(state: State<'_, AppState>) -> AppResult<AudioDevices> {
    let (devices, default) = tauri::async_runtime::spawn_blocking(audio::list_devices)
        .await
        .map_err(|e| AppError::Msg(e.to_string()))?;
    Ok(AudioDevices { devices, default, current: state.audio.current_device() })
}

#[tauri::command(async)]
pub fn set_audio_device(state: State<AppState>, name: Option<String>) -> AppResult<()> {
    state.audio.send(AudioCmd::SetDevice(name.clone()));
    state.db.set_setting("audio.device", &serde_json::to_string(&name).unwrap_or_else(|_| "null".into()))
}

fn gains_array(gains: &[f32]) -> [f32; EQ_BANDS] {
    let mut out = [0.0; EQ_BANDS];
    for (o, g) in out.iter_mut().zip(gains.iter()) {
        *o = g.clamp(-12.0, 12.0);
    }
    out
}

#[tauri::command(async)]
pub fn set_eq(state: State<AppState>, enabled: bool, gains: Vec<f32>) -> AppResult<()> {
    let arr = gains_array(&gains);
    state.audio.send(AudioCmd::SetEq { enabled, gains: arr });
    let json = serde_json::json!({ "enabled": enabled, "gains": arr });
    state.db.set_setting("audio.eq", &json.to_string())
}

#[tauri::command(async)]
pub fn set_crossfade(state: State<AppState>, ms: u32) -> AppResult<()> {
    state.audio.send(AudioCmd::SetCrossfade(ms));
    state.db.set_setting("audio.crossfade", &ms.to_string())
}

#[tauri::command(async)]
pub fn set_gapless(state: State<AppState>, on: bool) -> AppResult<()> {
    state.audio.send(AudioCmd::SetGapless(on));
    state.db.set_setting("audio.gapless", &on.to_string())
}

#[tauri::command(async)]
pub fn set_virtualizer(state: State<AppState>, enabled: bool, profile_name: Option<String>) -> AppResult<()> {
    state.audio.send(AudioCmd::SetVirtualizer { enabled, profile_name: profile_name.clone() });
    let json = serde_json::json!({ "enabled": enabled, "profile": profile_name });
    state.db.set_setting("audio.virtualizer", &json.to_string())
}

#[tauri::command(async)]
pub fn set_crystalizer(state: State<AppState>, mode: String, intensity: f32) -> AppResult<()> {
    state.audio.send(AudioCmd::SetCrystalizer { mode: mode.clone(), intensity });
    let json = serde_json::json!({ "mode": mode, "intensity": intensity });
    state.db.set_setting("audio.crystalizer", &json.to_string())
}

#[tauri::command(async)]
pub fn set_enhancer(
    state: State<AppState>,
    bass: f32,
    treble: f32,
    width: f32,
    surround: Option<bool>,
    booster: Option<bool>,
) -> AppResult<()> {
    if !bass.is_finite() || !treble.is_finite() || !width.is_finite() {
        return Err(AppError::Msg("Enhancer values must be finite".into()));
    }

    let bass = bass.clamp(0.0, 1.0);
    let treble = treble.clamp(0.0, 1.0);
    let width = width.clamp(0.0, 1.0);
    let surround = surround.unwrap_or(false);
    let booster = booster.unwrap_or(false);

    state.db.set_enhancer_settings(bass, treble, width, surround, booster)?;
    state.audio.send(AudioCmd::SetEnhancer {
        bass,
        treble,
        width,
        surround,
        booster,
    });
    Ok(())
}

pub fn apply_saved_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let saved = state.db.all_settings().unwrap_or_default();
    let get = |k: &str| saved.get(k).and_then(|v| serde_json::from_str::<Value>(v).ok());

    if let Some(Value::String(dev)) = get("audio.device") {
        state.audio.send(AudioCmd::SetDevice(Some(dev)));
    }

    if let Some(v) = get("player.volume").and_then(|v| v.as_f64()) {
        state.audio.send(AudioCmd::SetVolume(v as f32));
    }
    if let Some(ms) = get("audio.crossfade").and_then(|v| v.as_u64()) {
        state.audio.send(AudioCmd::SetCrossfade(ms as u32));
    }
    if let Some(on) = get("audio.gapless").and_then(|v| v.as_bool()) {
        state.audio.send(AudioCmd::SetGapless(on));
    }
    if let Some(eq) = get("audio.eq") {
        let enabled = eq.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        let gains: Vec<f32> = eq
            .get("gains")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect())
            .unwrap_or_default();
        state.audio.send(AudioCmd::SetEq { enabled, gains: gains_array(&gains) });
    }

    if let Some(virt) = get("audio.virtualizer") {
        let enabled = virt.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        let profile_name = virt.get("profile").and_then(|v| v.as_str()).map(|s| s.to_string());
        state.audio.send(AudioCmd::SetVirtualizer { enabled, profile_name });
    }
    if let Some(cry) = get("audio.crystalizer") {
        let mode = cry.get("mode").and_then(|v| v.as_str()).unwrap_or("crystalizer").to_string();
        let intensity = cry.get("intensity").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        state.audio.send(AudioCmd::SetCrystalizer { mode, intensity });
    }
    if let Some(enh) = get("audio.enhancer") {
        let amount = |key: &str| {
            enh.get(key)
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0) as f32
        };
        state.audio.send(AudioCmd::SetEnhancer {
            bass: amount("bass"),
            treble: amount("treble"),
            width: amount("width"),
            surround: enh.get("surround").and_then(Value::as_bool).unwrap_or(false),
            booster: enh.get("booster").and_then(Value::as_bool).unwrap_or(false),
        });
    }
    if let Some(on) = get("audio.replaygain").and_then(|v| v.as_bool()) {
        state.audio.send(AudioCmd::SetReplayGain { enabled: on });
    }

    let mut q = state.queue.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(on) = get("player.shuffle").and_then(|v| v.as_bool()) {
        q.shuffle = on;
    }
    if let Some(Value::String(r)) = get("player.repeat") {
        q.repeat = RepeatMode::parse(&r);
    }
}

// ================================================================== settings & window

#[tauri::command(async)]
pub fn get_settings(state: State<AppState>) -> AppResult<HashMap<String, Value>> {
    Ok(state
        .db
        .all_settings()?
        .into_iter()
        .map(|(k, v)| {
            let parsed = serde_json::from_str(&v).unwrap_or(Value::String(v));
            (k, parsed)
        })
        .collect())
}

#[tauri::command(async)]
pub fn set_setting(state: State<AppState>, key: String, value: Value) -> AppResult<()> {
    state.db.set_setting(&key, &value.to_string())
}

#[tauri::command]
pub fn set_mini_player(app: AppHandle, on: bool) -> AppResult<()> {
    shell::set_mini(&app, on)?;
    Ok(())
}

#[tauri::command]
pub fn set_always_on_top(app: AppHandle, on: bool) -> AppResult<()> {
    if let Some(w) = app.get_webview_window("main") {
        w.set_always_on_top(on)?;
    }
    Ok(())
}

// ================================================================== Discord Rich Presence

pub struct DiscordState {
    pub client: Mutex<Option<DiscordIpcClient>>,
}

#[tauri::command]
pub fn init_rpc(state: State<'_, DiscordState>) -> Result<(), String> {
    let mut client = DiscordIpcClient::new("1571135865030320178").map_err(|e| e.to_string())?;

    if client.connect().is_ok() {
        *state.client.lock().unwrap() = Some(client);
        Ok(())
    } else {
        Err("Failed to connect to Discord. Is it open and logged in?".into())
    }
}

#[tauri::command]
pub fn update_rpc(
    state: State<'_, DiscordState>,
    title: String,
    artist: String,
    is_playing: bool,
    current_time: i64,
    _duration: i64,
) -> Result<(), String> {
    if let Some(client) = &mut *state.client.lock().unwrap() {
        let artist_str = format!("by {}", artist);
        let title_str; 

        let mut act = activity::Activity::new()
            .activity_type(activity::ActivityType::Listening)
            .state(&artist_str)
            .assets(
                activity::Assets::new()
                    .large_image("pulse_logo")
                    .large_text("Pulse Music Player")
            );

        if is_playing {
            title_str = format!("▶ {}", title);
            act = act.details(&title_str);
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            
            let start_time = now - current_time;
            
            act = act.timestamps(
                activity::Timestamps::new()
                    .start(start_time)
            );
        } else {
            title_str = format!("⏸ {}", title);
            act = act.details(&title_str);
            
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
                
            act = act.timestamps(
                activity::Timestamps::new()
                    .start(now)
                    .end(now)
            );
        }

        client.set_activity(act).map_err(|e| e.to_string())?;
    } else {
        return Err("Cannot update: Client is not initialized in state!".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn clear_rpc(state: State<'_, DiscordState>) -> Result<(), String> {
    if let Some(client) = &mut *state.client.lock().unwrap() {
        let _ = client.clear_activity();
    }
    Ok(())
}
