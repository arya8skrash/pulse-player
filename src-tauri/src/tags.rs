//! Writing metadata back into audio files with lofty.

use std::{path::Path, time::UNIX_EPOCH};

use lofty::{config::WriteOptions, prelude::*, tag::Tag};

use crate::error::{AppError, AppResult};

/// Writes title/artist/album into the file's primary tag (creating one if the file has none).
/// Returns the file's new (size, mtime) so the scanner does not treat our own edit as a change.
pub fn write_basic_tags(path: &Path, title: &str, artist: &str, album: &str) -> AppResult<(i64, i64)> {
    let mut tagged = lofty::read_from_path(path).map_err(|e| AppError::Msg(format!("cannot read tags: {e}")))?;

    if tagged.primary_tag().is_none() {
        let tag_type = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tag_type));
    }
    let tag = tagged
        .primary_tag_mut()
        .ok_or_else(|| AppError::Msg("this file format has no writable tag".into()))?;

    tag.set_title(title.to_string());
    tag.set_artist(artist.to_string());
    tag.set_album(album.to_string());
    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| AppError::Msg(format!("cannot write tags: {e}")))?;

    let meta = std::fs::metadata(path)?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok((meta.len() as i64, mtime))
}
