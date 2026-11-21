//! Parallel harmonic exciter with smoothed controls and a stereo-linked limiter.
//!
//! `process_frame` accepts one interleaved frame or a buffer of interleaved
//! frames. Intensity is in [0, 1]. Construction allocates; processing and
//! parameter changes do not. Rebuild the processor when the format changes.
//!
//! Nonlinear processing runs at 4x the input rate, between windowed-sinc FIR
//! filters. These filters roll off the band immediately below Nyquist. The
//! output is delayed by `latency_samples()`, including at zero intensity.
//! The limiter remains active at zero intensity. All state is audio-thread
//! owned; deliver parameter changes to that thread rather than sharing it
//! concurrently with a UI thread.

#![allow(dead_code)]


use std::f64::consts::PI;

const OVERSAMPLE: usize = 4;
const FIR_TAPS: usize = 193;
const UP_TAPS: usize = (FIR_TAPS + OVERSAMPLE - 1) / OVERSAMPLE;
const FIR_LATENCY: usize = (FIR_TAPS - 1) / OVERSAMPLE;
const LIMITER_KNEE: f64 = 0.88;
const LIMITER_CEILING: f64 = 0.98;


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrystalizerMode {
    Crystalizer,
    CrystalizerPlus,
    CrystalizerHx,
    CrystalizerHxPlus,
}

impl Default for CrystalizerMode {
    fn default() -> Self {
        Self::Crystalizer
    }
}

