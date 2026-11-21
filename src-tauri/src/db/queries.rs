use std::collections::HashMap;

use rusqlite::{params, params_from_iter, types::Value, OptionalExtension, Row};

use super::{AlbumRow, ArtistRow, Db, Folder, GenreRow, NewTrack, Playlist, Track, TrackQuery};
use crate::error::{AppError, AppResult};

const TRACK_COLS: &str = "t.id, t.folder_id, t.path, t.filename, t.title, t.artist, t.album, t.album_artist, t.genre, \
     t.year, t.track_no, t.duration_ms, t.bitrate_kbps, t.sample_rate, t.art_path, t.play_count, t.added_at, \
     t.rating, t.is_favorite, t.last_played, t.track_gain_db, t.track_peak";

fn track_from_row(r: &Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        folder_id: r.get(1)?,
        path: r.get(2)?,
        filename: r.get(3)?,
        title: r.get(4)?,
        artist: r.get(5)?,
        album: r.get(6)?,
        album_artist: r.get(7)?,
        genre: r.get(8)?,
        year: r.get(9)?,
        track_no: r.get(10)?,
        duration_ms: r.get(11)?,
        bitrate_kbps: r.get(12)?,
        sample_rate: r.get(13)?,
        art_path: r.get(14)?,
        play_count: r.get(15)?,
        added_at: r.get(16)?,
        rating: r.get(17)?,
        is_favorite: r.get::<_, i64>(18)? != 0,
        last_played: r.get(19)?,
        track_gain_db: r.get(20)?,
        track_peak: r.get(21)?,
    })
}

