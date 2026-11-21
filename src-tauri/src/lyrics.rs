//! Lyrics: a `.lrc` file next to the audio file wins; otherwise embedded lyrics from the tags.
//! Timed lines (`[mm:ss.xx]text`) give synced lyrics, anything else is shown as plain text.

use std::{fs, path::Path};

use lofty::prelude::*;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub synced: bool,
    /// "lrc" (sidecar file) or "embedded"
    pub source: String,
    pub lines: Vec<LyricLine>,
}

pub fn load_for(audio_path: &Path) -> Option<Lyrics> {
    for ext in ["lrc", "LRC", "Lrc"] {
        let candidate = audio_path.with_extension(ext);
        if let Ok(bytes) = fs::read(&candidate) {
            let text = String::from_utf8_lossy(&bytes);
            if let Some(mut l) = parse_lrc(&text) {
                l.source = "lrc".into();
                return Some(l);
            }
        }
    }
    let tagged = lofty::read_from_path(audio_path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let text = tag.get_string(&ItemKey::Lyrics)?.to_string();
    let mut l = parse_lrc(&text)?;
    l.source = "embedded".into();
    Some(l)
}

fn parse_timestamp(tag: &str) -> Option<u64> {
    let (m, s) = tag.split_once(':')?;
    let minutes: u64 = m.trim().parse().ok()?;
    let secs: f64 = s.trim().replace(':', ".").parse().ok()?;
    Some(minutes * 60_000 + (secs * 1000.0).round() as u64)
}

/// Remove inline word timestamps like `<00:12.30>` from enhanced LRC.
fn strip_inline_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0;
    for ch in s.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

pub fn parse_lrc(text: &str) -> Option<Lyrics> {
    let mut timed: Vec<(u64, String)> = Vec::new();
    let mut plain: Vec<String> = Vec::new();
    let mut offset: i64 = 0;

    for raw in text.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        let mut rest = line;
        let mut times: Vec<u64> = Vec::new();
        let mut meta_only = false;
        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            let tag = &rest[1..end];
            if let Some(t) = parse_timestamp(tag) {
                times.push(t);
            } else if let Some(v) = tag.strip_prefix("offset:") {
                offset = v.trim().parse().unwrap_or(0);
                meta_only = true;
            } else {
                meta_only = true; // [ar:..] [ti:..] [al:..] etc.
            }
            rest = &rest[end + 1..];
        }
        if times.is_empty() {
            if !meta_only {
                plain.push(strip_inline_tags(line));
            }
            continue;
        }
        let body = strip_inline_tags(rest);
        for t in times {
            timed.push((t, body.clone()));
        }
    }

    if !timed.is_empty() {
        timed.sort_by_key(|(t, _)| *t);
        let lines = timed
            .into_iter()
            .map(|(t, text)| LyricLine { time_ms: Some((t as i64 - offset).max(0) as u64), text })
            .collect();
        return Some(Lyrics { synced: true, source: String::new(), lines });
    }
    if plain.iter().any(|l| !l.is_empty()) {
        let lines = plain.into_iter().map(|text| LyricLine { time_ms: None, text }).collect();
        return Some(Lyrics { synced: false, source: String::new(), lines });
    }
    None
}
