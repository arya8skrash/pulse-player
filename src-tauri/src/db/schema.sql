-- Migration 1. Append new migrations to MIGRATIONS in db/mod.rs; never edit old ones.

CREATE TABLE folders (
  id        INTEGER PRIMARY KEY,
  path      TEXT    NOT NULL UNIQUE,
  watch     INTEGER NOT NULL DEFAULT 1,           -- rescan on startup
  added_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE tracks (
  id            INTEGER PRIMARY KEY,
  folder_id     INTEGER REFERENCES folders(id) ON DELETE CASCADE,
  path          TEXT    NOT NULL UNIQUE,
  filename      TEXT    NOT NULL,
  title         TEXT    NOT NULL,
  artist        TEXT    NOT NULL DEFAULT '',
  album         TEXT    NOT NULL DEFAULT '',
  album_artist  TEXT    NOT NULL DEFAULT '',
  genre         TEXT    NOT NULL DEFAULT '',
  year          INTEGER,
  track_no      INTEGER,
  duration_ms   INTEGER NOT NULL DEFAULT 0,
  bitrate_kbps  INTEGER,
  sample_rate   INTEGER,
  art_path      TEXT,
  size_bytes    INTEGER NOT NULL,
  mtime         INTEGER NOT NULL,                 -- used for incremental rescans
  play_count    INTEGER NOT NULL DEFAULT 0,
  last_played   INTEGER,
  added_at      INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_tracks_folder   ON tracks(folder_id);
CREATE INDEX idx_tracks_artist   ON tracks(artist COLLATE NOCASE);
CREATE INDEX idx_tracks_album    ON tracks(album COLLATE NOCASE);
CREATE INDEX idx_tracks_added    ON tracks(added_at);
CREATE INDEX idx_tracks_played   ON tracks(play_count);

CREATE TABLE playlists (
  id          INTEGER PRIMARY KEY,
  name        TEXT    NOT NULL,
  kind        TEXT    NOT NULL DEFAULT 'manual' CHECK (kind IN ('manual','smart')),
  rule_json   TEXT,                               -- smart-playlist rule (artist, genre, most played, ...)
  created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
  updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE playlist_tracks (
  id           INTEGER PRIMARY KEY,
  playlist_id  INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
  track_id     INTEGER NOT NULL REFERENCES tracks(id)    ON DELETE CASCADE,
  position     INTEGER NOT NULL
);
CREATE INDEX idx_pt_playlist ON playlist_tracks(playlist_id, position);

-- Key/value store; values are JSON text.
CREATE TABLE settings (
  key    TEXT PRIMARY KEY,
  value  TEXT NOT NULL
);

-- Custom themes made in the theme editor (built-ins live in the frontend).
CREATE TABLE themes (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL,
  json  TEXT NOT NULL
);
