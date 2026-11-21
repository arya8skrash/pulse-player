//! cpal output. The device is opened in its *native* configuration (rate, channels, sample
//! format) and everything upstream is converted to match, which is what keeps pitch and speed
//! correct. The callback only pops pre-rendered f32 frames from the ring and applies EQ, volume,
//! pause ramps and a short fade-in after a flush, so it never blocks, allocates (after warm-up)
//! or decodes.

use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    Arc,
};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    FromSample, Sample, SizedSample,
};

use super::{
    eq::{Equalizer, BANDS},
    resample::MAX_CH,
    ring::Ring,
};

pub struct Shared {
    pub ring: Ring,
    /// Frames the callback has consumed (played or discarded). Position is derived from this.
    pub consumed: AtomicU64,
    pub paused: AtomicBool,
    pub failed: AtomicBool,
    pub volume_bits: AtomicU32,
    pub flush_gen: AtomicU64,
    pub ack_gen: AtomicU64,
    pub eq_gen: AtomicU64,
    pub eq_enabled: AtomicBool,
    pub eq_gains: [AtomicU32; BANDS],
}

impl Shared {
    fn new(ring_samples: usize) -> Self {
        Shared {
            ring: Ring::new(ring_samples),
            consumed: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            volume_bits: AtomicU32::new(1.0f32.to_bits()),
            flush_gen: AtomicU64::new(0),
            ack_gen: AtomicU64::new(0),
            eq_gen: AtomicU64::new(0),
            eq_enabled: AtomicBool::new(false),
            eq_gains: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }
}

pub struct Output {
    _stream: cpal::Stream, // kept alive; dropping it stops audio
    pub shared: Arc<Shared>,
    pub rate: u32,
    pub channels: usize,
    pub device_name: String,
}

/// (all output device names, default device name)
pub fn list_devices() -> (Vec<String>, Option<String>) {
    let host = cpal::default_host();
    let default = host.default_output_device().and_then(|d| d.name().ok());
    let names = host
        .output_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect::<Vec<_>>())
        .unwrap_or_default();
    (names, default)
}

pub fn open_output(wanted: Option<&str>) -> Result<Output, String> {
    let host = cpal::default_host();
    let device = wanted
        .and_then(|name| {
            host.output_devices()
                .ok()
                .and_then(|mut it| it.find(|d| d.name().map(|n| n == name).unwrap_or(false)))
        })
        .or_else(|| host.default_output_device())
        .ok_or_else(|| "No audio output device found".to_string())?;

    let device_name = device.name().unwrap_or_else(|_| "Unknown device".into());
    
    let is_razer = device_name.to_lowercase().contains("razer") || device_name.to_lowercase().contains("7.1");

    let (config, format) = if is_razer {
        log::info!("Razer 7.1 detected! Forcing 48000Hz, 16-bit to prevent glitches.");
        (cpal::StreamConfig {
            channels: 2,
            sample_rate: cpal::SampleRate(48000),
            buffer_size: cpal::BufferSize::Default,
        }, cpal::SampleFormat::I16)
    } else {
        let supported = device.default_output_config().map_err(|e| format!("{device_name}: {e}"))?;
        (supported.config(), supported.sample_format())
    };

    let rate = config.sample_rate.0;
    let channels = config.channels as usize;
    if channels == 0 || channels > MAX_CH {
        return Err(format!("{device_name}: unsupported channel count {channels}"));
    }

    // One second of audio buffered between decoder and device absorbs decode hiccups.
    let shared = Arc::new(Shared::new(rate as usize * channels));
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, shared.clone()),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, shared.clone()),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, shared.clone()),
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, shared.clone()),
        other => Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| format!("cannot start stream: {e}"))?;
    log::info!("audio output: {device_name} @ {rate} Hz, {channels} ch, {format:?}");

    Ok(Output { _stream: stream, shared, rate, channels, device_name })
}

struct Callback {
    ch: usize,
    rate: f32,
    flush_seen: u64,
    eq_seen: u64,
    eq: Equalizer,
    vol: f32,
    pause_gain: f32,
    fade_in: f32,
}

impl Callback {
    fn render(&mut self, sh: &Shared, out: &mut [f32]) {
        let ch = self.ch;

        // Flush requested (seek / track change / stop): drop buffered audio, then acknowledge.
        let g = sh.flush_gen.load(Ordering::Acquire);
        if g != self.flush_seen {
            let dropped = sh.ring.discard_all();
            sh.consumed.fetch_add((dropped / ch) as u64, Ordering::AcqRel);
            self.flush_seen = g;
            self.fade_in = 0.0;
            self.eq.reset();
            sh.ack_gen.store(g, Ordering::Release);
        }

        // EQ settings changed.
        let eg = sh.eq_gen.load(Ordering::Acquire);
        if eg != self.eq_seen {
            let mut gains = [0f32; BANDS];
            for (i, gv) in gains.iter_mut().enumerate() {
                *gv = f32::from_bits(sh.eq_gains[i].load(Ordering::Relaxed));
            }
            self.eq.configure(self.rate, sh.eq_enabled.load(Ordering::Relaxed), &gains);
            self.eq_seen = eg;
        }

        let paused = sh.paused.load(Ordering::Relaxed);
        if paused && self.pause_gain <= 0.0 {
            out.fill(0.0);
            return;
        }

        let got = sh.ring.pop(out);
        let got_frames = got / ch;
        out[got..].fill(0.0); // underrun -> silence, never garbage
        sh.consumed.fetch_add(got_frames as u64, Ordering::AcqRel);

        let target_vol = f32::from_bits(sh.volume_bits.load(Ordering::Relaxed));
        let pause_step = 1.0 / (0.02 * self.rate); // 20 ms ramp
        let fade_step = 1.0 / 256.0;

        for (f, frame) in out.chunks_exact_mut(ch).enumerate() {
            if f < got_frames {
                self.eq.process_frame(frame);
            }
            self.vol += (target_vol - self.vol) * 0.002;
            self.pause_gain = if paused { (self.pause_gain - pause_step).max(0.0) } else { (self.pause_gain + pause_step).min(1.0) };
            self.fade_in = (self.fade_in + fade_step).min(1.0);
            let gain = self.vol * self.pause_gain * self.fade_in;
            for s in frame.iter_mut() {
                *s = (*s * gain).clamp(-1.0, 1.0);
            }
        }
    }
}

fn build<T>(device: &cpal::Device, config: &cpal::StreamConfig, shared: Arc<Shared>) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let ch = config.channels as usize;
    let rate = config.sample_rate.0 as f32;
    let mut cb = Callback {
        ch,
        rate,
        flush_seen: 0,
        eq_seen: 0,
        eq: Equalizer::new(ch),
        vol: f32::from_bits(shared.volume_bits.load(Ordering::Relaxed)),
        pause_gain: 1.0,
        fade_in: 1.0,
    };
    let mut scratch: Vec<f32> = vec![0.0; 16384];
    let sh_cb = shared.clone();
    let sh_err = shared;

    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if scratch.len() < data.len() {
                    scratch.resize(data.len(), 0.0);
                }
                let buf = &mut scratch[..data.len()];
                cb.render(&sh_cb, buf);
                for (o, s) in data.iter_mut().zip(buf.iter()) {
                    *o = T::from_sample(*s);
                }
            },
            move |e| {
                log::error!("audio stream error: {e}");
                sh_err.failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|e| format!("cannot open stream: {e}"))
}

// Keep the trait imports used on every platform.
#[allow(dead_code)]
fn _assert_sample<T: Sample>() {}