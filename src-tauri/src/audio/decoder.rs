//! One decoded track: Symphonia demux/decode -> f32 -> channel map -> resample, so `pending`
//! always holds interleaved f32 in the *output device's* sample rate and channel count.

use std::{collections::VecDeque, fs::File, path::Path, sync::OnceLock};

use symphonia::core::{
    audio::SampleBuffer,
    codecs::{CodecRegistry, Decoder, DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymError,
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

use super::resample::Resampler;

/// Build the normal Symphonia decoder registry plus Opus support for YouTube's
/// WebM audio streams. Symphonia 0.5 does not include an Opus decoder itself.
pub(crate) fn codec_registry() -> &'static CodecRegistry {
    static CODECS: OnceLock<CodecRegistry> = OnceLock::new();
    CODECS.get_or_init(|| {
        let mut registry = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut registry);
        registry.register_all::<symphonia_adapter_libopus::OpusDecoder>();
        registry
    })
}

pub struct TrackDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    in_rate: u32,
    in_ch: usize,
    out_rate: u32,
    out_ch: usize,
    sbuf: Option<SampleBuffer<f32>>,
    raw: Vec<f32>,
    mapped: Vec<f32>,
    resampler: Option<Resampler>,
    skip_in_frames: u64,
    errors: u32,
    pub pending: VecDeque<f32>,
    pub eof: bool,
    pub total_out_frames: Option<u64>,
}

enum Step {
    Data(usize, usize),
    Skip,
    Done,
}

impl TrackDecoder {
    pub fn open(path: &str, out_rate: u32, out_ch: usize, start_ms: u64) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("Cannot open {path}: {e}"))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = Path::new(path).extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions { enable_gapless: true, ..Default::default() }, &MetadataOptions::default())
            .map_err(|e| format!("Cannot read {path}: {e}"))?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| format!("{path}: no audio track"))?;
        let track_id = track.id;
        let params = track.codec_params.clone();
        let decoder = codec_registry()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| format!("Unsupported audio in {path}: {e}"))?;

        let in_rate = params.sample_rate.unwrap_or(out_rate);
        let in_ch = params.channels.map(|c| c.count()).filter(|&c| c > 0).unwrap_or(2);
        let total_out_frames = params.n_frames.map(|n| n * out_rate as u64 / in_rate.max(1) as u64);

        let mut d = TrackDecoder {
            format,
            decoder,
            track_id,
            in_rate,
            in_ch,
            out_rate,
            out_ch,
            sbuf: None,
            raw: Vec::new(),
            mapped: Vec::new(),
            resampler: None,
            skip_in_frames: 0,
            errors: 0,
            pending: VecDeque::new(),
            eof: false,
            total_out_frames,
        };
        d.configure(in_rate, in_ch);
        if start_ms > 0 {
            if let Err(e) = d.seek(start_ms) {
                log::warn!("initial seek failed: {e}");
            }
        }
        Ok(d)
    }

    fn configure(&mut self, rate: u32, ch: usize) {
        self.in_rate = rate;
        self.in_ch = ch;
        self.resampler = if rate != self.out_rate { Some(Resampler::new(rate, self.out_rate, self.out_ch)) } else { None };
    }

    pub fn seek(&mut self, ms: u64) -> Result<(), String> {
        let time = Time::new(ms / 1000, (ms % 1000) as f64 / 1000.0);
        let seeked = self
            .format
            .seek(SeekMode::Accurate, SeekTo::Time { time, track_id: Some(self.track_id) })
            .map_err(|e| e.to_string())?;
        self.decoder.reset();
        self.pending.clear();
        self.eof = false;
        // The demuxer lands on a packet boundary at or before the target; trim the difference.
        let skip = seeked.required_ts.saturating_sub(seeked.actual_ts);
        self.skip_in_frames = if skip < self.in_rate as u64 * 10 { skip } else { 0 };
        if self.resampler.is_some() {
            self.configure(self.in_rate, self.in_ch); // fresh filter history
        }
        Ok(())
    }

    /// Decode one packet into `pending`, or set `eof`.
    pub fn decode_next(&mut self) {
        if self.eof {
            return;
        }
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymError::IoError(_)) | Err(SymError::ResetRequired) => {
                    self.finish();
                    return;
                }
                Err(e) => {
                    log::warn!("read error: {e}");
                    self.finish();
                    return;
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }

            let step = match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let need = decoded.capacity() * spec.channels.count();
                    if self.sbuf.as_ref().map_or(true, |b| b.capacity() < need) {
                        self.sbuf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                    }
                    let sb = self.sbuf.as_mut().unwrap();
                    sb.copy_interleaved_ref(decoded);
                    self.raw.clear();
                    self.raw.extend_from_slice(sb.samples());
                    Step::Data(spec.rate as usize, spec.channels.count())
                }
                Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => {
                    // A corrupt packet: skip it, give up only if the stream is hopeless.
                    self.errors += 1;
                    if self.errors > 200 { Step::Done } else { Step::Skip }
                }
                Err(e) => {
                    log::warn!("decode error: {e}");
                    Step::Done
                }
            };

            match step {
                Step::Data(rate, ch) => {
                    self.errors = 0;
                    self.process_raw(rate as u32, ch);
                    return;
                }
                Step::Skip => continue,
                Step::Done => {
                    self.finish();
                    return;
                }
            }
        }
    }

    fn process_raw(&mut self, rate: u32, ch: usize) {
        if ch == 0 {
            return;
        }
        if rate != self.in_rate || ch != self.in_ch {
            self.configure(rate, ch);
        }
        let raw = std::mem::take(&mut self.raw);
        let mut start = 0usize;
        if self.skip_in_frames > 0 {
            let frames = raw.len() / ch;
            let skip = (self.skip_in_frames as usize).min(frames);
            self.skip_in_frames -= skip as u64;
            start = skip * ch;
        }

        self.mapped.clear();
        map_channels(&raw[start..], ch, self.out_ch, &mut self.mapped);

        match self.resampler.as_mut() {
            Some(r) => {
                let mut out = Vec::with_capacity(self.mapped.len() * 2);
                r.process(&self.mapped, &mut out);
                self.pending.extend(out.iter().copied());
            }
            None => self.pending.extend(self.mapped.iter().copied()),
        }
        self.raw = raw;
    }

    fn finish(&mut self) {
        if let Some(r) = self.resampler.as_mut() {
            let mut tail = Vec::new();
            r.finish(&mut tail);
            self.pending.extend(tail.iter().copied());
        }
        self.eof = true;
    }
}

