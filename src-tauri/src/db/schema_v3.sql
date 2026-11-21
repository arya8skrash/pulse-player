-- Migration 3: add is_hidden to folders.
ALTER TABLE folders ADD COLUMN is_hidden INTEGER NOT NULL DEFAULT 0;