#[derive(Clone, Copy)]
struct BiquadCoefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl BiquadCoefficients {
    fn butterworth(sample_rate: f64, cutoff: f64, high_pass: bool) -> Self {
        let omega = 2.0 * PI * cutoff / sample_rate;
        let (sin, cos) = omega.sin_cos();
        let alpha = sin * std::f64::consts::FRAC_1_SQRT_2;
        let inverse_a0 = 1.0 / (1.0 + alpha);
        let b0 = if high_pass {
            0.5 * (1.0 + cos) * inverse_a0
        } else {
            0.5 * (1.0 - cos) * inverse_a0
        };
        Self {
            b0,
            b1: if high_pass { -2.0 * b0 } else { 2.0 * b0 },
            b2: b0,
            a1: -2.0 * cos * inverse_a0,
            a2: (1.0 - alpha) * inverse_a0,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct BiquadState {
    z1: f64,
    z2: f64,
}

#[inline]
fn flush_tiny(value: f64) -> f64 {
    // Prevent denormals without modifying the audio thread's floating-point mode.
    if value.abs() < 1.0e-30 { 0.0 } else { value }
}

impl BiquadState {
    #[inline]
    fn process(&mut self, input: f64, c: &BiquadCoefficients) -> f64 {
        let output = c.b0 * input + self.z1;
        self.z1 = flush_tiny(c.b1 * input - c.a1 * output + self.z2);
        self.z2 = flush_tiny(c.b2 * input - c.a2 * output);
        output
    }
}

struct CrossoverCoefficients {
    low: BiquadCoefficients,
    high: BiquadCoefficients,
}

impl CrossoverCoefficients {
    fn new(sample_rate: f64, cutoff: f64) -> Self {
        Self {
            low: BiquadCoefficients::butterworth(sample_rate, cutoff, false),
            high: BiquadCoefficients::butterworth(sample_rate, cutoff, true),
        }
    }
}

#[derive(Default)]
struct CrossoverState {
    low: [BiquadState; 2],
    high: [BiquadState; 2],
}

impl CrossoverState {
    #[inline]
    fn split(&mut self, input: f64, c: &CrossoverCoefficients) -> (f64, f64) {
        // LR4: cascade two identical second-order Butterworth sections per branch.
        // Both branches have the same polarity and sum to a unity-magnitude allpass.
        let low = self.low[0].process(input, &c.low);
        let low = self.low[1].process(low, &c.low);
        let high = self.high[0].process(input, &c.high);
        let high = self.high[1].process(high, &c.high);
        (low, high)
    }
}

struct History<const N: usize> {
    samples: [f64; N],
    next: usize,
}

impl<const N: usize> History<N> {
    fn new() -> Self {
        Self { samples: [0.0; N], next: 0 }
    }

    #[inline]
    fn push(&mut self, sample: f64) {
        self.samples[self.next] = sample;
        self.next += 1;
        if self.next == N {
            self.next = 0;
        }
    }

    #[inline]
    fn convolve(&self, coefficients: &[f64; N]) -> f64 {
        // Two contiguous reverse walks avoid a modulo operation for every tap.
        let mut sum = 0.0;
        for (coefficient, sample) in coefficients.iter().zip(
            self.samples[..self.next].iter().rev()
                .chain(self.samples[self.next..].iter().rev()),
        ) {
            sum += coefficient * sample;
        }
        sum
    }
}

#[inline]
pub(super) fn zr_soft_knee_gain(peak: f32) -> f32 {
    const KNEE: f32 = 0.85;
    const CEILING: f32 = 0.999;
    const SPAN: f32 = CEILING - KNEE;

    if peak <= KNEE {
        return 1.0;
    }

    // C1-continuous sample-peak curve with an asymptotic ceiling.
    // The caller applies this shared gain to every channel in the frame.
    let excess = peak - KNEE;
    let limited_peak = KNEE + SPAN * (excess / (SPAN + excess));
    limited_peak / peak
}

#[derive(Default)]
struct TransientEnvelope {
    fast: f64,
    slow: f64,
}

impl TransientEnvelope {
    #[inline]
    fn process(&mut self, input: f64, c: &DspCoefficients) -> f64 {
        let level = input.abs();
        let fast_rate = if level > self.fast { c.fast_attack } else { c.release };
        let slow_rate = if level > self.slow { c.slow_attack } else { c.release };
        self.fast = flush_tiny(self.fast + fast_rate * (level - self.fast));
        self.slow = flush_tiny(self.slow + slow_rate * (level - self.slow));
        // Modulate the original waveform, never a signum or rectified waveform.
        ((self.fast - self.slow) / (self.fast + 1.0e-9)).max(0.0)
    }
}

struct DspCoefficients {
    high_pass: BiquadCoefficients,
    low_split: CrossoverCoefficients,
    high_split: CrossoverCoefficients,
    fast_attack: f64,
    slow_attack: f64,
    release: f64,
    saturation_normalization: [f64; 4],
}

const DRIVES: [f64; 4] = [2.0, 1.15, 1.65, 2.2];

#[inline]
fn harmonic_residual(input: f64, index: usize, c: &DspCoefficients) -> f64 {
    // Odd, smooth and bounded saturation. Subtract the unprocessed band so the
    // parallel path contains only the excitation, not a second copy of the band.
    (DRIVES[index] * input).tanh() * c.saturation_normalization[index] - input
}

struct ChannelState {
    interpolator: History<UP_TAPS>,
    decimator: History<FIR_TAPS>,
    high_pass: BiquadState,
    low_split: CrossoverState,
    high_split: CrossoverState,
    low_phase_alignment: CrossoverState,
    standard_envelope: TransientEnvelope,
    hx_envelope: TransientEnvelope,
    delay: Box<[f64]>,
}

impl ChannelState {
    fn new(delay_length: usize) -> Self {
        Self {
            interpolator: History::new(),
            decimator: History::new(),
            high_pass: BiquadState::default(),
            low_split: CrossoverState::default(),
            high_split: CrossoverState::default(),
            low_phase_alignment: CrossoverState::default(),
            standard_envelope: TransientEnvelope::default(),
            hx_envelope: TransientEnvelope::default(),
            delay: vec![0.0; delay_length].into_boxed_slice(),
        }
    }

    #[inline]
    fn excite(
        &mut self,
        input: f64,
        intensity: f64,
        hx_mix: f64,
        plus_mix: f64,
        c: &DspCoefficients,
    ) -> f64 {
        // Always advance every linear filter and envelope. A mode change must
        // not expose cold filter histories or reset ringing filter states.
        let high_passed = self.high_pass.process(input, &c.high_pass);
        let (low, above_low) = self.low_split.split(input, &c.low_split);
        let (mid, high) = self.high_split.split(above_low, &c.high_split);

        // The upper crossover contributes an allpass to mid + high. Apply that
        // same allpass to low; otherwise a three-way LR4 tree is not coherent.
        let (aligned_low_lp, aligned_low_hp) =
            self.low_phase_alignment.split(low, &c.high_split);
        let low = aligned_low_lp + aligned_low_hp;
        let standard_transient = self.standard_envelope.process(high_passed, c);
        let hx_transient = self.hx_envelope.process(high, c);

        if intensity == 0.0 {
            return input;
        }

        let mut correction = 0.0;
        if hx_mix < 1.0 {
            let standard = 0.32 * harmonic_residual(high_passed, 0, c)
                + plus_mix * 0.16 * standard_transient * high_passed;
            correction += (1.0 - hx_mix) * standard;
        }
        if hx_mix > 0.0 {
            let multiband = 0.10 * harmonic_residual(low, 1, c)
                + 0.18 * harmonic_residual(mid, 2, c)
                + 0.32 * harmonic_residual(high, 3, c)
                + plus_mix * 0.20 * hx_transient * high;
            correction += hx_mix * multiband;
        }
        input + intensity * correction
    }
}

/// Real-time, multichannel harmonic enhancement with linked peak protection.
///
/// All channels share control ramps and limiter gain, preserving the stereo
/// image. Channel histories are independent. No fixed maximum channel count
/// or allocation is imposed on the processing callback.
pub struct Crystalizer {
    states: Box<[ChannelState]>,
    mode: CrystalizerMode,
    target_intensity: f64,
    intensity: f64,
    hx_mix: f64,
    plus_mix: f64,
    parameter_rate: f64,
    mode_rate: f64,
    coefficients: DspCoefficients,
    interpolation: [[f64; UP_TAPS]; OVERSAMPLE],
    decimation: [f64; FIR_TAPS],
    limiter_targets: Box<[f64]>,
    constrained_targets: usize,
    preview_weights: Box<[f64]>,
    delay_position: usize,
    limiter_gain: f64,
    limiter_release: f64,
}

impl Crystalizer {
    /// Invalid or unsupported rates use 48 kHz. Supported rates are 8-384 kHz.
    /// Zero channels create an inert processor.
    pub fn new(sample_rate: f32, channels: usize) -> Self {
        let sample_rate = if sample_rate.is_finite()
            && (8_000.0..=384_000.0).contains(&sample_rate)
        {
            sample_rate as f64
        } else {
            48_000.0
        };
        let internal_rate = sample_rate * OVERSAMPLE as f64;
        let lookahead = (0.003 * sample_rate).ceil() as usize;
        let delay_length = lookahead + 1;
        let (interpolation, decimation) = make_resampling_filters();
        let mut normalization = [0.0; 4];
        for (normalization, drive) in normalization.iter_mut().zip(DRIVES.iter()) {
            *normalization = 1.0 / drive.tanh();
        }
        let coefficients = DspCoefficients {
            high_pass: BiquadCoefficients::butterworth(internal_rate, 1_700.0, true),
            low_split: CrossoverCoefficients::new(internal_rate, 300.0),
            high_split: CrossoverCoefficients::new(internal_rate, 3_000.0),
            fast_attack: smoothing_rate(0.0015, internal_rate),
            slow_attack: smoothing_rate(0.030, internal_rate),
            release: smoothing_rate(0.060, internal_rate),
            saturation_normalization: normalization,
        };
        let preview_weights = (0..delay_length)
            .map(|distance| {
                // A future peak starts with zero influence and reaches its full
                // gain constraint with zero slope when it reaches the output.
                0.5 + 0.5 * (PI * distance as f64 / lookahead as f64).cos()
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            states: (0..channels)
                .map(|_| ChannelState::new(delay_length))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            mode: CrystalizerMode::Crystalizer,
            target_intensity: 0.0,
            intensity: 0.0,
            hx_mix: 0.0,
            plus_mix: 0.0,
            parameter_rate: smoothing_rate(0.025, sample_rate),
            mode_rate: smoothing_rate(0.035, sample_rate),
            coefficients,
            interpolation,
            decimation,
            limiter_targets: vec![1.0; delay_length].into_boxed_slice(),
            constrained_targets: 0,
            preview_weights,
            delay_position: 0,
            limiter_gain: 1.0,
            limiter_release: smoothing_rate(0.080, sample_rate),
        }
    }

    /// Crossfade modes without resetting the continuously running DSP paths.
    pub fn set_mode(&mut self, mode: CrystalizerMode) {
        self.mode = mode;
    }

    /// Set normalized intensity. Non-finite values are treated as zero.
    pub fn set_intensity(&mut self, intensity: f32) {
        self.target_intensity = if intensity.is_finite() {
            intensity.clamp(0.0, 1.0) as f64
        } else {
            0.0
        };
    }

    /// Fixed input-rate latency in frames, not interleaved samples.
    pub fn latency_samples(&self) -> usize {
        FIR_LATENCY + self.limiter_targets.len() - 1
    }

    /// Process one frame or an interleaved buffer in place without allocating.
    ///
    /// Smoothing advances once per frame, not once per channel or callback.
    /// A trailing partial frame is zero-padded internally. Non-finite input
    /// samples are replaced with silence before they enter any filter state.
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        let channels = self.states.len();
        if channels == 0 {
            return;
        }
        for frame in frame.chunks_mut(channels) {
            let (target_hx, target_plus) = match self.mode {
                CrystalizerMode::Crystalizer => (0.0, 0.0),
                CrystalizerMode::CrystalizerPlus => (0.0, 1.0),
                CrystalizerMode::CrystalizerHx => (1.0, 0.0),
                CrystalizerMode::CrystalizerHxPlus => (1.0, 1.0),
            };
            self.intensity = smooth(self.intensity, self.target_intensity, self.parameter_rate);
            self.hx_mix = smooth(self.hx_mix, target_hx, self.mode_rate);
            self.plus_mix = smooth(self.plus_mix, target_plus, self.mode_rate);

            let write = self.delay_position;
            let mut peak = 0.0_f64;
            for (channel, state) in self.states.iter_mut().enumerate() {
                let input = frame.get(channel).copied().unwrap_or(0.0);
                let input = if input.is_finite() { input as f64 } else { 0.0 };
                state.interpolator.push(input);
                let mut output = 0.0;
                for phase in 0..OVERSAMPLE {
                    let upsampled = state.interpolator.convolve(&self.interpolation[phase]);
                    let enhanced = state.excite(
                        upsampled,
                        self.intensity,
                        self.hx_mix,
                        self.plus_mix,
                        &self.coefficients,
                    );
                    state.decimator.push(enhanced);
                    // Phase zero gives an integer input-rate FIR delay.
                    if phase == 0 {
                        output = state.decimator.convolve(&self.decimation);
                    }
                }
                state.delay[write] = output;
                peak = peak.max(output.abs());
            }

            if self.limiter_targets[write] < 1.0 {
                self.constrained_targets -= 1;
            }
            let target_gain = limiter_target_gain(peak);
            self.limiter_targets[write] = target_gain;
            if target_gain < 1.0 {
                self.constrained_targets += 1;
            }
            let read = if write + 1 == self.limiter_targets.len() { 0 } else { write + 1 };
            let gain = self.preview_limiter_gain(read);
            self.limiter_gain = gain;
            for (sample, state) in frame.iter_mut().zip(self.states.iter()) {
                *sample = flush_tiny(state.delay[read] * gain) as f32;
            }
            self.delay_position = read;
        }
    }

    #[inline]
    fn preview_limiter_gain(&self, read: usize) -> f64 {
        // Look ahead at all pending peaks with a raised-cosine attack. Unlike
        // an instantaneous waveshaper, this applies a smooth, shared gain to
        // the delayed waveform. The oldest constraint is always applied in
        // full, so no sample can exceed the ceiling, even on abrupt overloads.
        let mut gain = self.limiter_gain + self.limiter_release * (1.0 - self.limiter_gain);
        if self.constrained_targets == 0 {
            return gain;
        }
        let targets = self.limiter_targets[read..].iter()
            .chain(self.limiter_targets[..read].iter());
        for (&target, &weight) in targets.zip(self.preview_weights.iter()) {
            if target < 1.0 {
                let constraint = (1.0 - weight) + weight * target;
                gain = gain.min(constraint);
            }
        }
        gain
    }
}

#[inline]
fn smoothing_rate(seconds: f64, sample_rate: f64) -> f64 {
    -(-1.0 / (seconds * sample_rate)).exp_m1()
}

#[inline]
fn smooth(current: f64, target: f64, rate: f64) -> f64 {
    let next = current + rate * (target - current);
    if (next - target).abs() < 1.0e-12 { target } else { next }
}

#[inline]
fn limiter_target_gain(peak: f64) -> f64 {
    if peak <= LIMITER_KNEE {
        1.0
    } else {
        // Unity below the knee, with continuous first and second derivatives
        // at the knee and an asymptotic ceiling. No hard waveform clipping.
        let width = LIMITER_CEILING - LIMITER_KNEE;
        let limited_peak = LIMITER_KNEE + width * ((peak - LIMITER_KNEE) / width).tanh();
        limited_peak / peak
    }
}

fn bessel_i0(x: f64) -> f64 {
    let y = x * x * 0.25;
    let mut sum = 1.0;
    let mut term = 1.0;
    for k in 1..=40 {
        term *= y / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1.0e-16 {
            break;
        }
    }
    sum
}

fn make_resampling_filters() -> ([[f64; UP_TAPS]; OVERSAMPLE], [f64; FIR_TAPS]) {
    let mut fir = [0.0; FIR_TAPS];
    let midpoint = (FIR_TAPS - 1) as f64 * 0.5;
    let cutoff = 0.5 / OVERSAMPLE as f64;
    let beta = 8.6;
    let inverse_window_scale = 1.0 / bessel_i0(beta);
    let mut sum = 0.0;
    for (index, tap) in fir.iter_mut().enumerate() {
        let offset = index as f64 - midpoint;
        let sinc = if offset == 0.0 {
            2.0 * cutoff
        } else {
            (2.0 * PI * cutoff * offset).sin() / (PI * offset)
        };
        let position = offset / midpoint;
        let window = bessel_i0(beta * (1.0 - position * position).max(0.0).sqrt())
            * inverse_window_scale;
        *tap = sinc * window;
        sum += *tap;
    }
    for tap in &mut fir {
        *tap /= sum;
    }
    let mut interpolation = [[0.0; UP_TAPS]; OVERSAMPLE];
    for (phase, coefficients) in interpolation.iter_mut().enumerate() {
        for (index, coefficient) in coefficients.iter_mut().enumerate() {
            let tap = phase + index * OVERSAMPLE;
            if tap < FIR_TAPS {
                // Compensate the zero-insertion gain in the interpolator only.
                *coefficient = fir[tap] * OVERSAMPLE as f64;
            }
        }
    }
    (interpolation, fir)
}

