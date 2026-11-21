use std::{
    collections::VecDeque,
    f32::consts::FRAC_PI_2,
    path::Path,
    sync::{
        atomic::Ordering,
        mpsc::{Receiver, RecvTimeoutError, TryRecvError},
        Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use rustfft::{num_complex::Complex, FftPlanner};

use super::{
    crystalizer::{Crystalizer, CrystalizerMode},
    decoder::TrackDecoder,
    enhancer::ZireneEnhancer,
    output::{open_output, Output, Shared},
    replaygain::ReplayGainProcessor, // NEW: Import ReplayGain
    virtualizer::{self, HeadphoneProfile, Virtualizer},
    AudioCmd,
    EQ_BANDS,
};
use crate::{
    db::{Db, Track},
    queue::Queue,
};

const CHUNK_FRAMES: usize = 1024;
const FFT_WINDOW: usize = 1024;
const FFT_BANDS: usize = 10;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Tick {
    track_id: i64,
    position_ms: u64,
    duration_ms: u64,
    playing: bool,
}

struct Active {
    dec: TrackDecoder,
    track: Track,
    track_id: i64,
    cursor: usize,
    duration_ms: u64,
    total_frames: u64,
    emitted: u64,
}

impl Active {
    fn fill(&mut self, frames: usize, ch: usize) {
        while self.dec.pending.len() < frames * ch && !self.dec.eof {
            self.dec.decode_next();
        }
    }

    fn take(&mut self, frames: usize, ch: usize, out: &mut Vec<f32>) -> usize {
        out.clear();
        let n = frames.min(self.dec.pending.len() / ch);
        out.extend(self.dec.pending.drain(..n * ch));
        self.emitted += n as u64;
        n
    }
}

#[derive(Clone)]
struct Boundary {
    start_frame: u64,
    track_id: i64,
    cursor: usize,
    start_ms: u64,
    duration_ms: u64,
    seq: u64,
}

struct Pipeline {
    app: AppHandle,
    db: Arc<Db>,
    queue: Arc<Mutex<Queue>>,
    rx: Receiver<AudioCmd>,
    device_name: Arc<Mutex<Option<String>>>,

    out: Option<Output>,
    device_pref: Option<String>,

    volume: f32,
    eq_enabled: bool,
    eq_gains: [f32; EQ_BANDS],
    crossfade_ms: u32,
    gapless: bool,

    paused: bool,
    cur: Option<Active>,
    incoming: Option<Active>,
    xfade_pos: usize,
    xfade_total: usize,
    xf_tried: bool,
    ended: bool,
    produced: u64,
    boundaries: VecDeque<Boundary>,
    seq: u64,
    announced: u64,
    last_tick: Instant,
    last_recover: Instant,

    chunk: Vec<f32>,
    abuf: Vec<f32>,
    bbuf: Vec<f32>,

    fft_planner: FftPlanner<f32>,
    fft_buf: Vec<Complex<f32>>,
    last_fft_time: Instant,

    virtualizer: Virtualizer,
    crystalizer: Crystalizer,
    enhancer: ZireneEnhancer,
    enhancer_amounts: [f32; 3],
    enhancer_surround: bool,
    enhancer_booster: bool,
    replaygain: ReplayGainProcessor, // NEW: ReplayGain field
    current_profile: Option<&'static HeadphoneProfile>,
}

pub fn run(
    app: AppHandle,
    db: Arc<Db>,
    queue: Arc<Mutex<Queue>>,
    rx: Receiver<AudioCmd>,
    device_name: Arc<Mutex<Option<String>>>,
) {
    let mut p = Pipeline {
        app,
        db,
        queue,
        rx,
        device_name,
        out: None,
        device_pref: None,
        volume: 0.8,
        eq_enabled: false,
        eq_gains: [0.0; EQ_BANDS],
        crossfade_ms: 0,
        gapless: true,
        paused: false,
        cur: None,
        incoming: None,
        xfade_pos: 0,
        xfade_total: 1,
        xf_tried: false,
        ended: false,
        produced: 0,
        boundaries: VecDeque::new(),
        seq: 0,
        announced: 0,
        last_tick: Instant::now(),
        last_recover: Instant::now(),
        chunk: Vec::new(),
        abuf: Vec::new(),
        bbuf: Vec::new(),
        fft_planner: FftPlanner::new(),
        fft_buf: vec![Complex::new(0.0, 0.0); FFT_WINDOW],
        last_fft_time: Instant::now(),
        virtualizer: Virtualizer::new(2),
        crystalizer: Crystalizer::new(44100.0, 2),
        enhancer: ZireneEnhancer::new(44100.0, 2),
        enhancer_amounts: [0.0; 3],
        enhancer_surround: false,
        enhancer_booster: false,
        replaygain: ReplayGainProcessor::new(), // NEW: Init ReplayGain
        current_profile: None,
    };
    p.open_device();
    p.main_loop();
}

impl Pipeline {
    fn q(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn report(&self, msg: String) {
        log::error!("{msg}");
        let _ = self.app.emit("player:error", msg);
    }

    fn main_loop(&mut self) {
        loop {
            loop {
                match self.rx.try_recv() {
                    Ok(cmd) => self.handle(cmd),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            let busy = self.produce_step();
            self.monitor();
            if !busy {
                let wait = if self.cur.is_some() { 5 } else { 25 };
                match self.rx.recv_timeout(Duration::from_millis(wait)) {
                    Ok(cmd) => self.handle(cmd),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }
    }

    fn handle(&mut self, cmd: AudioCmd) {
        match cmd {
            AudioCmd::PlayAt { cursor, offset_ms } => self.start_at(cursor, offset_ms, true),
            AudioCmd::PlayDirect { path, offset_ms } => self.play_direct(path, offset_ms),
            AudioCmd::PlayDirectTrack { track, offset_ms } => {
                self.play_direct_track(track, offset_ms)
            }
            AudioCmd::Pause => self.set_paused(true),
            AudioCmd::Resume => self.set_paused(false),
            AudioCmd::Toggle => {
                if !self.boundaries.is_empty() {
                    self.set_paused(!self.paused);
                }
            }
            AudioCmd::Stop => self.stop(),
            AudioCmd::Seek(ms) => self.seek(ms),
            AudioCmd::Next => self.next(),
            AudioCmd::Prev => self.prev(),
            AudioCmd::SetVolume(v) => {
                self.volume = v.clamp(0.0, 1.0);
                self.push_dsp();
            }
            AudioCmd::SetEq { enabled, gains } => {
                self.eq_enabled = enabled;
                self.eq_gains = gains;
                self.push_dsp();
            }
            AudioCmd::SetCrossfade(ms) => self.crossfade_ms = ms.min(12_000),
            AudioCmd::SetGapless(on) => self.gapless = on,
            AudioCmd::SetDevice(name) => self.switch_device(name),
            AudioCmd::SetVirtualizer {
                enabled,
                profile_name,
            } => {
                if enabled {
                    self.current_profile =
                        profile_name.as_deref().and_then(virtualizer::find_profile);
                } else {
                    self.current_profile = None;
                }
                if let Some(o) = &self.out {
                    self.virtualizer
                        .set_profile(o.rate as f32, self.current_profile);
                }
            }
            AudioCmd::SetCrystalizer { mode, intensity } => {
                let m = match mode.as_str() {
                    "plus" => CrystalizerMode::CrystalizerPlus,
                    "hx" => CrystalizerMode::CrystalizerHx,
                    "hxplus" => CrystalizerMode::CrystalizerHxPlus,
                    _ => CrystalizerMode::Crystalizer,
                };
                self.crystalizer.set_mode(m);
                self.crystalizer.set_intensity(intensity);
            }
            AudioCmd::SetEnhancer {
                bass,
                treble,
                width,
                surround,
                booster,
            } => {
                self.enhancer_amounts = [bass, treble, width];
                self.enhancer_surround = surround;
                self.enhancer_booster = booster;
                self.apply_enhancer_settings();
            }
            // NEW: Handle SetReplayGain (toggles the processor on/off globally)
            AudioCmd::SetReplayGain { enabled } => {
                self.replaygain.set_enabled(enabled);
            }
        }
    }

    fn play_direct(&mut self, path: String, offset_ms: u64) {
        let p = Path::new(&path);
        let filename = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let title = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let track = Track {
            id: -1,
            folder_id: None,
            path,
            filename,
            title,
            artist: "Unknown Artist".to_string(),
            album: "".to_string(),
            album_artist: "".to_string(),
            genre: "".to_string(),
            year: None,
            track_no: None,
            duration_ms: 0,
            bitrate_kbps: None,
            sample_rate: None,
            art_path: None,
            play_count: 0,
            added_at: 0,
            rating: 0,
            is_favorite: false,
            last_played: None,
            track_gain_db: None,
            track_peak: None,
        };
        self.play_direct_track(track, offset_ms);
    }

    fn play_direct_track(&mut self, mut track: Track, offset_ms: u64) {
        if self.out.is_none() {
            self.open_device();
        }
        if self.out.is_none() {
            return;
        }
        self.flush();
        self.cur = None;
        self.incoming = None;
        self.boundaries.clear();
        self.ended = false;
        self.xf_tried = false;
        self.paused = false;
        if let Some(o) = &self.out {
            o.shared.paused.store(false, Ordering::Release);
        }

        let (rate, ch) = match &self.out {
            Some(o) => (o.rate, o.channels),
            None => return,
        };

        // Direct playback (drag & drop) usually lacks DB metadata, so reset ReplayGain to neutral
        self.replaygain.set_track_gain(0.0);
        self.replaygain.set_track_peak(1.0);
        track.id = -1;
        track.folder_id = None;

        let path = track.path.clone();
        match TrackDecoder::open(&path, rate, ch, offset_ms) {
            Ok(dec) => {
                let total = dec.total_out_frames.unwrap_or(0);
                let duration_ms = if total > 0 {
                    total * 1000 / rate as u64
                } else {
                    track.duration_ms.max(0) as u64
                };
                track.duration_ms = duration_ms as i64;

                let a = Active {
                    dec,
                    track: track.clone(),
                    track_id: -1,
                    cursor: 0,
                    duration_ms,
                    total_frames: total,
                    emitted: offset_ms * rate as u64 / 1000,
                };
                self.push_boundary(&a, offset_ms);
                self.cur = Some(a);

                let _ = self.app.emit("player:track-changed", track);
            }
            Err(e) => {
                self.report(e);
                let _ = self.app.emit("player:stopped", ());
            }
        }
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if let Some(o) = &self.out {
            o.shared.paused.store(paused, Ordering::Release);
        }
        self.emit_tick();
    }

    fn stop(&mut self) {
        self.flush();
        self.cur = None;
        self.incoming = None;
        self.boundaries.clear();
        self.ended = false;
        self.xf_tried = false;
        self.paused = false;
        if let Some(o) = &self.out {
            o.shared.paused.store(false, Ordering::Release);
        }
        let _ = self.app.emit("player:stopped", ());
    }

    fn seek(&mut self, ms: u64) {
        let Some((tid, cursor, _, dur)) = self.audible() else {
            return;
        };
        let ms = if dur > 0 {
            ms.min(dur.saturating_sub(100))
        } else {
            ms
        };
        if tid == -1 {
            if let Some(track) = self.cur.as_ref().map(|cur| cur.track.clone()) {
                self.play_direct_track(track, ms);
            }
            return;
        }
        let c = self.q().locate(tid, cursor).unwrap_or(cursor);
        self.start_at(c, ms, false);
    }

    fn next(&mut self) {
        let Some((tid, cursor, _, _)) = self.audible() else {
            return;
        };
        if tid == -1 {
            self.stop();
            return;
        }
        let n = {
            let q = self.q();
            q.locate(tid, cursor).and_then(|c| q.next_cursor(c, false))
        };
        match n {
            Some(c) => self.start_at(c, 0, true),
            None => self.stop(),
        }
    }

    fn prev(&mut self) {
        let Some((tid, cursor, pos, _)) = self.audible() else {
            return;
        };
        if tid == -1 {
            self.stop();
            return;
        }
        if pos > 3000 {
            self.seek(0);
            return;
        }
        let p = {
            let q = self.q();
            q.locate(tid, cursor).and_then(|c| q.prev_cursor(c))
        };
        match p {
            Some(c) => self.start_at(c, 0, true),
            None => self.seek(0),
        }
    }

    fn apply_enhancer_settings(&mut self) {
        self.enhancer.set_bass_amount(self.enhancer_amounts[0]);
        self.enhancer.set_treble_amount(self.enhancer_amounts[1]);
        self.enhancer.set_width_amount(self.enhancer_amounts[2]);
        self.enhancer.set_surround_enabled(self.enhancer_surround);
        self.enhancer.set_booster_enabled(self.enhancer_booster);
    }

    fn open_device(&mut self) {
        let opened = match open_output(self.device_pref.as_deref()) {
            Ok(o) => Ok(o),
            Err(e) if self.device_pref.is_some() => {
                log::warn!(
                    "device {:?} unavailable ({e}); using default",
                    self.device_pref
                );
                open_output(None)
            }
            Err(e) => Err(e),
        };
        match opened {
            Ok(o) => {
                if let Ok(mut g) = self.device_name.lock() {
                    *g = Some(o.device_name.clone());
                }

                self.virtualizer = Virtualizer::new(o.channels);
                self.crystalizer = Crystalizer::new(o.rate as f32, o.channels);
                self.enhancer = ZireneEnhancer::new(o.rate as f32, o.channels);
                self.apply_enhancer_settings();
                self.enhancer.reset();

                if let Some(p) = self.current_profile {
                    self.virtualizer.set_profile(o.rate as f32, Some(p));
                }
                self.out = Some(o);
                self.push_dsp();
            }
            Err(e) => self.report(e),
        }
    }

    fn switch_device(&mut self, name: Option<String>) {
        let resume = self.audible();
        let direct_track = self.cur.as_ref().map(|active| active.track.clone());
        let was_paused = self.paused;
        self.cur = None;
        self.incoming = None;
        self.boundaries.clear();
        self.out = None;
        self.device_pref = name;
        self.open_device();
        if let (Some((tid, cursor, pos, _)), true) = (resume, self.out.is_some()) {
            if tid == -1 {
                if let Some(track) = direct_track {
                    self.play_direct_track(track, pos);
                    self.paused = was_paused;
                    if let Some(output) = &self.out {
                        output.shared.paused.store(was_paused, Ordering::Release);
                    }
                }
                return;
            }
            let c = self.q().locate(tid, cursor).unwrap_or(cursor);
            self.paused = was_paused;
            self.start_at(c, pos, false);
        }
    }

    fn push_dsp(&self) {
        let Some(o) = &self.out else { return };
        let sh = &o.shared;
        sh.volume_bits
            .store(self.volume.to_bits(), Ordering::Release);
        sh.eq_enabled.store(self.eq_enabled, Ordering::Release);
        for (i, g) in self.eq_gains.iter().enumerate() {
            sh.eq_gains[i].store(g.to_bits(), Ordering::Release);
        }
        sh.eq_gen.fetch_add(1, Ordering::AcqRel);
        sh.paused.store(self.paused, Ordering::Release);
    }

    fn flush(&mut self) {
        self.enhancer.reset();
        let Some(o) = &self.out else { return };
        let g = o.shared.flush_gen.fetch_add(1, Ordering::AcqRel) + 1;
        let t0 = Instant::now();
        while o.shared.ack_gen.load(Ordering::Acquire) != g
            && t0.elapsed() < Duration::from_millis(300)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        self.produced = o.shared.consumed.load(Ordering::Acquire);
    }

    fn start_at(&mut self, cursor: usize, offset_ms: u64, reset_pause: bool) {
        if self.out.is_none() {
            self.open_device();
        }
        if self.out.is_none() {
            return;
        }
        self.flush();
        self.cur = None;
        self.incoming = None;
        self.boundaries.clear();
        self.ended = false;
        self.xf_tried = false;
        if reset_pause {
            self.paused = false;
        }
        if let Some(o) = &self.out {
            o.shared.paused.store(self.paused, Ordering::Release);
        }
        match self.open_cursor(cursor, offset_ms) {
            Ok(a) => {
                self.push_boundary(&a, offset_ms);
                self.cur = Some(a);
            }
            Err(e) => {
                self.report(e);
                let _ = self.app.emit("player:stopped", ());
            }
        }
    }

    fn open_cursor(&self, cursor: usize, offset_ms: u64) -> Result<Active, String> {
        let (rate, ch) = match &self.out {
            Some(o) => (o.rate, o.channels),
            None => return Err("No audio output device".into()),
        };
        let id = self.q().track_at(cursor).ok_or("The queue is empty")?;
        let track = self.db.get_track(id).map_err(|e| e.to_string())?;
        let dec = TrackDecoder::open(&track.path, rate, ch, offset_ms)?;
        let total = dec
            .total_out_frames
            .unwrap_or_else(|| track.duration_ms.max(0) as u64 * rate as u64 / 1000);
        let duration_ms = if track.duration_ms > 0 {
            track.duration_ms as u64
        } else {
            total * 1000 / rate as u64
        };
        Ok(Active {
            dec,
            track: track.clone(),
            track_id: id,
            cursor,
            duration_ms,
            total_frames: total,
            emitted: offset_ms * rate as u64 / 1000,
        })
    }

    fn open_next(&mut self, from_track: i64, from_cursor: usize) -> Option<Active> {
        if from_track == -1 {
            return None;
        }
        let (mut tid, mut hint) = (from_track, from_cursor);
        for _ in 0..5 {
            let next = {
                let q = self.q();
                let c = q.locate(tid, hint)?;
                q.next_cursor(c, true)
            }?;
            let next_id = self.q().track_at(next);
            match self.open_cursor(next, 0) {
                Ok(a) => return Some(a),
                Err(e) => {
                    self.report(e);
                    tid = next_id?;
                    hint = next;
                }
            }
        }
        None
    }

    fn push_boundary(&mut self, a: &Active, start_ms: u64) {
        self.seq += 1;
        self.boundaries.push_back(Boundary {
            start_frame: self.produced,
            track_id: a.track_id,
            cursor: a.cursor,
            start_ms,
            duration_ms: a.duration_ms,
            seq: self.seq,
        });
    }

    fn produce_step(&mut self) -> bool {
        let (shared, ch, rate): (Arc<Shared>, usize, u32) = match &self.out {
            Some(o) => (o.shared.clone(), o.channels, o.rate),
            None => return false,
        };
        if self.cur.is_none() {
            return false;
        }
        let n = CHUNK_FRAMES;
        if shared.ring.free() < n * ch {
            return false;
        }

        if self.incoming.is_none() && !self.xf_tried && self.crossfade_ms > 0 {
            let xf = (self.crossfade_ms as u64 * rate as u64 / 1000) as usize;
            let (remaining, total, tid, cursor) = {
                let c = self.cur.as_ref().unwrap();
                (
                    c.total_frames.saturating_sub(c.emitted),
                    c.total_frames,
                    c.track_id,
                    c.cursor,
                )
            };
            if xf > 0 && total > xf as u64 * 3 && remaining <= xf as u64 {
                self.xf_tried = true;
                if let Some(a) = self.open_next(tid, cursor) {
                    self.xfade_total = (xf as u64).min(remaining.max(1)) as usize;
                    self.xfade_pos = 0;
                    self.push_boundary(&a, 0);
                    self.incoming = Some(a);
                }
            }
        }

        let mut chunk = std::mem::take(&mut self.chunk);
        let mut abuf = std::mem::take(&mut self.abuf);
        let mut bbuf = std::mem::take(&mut self.bbuf);
        chunk.clear();
        let mut finished_xfade = false;

        if self.incoming.is_some() {
            let fa = {
                let cur = self.cur.as_mut().unwrap();
                cur.fill(n, ch);
                cur.take(n, ch, &mut abuf)
            };
            let fb = {
                let inc = self.incoming.as_mut().unwrap();
                inc.fill(n, ch);
                inc.take(n, ch, &mut bbuf)
            };
            let frames = fa.max(fb);
            for f in 0..frames {
                let t = ((self.xfade_pos + f) as f32 / self.xfade_total.max(1) as f32).min(1.0);
                let (ga, gb) = ((t * FRAC_PI_2).cos(), (t * FRAC_PI_2).sin());
                for c in 0..ch {
                    let a = if f < fa { abuf[f * ch + c] } else { 0.0 };
                    let b = if f < fb { bbuf[f * ch + c] } else { 0.0 };
                    chunk.push(a * ga + b * gb);
                }
            }
            self.xfade_pos += frames;
            let cur_done = {
                let c = self.cur.as_ref().unwrap();
                c.dec.eof && c.dec.pending.is_empty()
            };
            if frames == 0 || self.xfade_pos >= self.xfade_total || cur_done {
                finished_xfade = true;
            }
        } else {
            let got = {
                let cur = self.cur.as_mut().unwrap();
                cur.fill(n, ch);
                cur.take(n, ch, &mut chunk)
            };
            if got == 0 {
                let (done, tid, cursor) = {
                    let c = self.cur.as_ref().unwrap();
                    (c.dec.eof, c.track_id, c.cursor)
                };
                if done {
                    self.chunk = chunk;
                    self.abuf = abuf;
                    self.bbuf = bbuf;
                    return self.on_track_exhausted(tid, cursor, &shared);
                }
            }
        }

        let pushed = !chunk.is_empty();
        if pushed {
            self.replaygain.apply(&mut chunk);

            // Booster precedes both Crystalizer and the ZR enhancer.
            self.enhancer.process_preamp(&mut chunk);
            self.crystalizer.process_frame(&mut chunk);

            // Surround runs inside the enhancer, before its linked limiter.
            self.enhancer.process_frame(&mut chunk);
            self.virtualizer.process_frame(&mut chunk);

            // Virtualization may create new peaks after the enhancer limiter.
            self.enhancer.protect_output(&mut chunk);

            if self.last_fft_time.elapsed() >= Duration::from_millis(33) {
                self.last_fft_time = Instant::now();
                self.calc_fft(&chunk, ch);
            }

            shared.ring.push(&chunk);
            self.produced += (chunk.len() / ch) as u64;
        }
        self.chunk = chunk;
        self.abuf = abuf;
        self.bbuf = bbuf;

        if finished_xfade {
            self.cur = self.incoming.take();
            self.xf_tried = false;
        }
        pushed || finished_xfade
    }

    fn calc_fft(&mut self, chunk: &[f32], ch: usize) {
        if chunk.len() < FFT_WINDOW * ch {
            return;
        }

        for i in 0..FFT_WINDOW {
            let window =
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / FFT_WINDOW as f32).cos());
            self.fft_buf[i] = Complex::new(chunk[i * ch] * window, 0.0);
        }

        let fft = self.fft_planner.plan_fft_forward(FFT_WINDOW);
        fft.process(&mut self.fft_buf);

        let mut bands = [0.0f32; FFT_BANDS];
        let nyquist = FFT_WINDOW / 2;

        for i in 0..FFT_BANDS {
            let start_freq = (nyquist as f32).powf(i as f32 / FFT_BANDS as f32) as usize;
            let end_freq = (nyquist as f32).powf((i + 1) as f32 / FFT_BANDS as f32) as usize;

            let start = start_freq.max(1);
            let end = end_freq.min(nyquist).max(start + 1);

            let mut sum = 0.0;
            for j in start..end {
                sum += self.fft_buf[j].norm();
            }

            let avg = sum / (end - start) as f32;
            let gated = (avg - 0.01).max(0.0);
            let scaled = (gated * 200.0).ln_1p() / 5.0;

            bands[i] = scaled.min(1.0);
        }

        let _ = self.app.emit("player:fft", bands);
    }

    fn on_track_exhausted(&mut self, tid: i64, cursor: usize, shared: &Arc<Shared>) -> bool {
        if !self.gapless && shared.ring.len() > 0 {
            return false;
        }
        if tid == -1 {
            self.cur = None;
            self.ended = true;
            return false;
        }
        match self.open_next(tid, cursor) {
            Some(a) => {
                self.push_boundary(&a, 0);
                self.cur = Some(a);
                self.xf_tried = false;
                true
            }
            None => {
                self.cur = None;
                self.ended = true;
                false
            }
        }
    }

    fn audible(&self) -> Option<(i64, usize, u64, u64)> {
        let b = self.boundaries.front()?;
        let o = self.out.as_ref()?;
        let consumed = o.shared.consumed.load(Ordering::Acquire);
        let frames = consumed.saturating_sub(b.start_frame);
        let mut pos = b.start_ms + frames * 1000 / o.rate as u64;
        if b.duration_ms > 0 {
            pos = pos.min(b.duration_ms);
        }
        Some((b.track_id, b.cursor, pos, b.duration_ms))
    }

    fn emit_tick(&self) {
        if let Some((track_id, _, position_ms, duration_ms)) = self.audible() {
            let _ = self.app.emit(
                "player:tick",
                Tick {
                    track_id,
                    position_ms,
                    duration_ms,
                    playing: !self.paused,
                },
            );
        }
    }

    fn monitor(&mut self) {
        let (shared, consumed) = match &self.out {
            Some(o) => (o.shared.clone(), o.shared.consumed.load(Ordering::Acquire)),
            None => return,
        };

        while self.boundaries.len() >= 2 && self.boundaries[1].start_frame <= consumed {
            self.boundaries.pop_front();
        }
        if let Some(b) = self.boundaries.front().cloned() {
            if b.seq != self.announced {
                self.announced = b.seq;
                self.announce(&b);
            }
            if self.last_tick.elapsed() >= Duration::from_millis(50) {
                self.last_tick = Instant::now();
                self.emit_tick();
            }
        }

        if self.cur.is_none() && self.incoming.is_none() && self.ended && consumed >= self.produced
        {
            self.ended = false;
            self.boundaries.clear();
            let _ = self.app.emit("player:queue-ended", ());
        }

        if shared.failed.load(Ordering::Acquire)
            && self.last_recover.elapsed() > Duration::from_secs(2)
        {
            self.last_recover = Instant::now();
            log::warn!("audio stream failed; reopening output");
            let pref = self.device_pref.clone();
            self.switch_device(pref);
        }
    }

    fn announce(&self, b: &Boundary) {
        if b.track_id == -1 {
            return;
        }
        {
            let mut q = self.q();
            if let Some(c) = q.locate(b.track_id, b.cursor) {
                q.set_cursor(c);
            }
        }
        if b.start_ms == 0 {
            let _ = self.db.record_play(b.track_id);
        }
        if let Ok(t) = self.db.get_track(b.track_id) {
            let _ = self.app.emit("player:track-changed", t);
        }
    }
}
