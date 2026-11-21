use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub path: String,
    pub filename: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub duration_ms: i64,
    pub bitrate_kbps: Option<i64>,
    pub sample_rate: Option<i64>,
    pub art_path: Option<String>,
    pub play_count: i64,
    pub added_at: i64,
    pub rating: i64,
    pub is_favorite: bool,
    pub last_played: Option<i64>,
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
}

/// What the scanner produces before a row id exists.
#[derive(Debug, Clone)]
pub struct NewTrack {
    pub folder_id: i64,
    pub path: String,
    pub filename: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub duration_ms: i64,
    pub bitrate_kbps: Option<i64>,
    pub sample_rate: Option<i64>,
    pub art_path: Option<String>,
    pub size_bytes: i64,
    pub mtime: i64,
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: i64,
    pub path: String,
    pub watch: bool,
    pub added_at: i64,
    pub track_count: i64,
    pub is_hidden: bool, // New field
}

/// One flexible query used by every library view (songs, artist/album/genre detail, playlists, folders).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackQuery {
    pub query: Option<String>,
    /// title | artist | album | duration | added | plays | rating | year
    pub sort: Option<String>,
    pub desc: Option<bool>,
    pub favorites_only: Option<bool>,
    pub min_rating: Option<i64>,
    pub played_only: Option<bool>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub folder_id: Option<i64>,
    pub playlist_id: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistRow {
    pub name: String,
    pub track_count: i64,
    pub album_count: i64,
    pub art_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumRow {
    pub name: String,
    pub artist: String,
    pub track_count: i64,
    pub year: Option<i64>,
    pub duration_ms: i64,
    pub art_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenreRow {
    pub name: String,
    pub track_count: i64,
    pub art_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub created_at: i64,
}