-- Migration 2: ratings and favorites.
ALTER TABLE tracks ADD COLUMN rating      INTEGER NOT NULL DEFAULT 0;   -- 0 = unrated, 1..5 stars
ALTER TABLE tracks ADD COLUMN is_favorite INTEGER NOT NULL DEFAULT 0;
ALTER TABLE folders ADD COLUMN is_hidden INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_tracks_favorite ON tracks(is_favorite) WHERE is_favorite = 1;
CREATE INDEX idx_tracks_rating   ON tracks(rating);
CREATE INDEX idx_tracks_genre    ON tracks(genre COLLATE NOCASE);
CREATE INDEX idx_tracks_lastplay ON tracks(last_played);
