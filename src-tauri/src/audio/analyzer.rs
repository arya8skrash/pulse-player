//! Allocation-free audio ingestion for offline BS.1770-4 loudness analysis.
//!
//! Only complete 400 ms blocks are measured; trailing partial blocks are ignored.
//! The ReplayGain target is -18 LUFS. Peak reporting is sample peak, not true peak.

#![allow(dead_code)]

use std::f64::consts::PI;

const TARGET_LUFS: f64 = -18.0;
const LOUDNESS_OFFSET: f64 = -0.691;
// 10^((-70 - LOUDNESS_OFFSET) / 10).
const ABSOLUTE_GATE_ENERGY: f64 = 1.172_465_304_582_298_1e-7;
const DEFAULT_BLOCK_CAPACITY: usize = 24 * 60 * 60 * 10;
const SURROUND_WEIGHT: f64 = 1.41;

/// Offline ReplayGain analyzer using K-weighting and two-stage loudness gating.
///
/// Default interleaved channel layouts:
/// - 1: mono
/// - 2: L, R
/// - 3: L, R, C
/// - 4: L, R, Ls, Rs
/// - 5: L, R, C, Ls, Rs
/// - 6: L, R, C, LFE, Ls, Rs
/// - 7: L, R, C, LFE, back-center, Ls, Rs
/// - 8: L, R, C, LFE, back-left, back-right, Ls, Rs
///
/// Ls/Rs are assumed to lie between 60 and 120 degrees in azimuth and receive
/// an energy weight of 1.41. Back channels beyond 120 degrees receive 1.0, as
/// specified by BS.1770-4's position-dependent weighting. LFE receives 0.0.
/// Other channel counts default to unit weights. Use `set_channel_weights`
/// whenever the actual channel layout differs from these assumptions.
///
/// Exact gating requires retained block energies. `new` reserves approximately
/// 24 hours of blocks; use `with_capacity` for a different maximum duration.
/// Storage exhaustion panics instead of allocating or silently losing blocks.
/// `reset` preserves all allocated storage.
pub struct ReplayGainAnalyzer {
    channels: Vec<ChannelState>,
    block_energies: Vec<f64>,
    peak: f32,
    current_channel: usize,

    // Four consecutive hop energies constitute one complete 400 ms block.
    hop_energies: [f64; 4],
    hop_cursor: usize,
    completed_hops: usize,
    current_hop_energy: f64,

    // u128 accommodates the frame count of every finite f32 sample rate.
    block_frames: u128,
    hop_base_frames: u128,
    hop_extra_frames: u8,
    hop_phase: u8,
    frames_remaining_in_hop: u128,
}

impl ReplayGainAnalyzer {
    /// Creates an analyzer with approximately 24 hours of block storage.
    ///
    /// Panics if the sample rate is non-finite or below 8000 Hz, or if there
    /// are no channels.
    pub fn new(sample_rate: f32, channels: usize) -> Self {
        Self::with_capacity(sample_rate, channels, DEFAULT_BLOCK_CAPACITY)
    }

    /// Creates an analyzer with an explicit maximum number of retained blocks.
    ///
    /// Reserving `ceil(duration_seconds * 10)` entries is sufficient at normal
    /// audio sample rates. Only blocks above the absolute gate consume entries.
    /// All heap allocation happens here, never during successful ingestion.
    pub fn with_capacity(sample_rate: f32, channels: usize, block_capacity: usize) -> Self {
        assert!(
            sample_rate.is_finite() && sample_rate >= 8_000.0,
            "sample_rate must be finite and at least 8000 Hz"
        );
        assert!(channels > 0, "channels must be greater than zero");
        assert!(block_capacity > 0, "block_capacity must be greater than zero");

        let rate = f64::from(sample_rate);
        let shelf = Biquad::high_shelf(rate);
        let high_pass = Biquad::high_pass(rate);
        let mut channel_states = Vec::with_capacity(channels);
        for index in 0..channels {
            channel_states.push(ChannelState {
                shelf,
                high_pass,
                weight: default_channel_weight(channels, index),
            });
        }

        // Round the 400 ms window once. Fractional quarter-window hops are
        // distributed across four hops, so every window has exactly this size.
        let block_frames = (rate * 0.4).round() as u128;
        let mut analyzer = Self {
            channels: channel_states,
            block_energies: Vec::with_capacity(block_capacity),
            peak: 0.0,
            current_channel: 0,
            hop_energies: [0.0; 4],
            hop_cursor: 0,
            completed_hops: 0,
            current_hop_energy: 0.0,
            block_frames,
            hop_base_frames: block_frames / 4,
            hop_extra_frames: (block_frames % 4) as u8,
            hop_phase: 2,
            frames_remaining_in_hop: 0,
        };
        analyzer.schedule_next_hop();
        analyzer
    }