fn like_escape(term: &str) -> String {
    let escaped = term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

fn folder_from_row(r: &Row) -> rusqlite::Result<Folder> {
    Ok(Folder { 
        id: r.get(0)?, 
        path: r.get(1)?, 
        watch: r.get::<_, i64>(2)? != 0, 
        added_at: r.get(3)?, 
        is_hidden: r.get::<_, i64>(4)? != 0, 
        track_count: r.get(5)? 
    })
}

const FOLDER_SELECT: &str = "SELECT f.id, f.path, f.watch, f.added_at, f.is_hidden, \
     (SELECT COUNT(*) FROM tracks t WHERE t.folder_id = f.id) FROM folders f";

impl Db {

    pub fn get_unanalyzed_tracks(&self) -> AppResult<Vec<Track>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM tracks t WHERE t.track_gain_db IS NULL", TRACK_COLS)
        )?;
        let rows = stmt.query_map([], track_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

        pub fn set_track_replaygain(&self, id: i64, gain_db: f32, peak: f32) -> AppResult<()> {
        self.lock().execute(
            "UPDATE tracks SET track_gain_db = ?2, track_peak = ?3 WHERE id = ?1",
            params![id, gain_db, peak],
        )?;
        Ok(())
    }

    pub fn add_folder(&self, path: &str) -> AppResult<Folder> {
        let id: i64 = {
            let conn = self.lock();
            conn.execute("INSERT OR IGNORE INTO folders (path) VALUES (?1)", params![path])?;
            conn.query_row("SELECT id FROM folders WHERE path = ?1", params![path], |r| r.get(0))?
        };
        self.get_folder(id)
    }

    pub fn get_folder(&self, id: i64) -> AppResult<Folder> {
        let conn = self.lock();
        conn.query_row(&format!("{FOLDER_SELECT} WHERE f.id = ?1"), params![id], folder_from_row).map_err(Into::into)
    }

    pub fn list_folders(&self) -> AppResult<Vec<Folder>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!("{FOLDER_SELECT} ORDER BY f.path COLLATE NOCASE"))?;
        let rows = stmt.query_map([], folder_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn remove_folder(&self, id: i64) -> AppResult<()> {
        self.lock().execute("DELETE FROM folders WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_folder_hidden(&self, id: i64, hidden: bool) -> AppResult<()> {
        self.lock().execute("UPDATE folders SET is_hidden = ?2 WHERE id = ?1", params![id, hidden as i64])?;
        Ok(())
    }

    pub fn track_fingerprints(&self, folder_id: i64) -> AppResult<HashMap<String, (i64, i64)>> {
        let conn = self.lock();
        let mut stmt = conn.prepare("SELECT path, size_bytes, mtime FROM tracks WHERE folder_id = ?1")?;
        let rows = stmt.query_map(params![folder_id], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?))))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn upsert_tracks_batch(&self, tracks: &[NewTrack]) -> AppResult<()> {
        if tracks.is_empty() {
            return Ok(());
        }

        let mut conn = self.lock();
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO tracks (folder_id, path, filename, title, artist, album, album_artist, genre, year, track_no,
                                     duration_ms, bitrate_kbps, sample_rate, art_path, size_bytes, mtime, track_gain_db, track_peak)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
                 ON CONFLICT(path) DO UPDATE SET
                   folder_id=excluded.folder_id, filename=excluded.filename, title=excluded.title,
                   artist=excluded.artist, album=excluded.album, album_artist=excluded.album_artist,
                   genre=excluded.genre, year=excluded.year, track_no=excluded.track_no,
                   duration_ms=excluded.duration_ms, bitrate_kbps=excluded.bitrate_kbps,
                   sample_rate=excluded.sample_rate, art_path=excluded.art_path,
                   size_bytes=excluded.size_bytes, mtime=excluded.mtime,
                   track_gain_db=excluded.track_gain_db, track_peak=excluded.track_peak"
            )?;
            
            for t in tracks {
                stmt.execute(params![
                    t.folder_id, t.path, t.filename, t.title, t.artist, t.album, t.album_artist, t.genre, t.year,
                    t.track_no, t.duration_ms, t.bitrate_kbps, t.sample_rate, t.art_path, t.size_bytes, t.mtime,
                    t.track_gain_db, t.track_peak
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_tracks_by_path(&self, paths: &[String]) -> AppResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        for p in paths {
            tx.execute("DELETE FROM tracks WHERE path = ?1", params![p])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_track(&self, id: i64) -> AppResult<Track> {
        let conn = self.lock();
        conn.query_row(&format!("SELECT {TRACK_COLS} FROM tracks t WHERE t.id = ?1"), params![id], track_from_row)
            .optional()?
            .ok_or_else(|| AppError::Msg(format!("track {id} not found")))
    }

    pub fn query_tracks(&self, q: &TrackQuery) -> AppResult<Vec<Track>> {
        let mut sql = format!("SELECT {TRACK_COLS} FROM tracks t JOIN folders f ON f.id = t.folder_id");
        let mut wh: Vec<String> = vec!["f.is_hidden = 0".into()];
        let mut args: Vec<Value> = Vec::new();

        if let Some(pid) = q.playlist_id {
            sql.push_str(" JOIN playlist_tracks pt ON pt.track_id = t.id");
            args.push(Value::Integer(pid));
            wh.push(format!("pt.playlist_id = ?{}", args.len()));
        }
        
        if let Some(s) = q.query.as_deref() {
            for term in s.split_whitespace() {
                args.push(Value::Text(like_escape(term)));
                let n = args.len();
                wh.push(format!(
                    "(t.title LIKE ?{n} ESCAPE '\\' OR t.artist LIKE ?{n} ESCAPE '\\' OR t.album LIKE ?{n} ESCAPE '\\' \
                     OR t.filename LIKE ?{n} ESCAPE '\\' OR t.genre LIKE ?{n} ESCAPE '\\')"
                ));
            }
        }
        if q.favorites_only == Some(true) {
            wh.push("t.is_favorite = 1".into());
        }
        if q.played_only == Some(true) {
            wh.push("t.play_count > 0".into());
        }
        if let Some(r) = q.min_rating {
            args.push(Value::Integer(r));
            wh.push(format!("t.rating >= ?{}", args.len()));
        }
        if let Some(a) = &q.artist {
            args.push(Value::Text(a.clone()));
            wh.push(format!("t.artist = ?{} COLLATE NOCASE", args.len()));
        }
        if let Some(a) = &q.album {
            args.push(Value::Text(a.clone()));
            wh.push(format!("t.album = ?{} COLLATE NOCASE", args.len()));
        }
        if let Some(a) = &q.album_artist {
            args.push(Value::Text(a.clone()));
            wh.push(format!("COALESCE(NULLIF(t.album_artist,''), t.artist) = ?{} COLLATE NOCASE", args.len()));
        }
        if let Some(g) = &q.genre {
            args.push(Value::Text(g.clone()));
            wh.push(format!("t.genre = ?{} COLLATE NOCASE", args.len()));
        }
        if let Some(f) = q.folder_id {
            args.push(Value::Integer(f));
            wh.push(format!("t.folder_id = ?{}", args.len()));
        }
        if !wh.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&wh.join(" AND "));
        }

        let dir = if q.desc == Some(true) { "DESC" } else { "ASC" };
        let order = match q.sort.as_deref() {
            Some("title") => format!("t.title COLLATE NOCASE {dir}"),
            Some("artist") => format!("t.artist COLLATE NOCASE {dir}, t.album COLLATE NOCASE, t.track_no, t.title COLLATE NOCASE"),
            Some("album") => format!("t.album COLLATE NOCASE {dir}, t.track_no, t.title COLLATE NOCASE"),
            Some("duration") => format!("t.duration_ms {dir}"),
            Some("added") => format!("t.added_at {dir}, t.id {dir}"),
            Some("plays") => format!("t.play_count {dir}, t.title COLLATE NOCASE"),
            Some("rating") => format!("t.rating {dir}, t.title COLLATE NOCASE"),
            Some("year") => format!("t.year {dir}, t.album COLLATE NOCASE, t.track_no"),
            _ => {
                if q.playlist_id.is_some() {
                    "pt.position, pt.id".to_string()
                } else if q.album.is_some() {
                    "t.track_no, t.title COLLATE NOCASE".to_string()
                } else if q.artist.is_some() {
                    "t.album COLLATE NOCASE, t.track_no, t.title COLLATE NOCASE".to_string()
                } else {
                    "t.artist COLLATE NOCASE, t.album COLLATE NOCASE, t.track_no, t.title COLLATE NOCASE".to_string()
                }
            }
        };
        sql.push_str(&format!(
            " ORDER BY {order} LIMIT {} OFFSET {}",
            q.limit.unwrap_or(5000).clamp(1, 20000),
            q.offset.unwrap_or(0).max(0)
        ));

        let conn = self.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), track_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn record_play(&self, id: i64) -> AppResult<()> {
        self.lock().execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = unixepoch() WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn set_rating(&self, id: i64, rating: i64) -> AppResult<()> {
        self.lock().execute("UPDATE tracks SET rating = ?2 WHERE id = ?1", params![id, rating.clamp(0, 5)])?;
        Ok(())
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> AppResult<()> {
        self.lock().execute("UPDATE tracks SET is_favorite = ?2 WHERE id = ?1", params![id, favorite as i64])?;
        Ok(())
    }

    pub fn update_metadata(&self, id: i64, title: &str, artist: &str, album: &str, size: i64, mtime: i64) -> AppResult<()> {
        self.lock().execute(
            "UPDATE tracks SET title=?2, artist=?3, album=?4, size_bytes=?5, mtime=?6 WHERE id=?1",
            params![id, title, artist, album, size, mtime],
        )?;
        Ok(())
    }

    pub fn list_artists(&self) -> AppResult<Vec<ArtistRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT artist, COUNT(*), COUNT(DISTINCT album), MAX(art_path) FROM tracks t JOIN folders f ON f.id = t.folder_id
             WHERE f.is_hidden = 0
             GROUP BY artist COLLATE NOCASE ORDER BY (artist = ''), artist COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ArtistRow { name: r.get(0)?, track_count: r.get(1)?, album_count: r.get(2)?, art_path: r.get(3)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn list_albums(&self) -> AppResult<Vec<AlbumRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT album, COALESCE(NULLIF(album_artist,''), artist) AS aa, COUNT(*), MAX(year), SUM(duration_ms), MAX(art_path)
             FROM tracks t JOIN folders f ON f.id = t.folder_id
             WHERE f.is_hidden = 0
             GROUP BY album COLLATE NOCASE, COALESCE(NULLIF(album_artist,''), artist) COLLATE NOCASE
             ORDER BY (album = ''), album COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AlbumRow {
                name: r.get(0)?,
                artist: r.get(1)?,
                track_count: r.get(2)?,
                year: r.get(3)?,
                duration_ms: r.get(4)?,
                art_path: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn list_genres(&self) -> AppResult<Vec<GenreRow>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT genre, COUNT(*), MAX(art_path) FROM tracks t JOIN folders f ON f.id = t.folder_id
             WHERE f.is_hidden = 0
             GROUP BY genre COLLATE NOCASE ORDER BY (genre = ''), genre COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| Ok(GenreRow { name: r.get(0)?, track_count: r.get(1)?, art_path: r.get(2)? }))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn list_playlists(&self) -> AppResult<Vec<Playlist>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id), p.created_at
             FROM playlists p ORDER BY p.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Playlist { id: r.get(0)?, name: r.get(1)?, track_count: r.get(2)?, created_at: r.get(3)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn create_playlist(&self, name: &str) -> AppResult<Playlist> {
        let id = {
            let conn = self.lock();
            conn.execute("INSERT INTO playlists (name) VALUES (?1)", params![name])?;
            conn.last_insert_rowid()
        };
        self.list_playlists()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::Msg("playlist was not created".into()))
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> AppResult<()> {
        self.lock().execute(
            "UPDATE playlists SET name = ?2, updated_at = unixepoch() WHERE id = ?1",
            params![id, name],
        )?;
        Ok(())
    }

    pub fn delete_playlist(&self, id: i64) -> AppResult<()> {
        self.lock().execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_to_playlist(&self, playlist_id: i64, track_ids: &[i64]) -> AppResult<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let mut pos: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |r| r.get(0),
        )?;
        for tid in track_ids {
            tx.execute(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
                params![playlist_id, tid, pos],
            )?;
            pos += 1;
        }
        tx.execute("UPDATE playlists SET updated_at = unixepoch() WHERE id = ?1", params![playlist_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn remove_from_playlist(&self, playlist_id: i64, track_id: i64) -> AppResult<()> {
        self.lock().execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![playlist_id, track_id],
        )?;
        Ok(())
    }

    pub fn set_enhancer_settings(
        &self,
        bass: f32,
        treble: f32,
        width: f32,
        surround: bool,
        booster: bool,
    ) -> AppResult<()> {
        let value = serde_json::json!({
            "bass": bass,
            "treble": treble,
            "width": width,
            "surround": surround,
            "booster": booster,
        });
        self.set_setting("audio.enhancer", &value.to_string())
    }

    pub fn all_settings(&self) -> AppResult<HashMap<String, String>> {
        let conn = self.lock();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> AppResult<()> {
        self.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}