use std::{
    borrow::Cow,
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use lofty::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

use crate::{
    db::{Db, NewTrack},
    error::{AppError, AppResult},
};

pub const EXTENSIONS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac"];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub folder: String,
    pub scanned: usize,
    pub total: usize,
    pub added: usize,
    pub current: String,
    pub done: bool,
}

#[derive(Default, Debug)]
pub struct ScanSummary {
    pub added_or_updated: usize,
    pub removed: usize,
    pub failed: usize,
}

struct Found {
    path: PathBuf,
    size: i64,
    mtime: i64,
}

fn is_audio(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn scan_folder(app: &AppHandle, db: &Db, art_dir: &Path, folder_id: i64, folder: &Path) -> AppResult<ScanSummary> {
    let folder_label = folder.to_string_lossy().to_string();

    let mut found: Vec<Found> = Vec::new();
    for entry in WalkDir::new(folder).follow_links(false).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || !is_audio(entry.path()) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        found.push(Found { path: entry.path().to_path_buf(), size: meta.len() as i64, mtime });
    }

    let known = db.track_fingerprints(folder_id)?;
    let total = found.len();
    let mut summary = ScanSummary::default();
    let mut seen: HashSet<String> = HashSet::with_capacity(total);

    let mut batch_tracks: Vec<NewTrack> = Vec::with_capacity(50);

    for (i, f) in found.iter().enumerate() {
        let path_str = f.path.to_string_lossy().to_string();
        seen.insert(path_str.clone());

        if known.get(&path_str) != Some(&(f.size, f.mtime)) {
            match parse_file(f, folder_id, art_dir) {
                Ok(track) => {
                    batch_tracks.push(track);
                    summary.added_or_updated += 1;

                    if batch_tracks.len() >= 50 {
                        let _ = db.upsert_tracks_batch(&batch_tracks);
                        batch_tracks.clear();
                    }
                }
                Err(e) => {
                    log::warn!("skipping {path_str}: {e}");
                    summary.failed += 1;
                }
            }
        }

        if i % 50 == 0 || i + 1 == total {
            let _ = app.emit(
                "scan:progress",
                ScanProgress {
                    folder: folder_label.clone(),
                    scanned: i + 1,
                    total,
                    added: summary.added_or_updated,
                    current: f.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                    done: false,
                },
            );
        }
    }

    if !batch_tracks.is_empty() {
        let _ = db.upsert_tracks_batch(&batch_tracks);
    }

    let missing: Vec<String> = known.keys().filter(|p| !seen.contains(*p)).cloned().collect();
    summary.removed = missing.len();
    if !missing.is_empty() {
        db.delete_tracks_by_path(&missing)?;
    }

    let _ = app.emit(
        "scan:progress",
        ScanProgress { folder: folder_label, scanned: total, total, added: summary.added_or_updated, current: String::new(), done: true },
    );
    Ok(summary)
}

fn clean(s: Option<Cow<'_, str>>) -> String {
    s.map(|c| c.trim().to_string()).unwrap_or_default()
}

fn parse_file(f: &Found, folder_id: i64, art_dir: &Path) -> AppResult<NewTrack> {
    let tagged = lofty::read_from_path(&f.path)
        .map_err(|e| AppError::Msg(format!("{}: {e}", f.path.display())))?;
    let props = tagged.properties();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let filename = f.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let stem = f.path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| filename.clone());

    let title = {
        let t = clean(tag.and_then(|t| t.title()));
        if t.is_empty() { stem } else { t }
    };
    let artist = clean(tag.and_then(|t| t.artist()));
    let album = clean(tag.and_then(|t| t.album()));
    let genre = clean(tag.and_then(|t| t.genre()));
    let album_artist = tag
        .and_then(|t| t.get_string(&ItemKey::AlbumArtist))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let track_no = tag.and_then(|t| t.track()).map(|n| n as i64);
    let year = tag
        .and_then(|t| t.get_string(&ItemKey::Year).or_else(|| t.get_string(&ItemKey::RecordingDate)))
        .and_then(|s| s.get(..4))
        .and_then(|s| s.parse::<i64>().ok());

    let art_path = tag
        .and_then(|t| t.pictures().first())
        .and_then(|pic| save_art(art_dir, pic.data()).ok());

    let track_gain_db = tag
        .and_then(|t| t.get_string(&ItemKey::ReplayGainTrackGain))
        .and_then(|s| s.trim().trim_end_matches("dB").trim().parse::<f32>().ok());
        
    let track_peak = tag
        .and_then(|t| t.get_string(&ItemKey::ReplayGainTrackPeak))
        .and_then(|s| s.trim().parse::<f32>().ok());

    Ok(NewTrack {
        folder_id,
        path: f.path.to_string_lossy().to_string(),
        filename,
        title,
        artist,
        album,
        album_artist,
        genre,
        year,
        track_no,
        duration_ms: props.duration().as_millis() as i64,
        bitrate_kbps: props.audio_bitrate().map(|b| b as i64),
        sample_rate: props.sample_rate().map(|s| s as i64),
        art_path,
        size_bytes: f.size,
        mtime: f.mtime,
        track_gain_db,
        track_peak,
    })
}

fn save_art(art_dir: &Path, data: &[u8]) -> AppResult<String> {
    if data.is_empty() {
        return Err(AppError::Msg("empty picture".into()));
    }
    let ext = if data.starts_with(&[0x89, b'P', b'N', b'G']) { "png" } else { "jpg" };
    let hash = blake3::hash(data).to_hex();
    let path = art_dir.join(format!("{}.{ext}", &hash.as_str()[..24]));
    if !path.exists() {
        fs::write(&path, data)?;
    }
    Ok(path.to_string_lossy().to_string())
}