    /// Sets per-channel energy weights and resets the current measurement.
    ///
    /// Use 1.0 for front/back/height channels, 1.41 for side surrounds, and 0.0
    /// for LFE, according to the actual loudspeaker positions in BS.1770-4.
    /// Peak measurement always includes every channel, even channels weighted 0.
    /// This method preserves allocated storage and performs no allocation.
    pub fn set_channel_weights(&mut self, weights: &[f32]) {
        assert!(
            weights.len() == self.channels.len(),
            "one energy weight is required for each channel"
        );
        assert!(
            weights.iter().all(|weight| weight.is_finite() && *weight >= 0.0),
            "channel weights must be finite and nonnegative"
        );

        self.reset();
        for (channel, &weight) in self.channels.iter_mut().zip(weights.iter()) {
            channel.weight = f64::from(weight);
        }
    }

    /// Processes interleaved floating-point PCM without allocating.
    ///
    /// Buffer boundaries may split a channel frame; channel position is carried
    /// into the next call. Non-finite input samples are treated as zero. Finite
    /// samples are not clipped to [-1, 1]. No zero-padding or filter-tail flushing
    /// is performed, and incomplete loudness blocks are not measured.
    ///
    /// Panics if the preallocated block storage is exhausted. Reserve sufficient
    /// capacity with `with_capacity` before starting a longer scan.
    pub fn push_frames(&mut self, frame: &[f32]) {
        for &input in frame {
            let sample = if input.is_finite() { input } else { 0.0 };
            self.peak = self.peak.max(sample.abs());

            let channel = &mut self.channels[self.current_channel];
            if channel.weight != 0.0 {
                let shelved = channel.shelf.process(sample);
                let weighted = f64::from(channel.high_pass.process(shelved));
                // Keep the IIR signal path in f32, but accumulate power in f64.
                self.current_hop_energy += channel.weight * weighted * weighted;
            }

            self.current_channel += 1;
            if self.current_channel == self.channels.len() {
                self.current_channel = 0;
                self.frames_remaining_in_hop -= 1;
                if self.frames_remaining_in_hop == 0 {
                    self.finish_hop();
                }
            }
        }
    }

    /// Returns -18.0 minus integrated loudness, in dB.
    ///
    /// Returns neutral gain (0 dB) if there is no complete block above the
    /// absolute gate, including silence and input shorter than 400 ms.
    /// This is loudness gain only: it is not limited using the sample peak.
    pub fn get_gain(&self) -> f32 {
        if self.block_energies.is_empty() {
            return 0.0;
        }

        // Blocks were absolute-gated during ingestion. Average linear energy,
        // never dB values, to derive a relative gate 10 LU below that mean.
        let absolute_sum: f64 = self.block_energies.iter().copied().sum();
        let absolute_mean = absolute_sum / self.block_energies.len() as f64;
        let gate = ABSOLUTE_GATE_ENERGY.max(absolute_mean * 0.1);

        let mut gated_sum = 0.0;
        let mut gated_count = 0usize;
        for &energy in &self.block_energies {
            if energy > gate {
                gated_sum += energy;
                gated_count += 1;
            }
        }

        if gated_count == 0 {
            return 0.0;
        }

        let integrated_energy = gated_sum / gated_count as f64;
        let measured_lufs = LOUDNESS_OFFSET + 10.0 * integrated_energy.log10();
        (TARGET_LUFS - measured_lufs) as f32
    }

    /// Returns the maximum absolute, unfiltered sample amplitude across channels.
    ///
    /// This is not an oversampled true-peak estimate; intersample peaks can exceed
    /// it. No gain is applied to this value, and LFE samples are included.
    pub fn get_peak(&self) -> f32 {
        self.peak
    }