/// Convert interleaved `ic`-channel audio to `oc` channels.
fn map_channels(inp: &[f32], ic: usize, oc: usize, out: &mut Vec<f32>) {
    if ic == oc {
        out.extend_from_slice(inp);
        return;
    }
    let frames = inp.len() / ic;
    out.reserve(frames * oc);
    for f in 0..frames {
        let fr = &inp[f * ic..(f + 1) * ic];
        match (ic, oc) {
            (1, _) => (0..oc).for_each(|_| out.push(fr[0])),
            (_, 1) => out.push(fr.iter().sum::<f32>() / ic as f32),
            // 5.1 -> stereo (FL FR FC LFE SL SR), ITU-style downmix, normalised
            (6, 2) => {
                let c = fr[2] * 0.7071;
                out.push((fr[0] + c + fr[4] * 0.7071) * 0.4142);
                out.push((fr[1] + c + fr[5] * 0.7071) * 0.4142);
            }
            (_, 2) => {
                let (mut l, mut r, mut nl, mut nr) = (0.0, 0.0, 0.0, 0.0);
                for (i, s) in fr.iter().enumerate() {
                    if i % 2 == 0 { l += s; nl += 1.0 } else { r += s; nr += 1.0 }
                }
                out.push(l / f32::max(nl, 1.0));
                out.push(r / f32::max(nr, 1.0));
            }
            (2, _) => {
                out.push(fr[0]);
                out.push(fr[1]);
                (2..oc).for_each(|_| out.push(0.0));
            }
            _ => (0..oc).for_each(|i| out.push(fr[i % ic])),
        }
    }
}
