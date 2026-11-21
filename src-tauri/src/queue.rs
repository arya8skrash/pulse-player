//! Play queue with shuffle and repeat. Lives in Rust so playback never depends on the UI.
//!
//! `tracks` is the list as the user sent it; `order` is the play order (indices into `tracks`).
//! Cursors are positions in `order`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "all" => RepeatMode::All,
            "one" => RepeatMode::One,
            _ => RepeatMode::Off,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            RepeatMode::Off => "off",
            RepeatMode::All => "all",
            RepeatMode::One => "one",
        }
    }
}

pub struct Queue {
    tracks: Vec<i64>,
    order: Vec<usize>,
    cursor: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    rng: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueState {
    pub ids: Vec<i64>,
    pub cursor: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

impl Queue {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1;
        Queue { tracks: Vec::new(), order: Vec::new(), cursor: None, shuffle: false, repeat: RepeatMode::Off, rng: seed }
    }

    fn rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// All indices except `keep`, Fisher-Yates shuffled.
    fn shuffled_rest(&mut self, keep: usize) -> Vec<usize> {
        let mut rest: Vec<usize> = (0..self.tracks.len()).filter(|&i| i != keep).collect();
        for i in (1..rest.len()).rev() {
            let j = (self.rand() % (i as u64 + 1)) as usize;
            rest.swap(i, j);
        }
        rest
    }

    /// Replace the queue. With shuffle on, the chosen track plays first and the rest are shuffled.
    pub fn set(&mut self, tracks: Vec<i64>, start: usize) {
        self.tracks = tracks;
        if self.tracks.is_empty() {
            self.order.clear();
            self.cursor = None;
            return;
        }
        let start = start.min(self.tracks.len() - 1);
        if self.shuffle {
            let mut order = vec![start];
            order.extend(self.shuffled_rest(start));
            self.order = order;
            self.cursor = Some(0);
        } else {
            self.order = (0..self.tracks.len()).collect();
            self.cursor = Some(start);
        }
    }

    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub fn set_cursor(&mut self, c: usize) {
        if c < self.order.len() {
            self.cursor = Some(c);
        }
    }

    pub fn track_at(&self, cursor: usize) -> Option<i64> {
        self.order.get(cursor).and_then(|&i| self.tracks.get(i)).copied()
    }

    /// Find a track's cursor, preferring `hint` (cursors can shift when shuffle is toggled).
    pub fn locate(&self, id: i64, hint: usize) -> Option<usize> {
        if self.track_at(hint) == Some(id) {
            return Some(hint);
        }
        (0..self.order.len()).find(|&c| self.track_at(c) == Some(id))
    }

    /// `auto` = the previous track ended by itself (repeat-one replays it).
    /// `auto == false` = the user pressed Next (repeat-one behaves like repeat-all).
    pub fn next_cursor(&self, from: usize, auto: bool) -> Option<usize> {
        let len = self.order.len();
        if len == 0 {
            return None;
        }
        if auto && self.repeat == RepeatMode::One {
            return Some(from);
        }
        if from + 1 < len {
            Some(from + 1)
        } else if self.repeat != RepeatMode::Off {
            Some(0)
        } else {
            None
        }
    }

    pub fn prev_cursor(&self, from: usize) -> Option<usize> {
        let len = self.order.len();
        if len == 0 {
            None
        } else if from > 0 {
            Some(from - 1)
        } else if self.repeat != RepeatMode::Off {
            Some(len - 1)
        } else {
            None
        }
    }

    /// Toggle shuffle without interrupting the current track.
    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        let Some(c) = self.cursor else { return };
        let Some(&orig) = self.order.get(c) else { return };
        if on {
            let mut order = vec![orig];
            order.extend(self.shuffled_rest(orig));
            self.order = order;
            self.cursor = Some(0);
        } else {
            self.order = (0..self.tracks.len()).collect();
            self.cursor = Some(orig);
        }
    }

    pub fn state(&self) -> QueueState {
        QueueState {
            ids: self.order.iter().filter_map(|&i| self.tracks.get(i).copied()).collect(),
            cursor: self.cursor,
            shuffle: self.shuffle,
            repeat: self.repeat,
        }
    }
}