    /// Clears audio history while retaining configuration and allocated capacity.
    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.shelf.reset();
            channel.high_pass.reset();
        }
        self.block_energies.clear();
        self.peak = 0.0;
        self.current_channel = 0;
        self.hop_energies = [0.0; 4];
        self.hop_cursor = 0;
        self.completed_hops = 0;
        self.current_hop_energy = 0.0;
        self.hop_phase = 2;
        self.schedule_next_hop();
    }

    fn schedule_next_hop(&mut self) {
        // Integer phase accumulation rounds n * block_frames / 4 without drift.
        // This also handles rates such as 11025 Hz, with alternating hop sizes.
        let phase = self.hop_phase + self.hop_extra_frames;
        self.frames_remaining_in_hop = self.hop_base_frames + u128::from(phase / 4);
        self.hop_phase = phase % 4;
    }

    fn finish_hop(&mut self) {
        self.hop_energies[self.hop_cursor] = self.current_hop_energy;
        self.current_hop_energy = 0.0;
        self.hop_cursor = (self.hop_cursor + 1) % 4;
        if self.completed_hops < 4 {
            self.completed_hops += 1;
        }

        if self.completed_hops == 4 {
            // Sum the four terms afresh to avoid rolling-subtraction drift.
            let energy = self.hop_energies.iter().copied().sum::<f64>()
                / self.block_frames as f64;
            if energy > ABSOLUTE_GATE_ENERGY {
                // A guarded push cannot trigger Vec reallocation. Do not silently
                // discard old blocks: doing so would bias the relative gate.
                assert!(
                    self.block_energies.len() < self.block_energies.capacity(),
                    "ReplayGain block capacity exhausted; use with_capacity before scanning"
                );
                self.block_energies.push(energy);
            }
        }
        self.schedule_next_hop();
    }
}

struct ChannelState {
    shelf: Biquad,
    high_pass: Biquad,
    weight: f64,
}

fn default_channel_weight(channels: usize, index: usize) -> f64 {
    match channels {
        4 if index >= 2 => SURROUND_WEIGHT,
        5 if index >= 3 => SURROUND_WEIGHT,
        6..=8 if index == 3 => 0.0,
        6 if index >= 4 => SURROUND_WEIGHT,
        7 if index >= 5 => SURROUND_WEIGHT,
        8 if index >= 6 => SURROUND_WEIGHT,
        _ => 1.0,
    }
}

/// Second-order IIR filter in a trapezoidal-integrator (TPT) realization.
///
/// This is algebraically equivalent to a bilinear-transform biquad, but avoids
/// subtracting nearly equal denominator coefficients at high sample rates.
/// Coefficient design uses f64; all coefficients, states and sample arithmetic
/// in the processing path use f32.
#[derive(Clone, Copy)]
struct Biquad {
    a1: f32,
    a2: f32,
    a3: f32,
    m0: f32,
    m1: f32,
    m2: f32,
    s1: f32,
    s2: f32,
}

impl Biquad {
    fn high_shelf(sample_rate: f64) -> Self {
        // De Man parameterization reproduces the BS.1770 reference coefficients
        // at 48 kHz while allowing a bilinear redesign at other sample rates.
        let frequency = 1_681.974_450_955_533;
        let q = 0.707_175_236_955_419_6;
        let gain_db = 3.999_843_853_973_347;
        let g = (PI * frequency / sample_rate).tan();
        let vh = 10.0_f64.powf(gain_db / 20.0);
        let vb = vh.powf(0.499_666_774_154_541_6);
        Self::from_analog(g, q, vh, vb / q, 1.0)
    }

    fn high_pass(sample_rate: f64) -> Self {
        let frequency = 38.135_470_876_024_44;
        let q = 0.500_327_037_323_877_3;
        let g = (PI * frequency / sample_rate).tan();
        // RLB uses digital numerator [1, -2, 1], not a unity-Nyquist-gain HPF.
        // Undo the usual bilinear numerator normalization to match that filter.
        let normalization = 1.0 + g / q + g * g;
        Self::from_analog(g, q, normalization, 0.0, 0.0)
    }

    /// Bilinear transform of (high*s^2 + band*s + low)/(s^2 + s/Q + 1),
    /// with prewarped integrator coefficient g = tan(pi * frequency / rate).
    fn from_analog(g: f64, q: f64, high: f64, band: f64, low: f64) -> Self {
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        Self {
            a1: a1 as f32,
            a2: a2 as f32,
            a3: a3 as f32,
            m0: high as f32,
            m1: (band - high * k) as f32,
            m2: (low - high) as f32,
            s1: 0.0,
            s2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, sample: f32) -> f32 {
        let v3 = sample - self.s2;
        let v1 = self.a1.mul_add(self.s1, self.a2 * v3);
        let v2 = self.a2.mul_add(self.s1, self.a3.mul_add(v3, self.s2));
        self.s1 = flush_tiny(2.0 * v1 - self.s1);
        self.s2 = flush_tiny(2.0 * v2 - self.s2);
        self.m0.mul_add(sample, self.m1.mul_add(v1, self.m2 * v2))
    }

    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }
}

#[inline]
fn flush_tiny(value: f32) -> f32 {
    // Suppress denormal slowdowns far below any relevant loudness threshold.
    if value.abs() < 1.0e-30 {
        0.0
    } else {
        value
    }
}
