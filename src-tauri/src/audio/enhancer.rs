//! Independent, patent-informed bass/treble enhancement and stereo widening.
//!
//! Sources: US8582784B2 (low-band gain -> hard clipping -> low-pass filtering)
//! and US20110002467A1 (band enhancement controlled by headroom estimated using
//! the previous enhancement gain). This is an independent implementation, not
//! proprietary Zirene/AM3D code, an equivalence claim, or patent clearance.
//!
//! All amounts are in [0, 1]; defaults are neutral. Bass reaches approximately
//! +12 dB at low signal levels; treble selects a nominal gain of 0..20 dB.
//! Fixed-pole shelving bases avoid crossover notches; their small shoulders
//! can exceed the nominal gain by less than approximately 0.25 dB.
//! Width maps 0..1 to a side gain of 1..1.5. Parameters ramp over 20 ms.
//! M/S preserves the mono sum before limiting, but can strengthen existing
//! anti-correlated material; it is not a phase-correction algorithm.
//! Frequencies, time constants, complementary splits, and M/S are engineering
//! choices, not values claimed to reproduce a particular commercial product.
//!
//! Input is normalized, interleaved f32 PCM. Construct off the audio thread;
//! processing/reset/parameter updates do not allocate. Use exclusive ownership
//! on the audio thread; cross-thread parameter transport belongs to the caller.
//!
//! ZR-Surround adds stereo-only, shelf-filtered delayed differential crossfeed.
//! ZR-Booster adds a smoothed +4.5 dB preamp with linked soft-knee protection.
//! For each chunk, call process_preamp before Crystalizer, process_frame after
//! Crystalizer, then protect_output after downstream virtualization.

#![allow(dead_code)]

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use super::crystalizer::zr_soft_knee_gain;

const BOOSTER_GAIN: f32 = 1.678_804; // +4.5 dB.

const BASS_HZ: f64 = 120.0;
const TREBLE_HZ: f64 = 4_000.0;
const BASS_MAX_GAIN: f32 = 3.981_071_7; // +12 dB before clipping/mixing.
const BASS_CLIP: f32 = 1.0;
const GC_CEILING: f32 = 0.95;
const INTERNAL_LIMIT: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
struct Coefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Coefficients {
    // LP/HP plus half the matched band-pass produces a fixed-pole shelf basis.
    // Unlike simply adding a second-order LP/HP to dry, boosting this basis
    // does not produce a destructive crossover notch. All filters are biquads.
    fn shelving_basis(sample_rate: f32, cutoff: f64, high_pass: bool) -> Self {
        let mut c = Self::butterworth(sample_rate, cutoff, high_pass);
        let half_band_pass = 0.25 * (1.0 - c.a2);
        c.b0 += half_band_pass;
        c.b2 -= half_band_pass;
        c
    }

    // RBJ Butterworth biquad. Design in f64, process/store samples in f32.
    fn butterworth(sample_rate: f32, cutoff: f64, high_pass: bool) -> Self {
        let cutoff = cutoff.min(0.45 * sample_rate as f64);
        let w = 2.0 * PI * cutoff / sample_rate as f64;
        let (sin_w, cos_w) = w.sin_cos();
        let alpha = sin_w / (2.0 * FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        let (b0, b1) = if high_pass {
            ((1.0 + cos_w) * 0.5, -(1.0 + cos_w))
        } else {
            ((1.0 - cos_w) * 0.5, 1.0 - cos_w)
        };
        Self {
            b0: (b0 / a0) as f32,
            b1: (b1 / a0) as f32,
            b2: (b0 / a0) as f32,
            a1: (-2.0 * cos_w / a0) as f32,
            a2: ((1.0 - alpha) / a0) as f32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Biquad {
    c: Coefficients,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn new(c: Coefficients) -> Self {
        Self {
            c,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        // Transposed direct form II. Never share these states between channels.
        let y = self.c.b0 * x + self.z1;
        self.z1 = flush_tiny(self.c.b1 * x - self.c.a1 * y + self.z2);
        self.z2 = flush_tiny(self.c.b2 * x - self.c.a2 * y);
        flush_tiny(y)
    }

    fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[inline]
fn flush_tiny(x: f32) -> f32 {
    // Suppress inaudible tails before they become CPU-expensive subnormals.
    if x.abs() < 1.0e-30 {
        0.0
    } else {
        x
    }
}

#[derive(Clone, Copy, Debug)]
struct Ramp {
    value: f32,
    target: f32,
    step: f32,
    remaining: u32,
}

impl Ramp {
    fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            step: 0.0,
            remaining: 0,
        }
    }

    fn set(&mut self, target: f32, samples: u32) {
        if target != self.target {
            self.target = target;
            self.remaining = samples;
            self.step = (target - self.value) / samples as f32;
        }
    }

    #[inline]
    fn next(&mut self) -> f32 {
        if self.remaining > 0 {
            self.remaining -= 1;
            // Derive each value from the endpoint, avoiding accumulated f32 drift.
            self.value = self.target - self.step * self.remaining as f32;
        }
        self.value
    }

    fn finish(&mut self) {
        self.value = self.target;
        self.remaining = 0;
        self.step = 0.0;
    }
}

#[derive(Clone, Debug)]
struct ChannelState {
    bass_split: Biquad,
    bass_post: Biquad,
    treble_split: Biquad,
    base: f32, // Per-channel scratch; no temporary Vec in the callback.
    high: f32,
}

impl ChannelState {
    fn new(low: Coefficients, post: Coefficients, high: Coefficients) -> Self {
        Self {
            bass_split: Biquad::new(low),
            bass_post: Biquad::new(post),
            treble_split: Biquad::new(high),
            base: 0.0,
            high: 0.0,
        }
    }
}

#[derive(Debug)]
struct ZrSurround {
    mix: Ramp,
    delay: Vec<[f32; 2]>,
    position: usize,
    shelf_low: [f32; 2],
    shelf_alpha: f32,
}

impl ZrSurround {
    fn new(sample_rate: f32) -> Self {
        // A subtle 0.65 ms interaural delay, allocated only at construction.
        let delay_frames = (sample_rate * 0.00065).round().max(1.0) as usize;
        Self {
            mix: Ramp::new(0.0),
            delay: vec![[0.0; 2]; delay_frames],
            position: 0,
            shelf_low: [0.0; 2],
            shelf_alpha: 1.0
                - (-2.0 * std::f32::consts::PI * 700.0 / sample_rate).exp(),
        }
    }

    fn reset(&mut self) {
        self.delay.fill([0.0; 2]);
        self.position = 0;
        self.shelf_low = [0.0; 2];
        self.mix.finish();
    }

    #[inline]
    fn process(&mut self, samples: &mut [f32]) {
        let mix = self.mix.next();

        // Explicit stereo only. Mono and multichannel layouts are untouched.
        if samples.len() != 2 {
            return;
        }

        let clean = |x: f32| {
            if x.is_finite() {
                x.clamp(-16.0, 16.0)
            } else {
                0.0
            }
        };
        let input = [clean(samples[0]), clean(samples[1])];
        let delayed = self.delay[self.position];
        self.delay[self.position] = input;
        self.position += 1;
        if self.position == self.delay.len() {
            self.position = 0;
        }

        let mut filtered = [0.0; 2];
        for channel in 0..2 {
            self.shelf_low[channel] = flush_tiny(
                self.shelf_low[channel]
                    + self.shelf_alpha * (delayed[channel] - self.shelf_low[channel]),
            );

            // First-order low shelf: unity at DC, 0.22 at high frequencies.
            // Crossfeed is deliberately darker than the direct sound.
            filtered[channel] =
                0.22 * delayed[channel] + 0.78 * self.shelf_low[channel];
        }

        let mid = 0.5 * (input[0] + input[1]);
        let side = 0.5 * (input[0] - input[1]);

        // Differential crossfeed subtracts its common-mode component.
        // The dry center and pre-limiter mono sum are consequently retained.
        // A delayed opposite-channel contribution supplies the subtle Haas cue.
        let common = 0.5 * (filtered[0] + filtered[1]);
        let wet_left = mid + 1.18 * side + 0.12 * (filtered[1] - common);
        let wet_right = mid - 1.18 * side + 0.12 * (filtered[0] - common);

        // Keep delay/filter history warm while bypassed, without modifying PCM.
        if mix > 0.0 {
            samples[0] = input[0] + mix * (wet_left - input[0]);
            samples[1] = input[1] + mix * (wet_right - input[1]);
        }
    }
}

/// Sample-by-sample enhancer with channel-linked treble/output dynamics.
#[derive(Debug)]
pub struct ZireneEnhancer {
    states: Vec<ChannelState>,
    bass: Ramp,
    treble_max: Ramp,
    width: Ramp,
    surround: ZrSurround,
    booster: Ramp,
    // Separate envelopes stay frame-aligned across the three chunk passes.
    booster_limiter: Ramp,
    booster_output: Ramp,
    output_guard_gain: f32,
    ramp_samples: u32,
    enhancement_gain: f32,
    attack: f32,
    release: f32,
    limiter_gain: f32,
    limiter_release: f32,
}

/// Compatibility name for callers using the earlier module interface.
pub type Crystalizer = ZireneEnhancer;

impl ZireneEnhancer {
    /// Construct for 8..384 kHz PCM and at least one channel.
    /// Panics on invalid configuration; allocation occurs only here.
    pub fn new(sample_rate: f32, channels: usize) -> Self {
        assert!(
            sample_rate.is_finite() && (8_000.0..=384_000.0).contains(&sample_rate),
            "sample_rate must be finite and in 8000..=384000 Hz"
        );
        assert!(channels > 0, "channels must be nonzero");
        let low = Coefficients::shelving_basis(sample_rate, BASS_HZ, false);
        let post = Coefficients::butterworth(sample_rate, BASS_HZ, false);
        let high = Coefficients::shelving_basis(sample_rate, TREBLE_HZ, true);
        let rate = |seconds: f64| (1.0 - (-1.0 / (sample_rate as f64 * seconds)).exp()) as f32;
        Self {
            states: vec![ChannelState::new(low, post, high); channels],
            bass: Ramp::new(0.0),
            treble_max: Ramp::new(1.0),
            width: Ramp::new(1.0),
            surround: ZrSurround::new(sample_rate),
            booster: Ramp::new(1.0),
            booster_limiter: Ramp::new(0.0),
            booster_output: Ramp::new(0.0),
            output_guard_gain: 1.0,
            ramp_samples: (0.020 * sample_rate).round().max(1.0) as u32,
            enhancement_gain: 1.0,
            attack: rate(0.002),
            release: rate(0.100),
            limiter_gain: 1.0,
            limiter_release: rate(0.060),
        }
    }

    /// Bass amount. Controls both low-band drive and wet contribution.
    /// Finite values are clamped to [0, 1]; nonfinite values are ignored.
    pub fn set_bass_amount(&mut self, amount: f32) {
        if amount.is_finite() {
            self.bass.set(amount.clamp(0.0, 1.0), self.ramp_samples);
        }
    }

    /// Requested treble gain: 20 * amount dB, dynamically reduced for headroom.
    pub fn set_treble_amount(&mut self, amount: f32) {
        if amount.is_finite() {
            let gain = 10.0_f32.powf(amount.clamp(0.0, 1.0));
            self.treble_max.set(gain, self.ramp_samples);
        }
    }

    /// Stereo side gain: 1 + 0.5 * amount. Mono/multichannel width is unchanged.
    pub fn set_width_amount(&mut self, amount: f32) {
        if amount.is_finite() {
            self.width
                .set(1.0 + 0.5 * amount.clamp(0.0, 1.0), self.ramp_samples);
        }
    }

    /// Short-form alias for set_bass_amount.
    pub fn set_bass(&mut self, amount: f32) {
        self.set_bass_amount(amount);
    }

    /// Short-form alias for set_treble_amount.
    pub fn set_treble(&mut self, amount: f32) {
        self.set_treble_amount(amount);
    }

    /// Short-form alias for set_width_amount.
    pub fn set_width(&mut self, amount: f32) {
        self.set_width_amount(amount);
    }

    pub fn set_surround_enabled(&mut self, enabled: bool) {
        self.surround
            .mix
            .set(if enabled { 1.0 } else { 0.0 }, self.ramp_samples);
    }

    pub fn set_booster_enabled(&mut self, enabled: bool) {
        self.booster
            .set(if enabled { BOOSTER_GAIN } else { 1.0 }, self.ramp_samples);
        let mix = if enabled { 1.0 } else { 0.0 };
        self.booster_limiter.set(mix, self.ramp_samples);
        self.booster_output.set(mix, self.ramp_samples);
    }

    /// Apply before Crystalizer and this enhancer, once per PCM chunk.
    /// Gain ramps per sample frame, not per interleaved sample.
    pub fn process_preamp(&mut self, pcm: &mut [f32]) {
        let channels = self.states.len();
        for frame in pcm.chunks_exact_mut(channels) {
            let gain = self.booster.next();
            if gain == 1.0 {
                continue;
            }
            for sample in frame {
                let input = if sample.is_finite() {
                    sample.clamp(-16.0, 16.0)
                } else {
                    0.0
                };
                *sample = input * gain;
            }
        }
    }

    /// Guard downstream virtualizer peaks while Booster is active or releasing.
    /// This is sample-peak protection, not oversampled true-peak limiting.
    pub fn protect_output(&mut self, pcm: &mut [f32]) {
        let channels = self.states.len();
        for frame in pcm.chunks_exact_mut(channels) {
            // Advance the same per-frame envelope as preamp and enhancer passes.
            let mix = self.booster_output.next();
            if mix == 0.0 && self.output_guard_gain == 1.0 {
                continue;
            }

            let mut peak = 0.0_f32;
            for sample in frame.iter_mut() {
                if !sample.is_finite() {
                    *sample = 0.0;
                }
                peak = peak.max(sample.abs());
            }

            let soft = zr_soft_knee_gain(peak);
            let hard = if peak > 1.0 { 1.0 / peak } else { 1.0 };
            // Retain the hard ceiling during release, then fully bypass at unity.
            let required = ((1.0 - mix) + mix * soft).min(hard);
            self.output_guard_gain = if required < self.output_guard_gain {
                required
            } else {
                approach(self.output_guard_gain, required, self.limiter_release)
            };

            for sample in frame {
                *sample = (*sample * self.output_guard_gain).clamp(-1.0, 1.0);
            }
        }
    }

    /// Clear tails/envelopes and immediately adopt the current parameter targets.
    /// Call at a stream discontinuity, not to switch effects during playback.
    pub fn reset(&mut self) {
        for s in &mut self.states {
            s.bass_split.reset();
            s.bass_post.reset();
            s.treble_split.reset();
            s.base = 0.0;
            s.high = 0.0;
        }
        self.bass.finish();
        self.treble_max.finish();
        self.width.finish();
        self.surround.reset();
        self.booster.finish();
        self.booster_limiter.finish();
        self.booster_output.finish();
        self.output_guard_gain = 1.0;
        self.enhancement_gain = 1.0;
        self.limiter_gain = 1.0;
    }

    /// Process one or more complete interleaved sample frames in place.
    /// An incomplete trailing frame is left untouched. No allocations, locks,
    /// I/O, coefficient redesign, or transcendental functions occur here.
    /// Nonfinite input is treated as zero; extreme finite input is bounded to +/-16.
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        let channels = self.states.len();
        for samples in frame.chunks_exact_mut(channels) {
            self.process_one(samples);
        }
    }

    fn process_one(&mut self, samples: &mut [f32]) {
        self.surround.process(samples);
        let bass = self.bass.next();
        let drive = 1.0 + bass * (BASS_MAX_GAIN - 1.0);
        let max_gain = self.treble_max.next();
        let width = self.width.next();

        for (s, &input) in self.states.iter_mut().zip(samples.iter()) {
            let x = if input.is_finite() {
                input.clamp(-16.0, 16.0)
            } else {
                0.0
            };
            let low = s.bass_split.process(x);
            let amplified = drive * low;
            let clipped = amplified.clamp(-BASS_CLIP, BASS_CLIP);

            // US8582784B2 informs the gain -> clipping -> low-pass branch.
            // Filter only the nonlinear residual, then add the linear shelf
            // separately. This engineering choice avoids putting two cascaded
            // LP phase shifts in the linear boost path (a crossover notch).
            // In steady DC it has the same clipped low-band replacement;
            // bass == 0 is dry even while the filters stay warm.
            // LPF attenuates clipping harmonics; it is NOT alias-free oversampling.
            let distortion = s.bass_post.process(clipped - amplified);
            s.base = x + bass * ((drive - 1.0) * low + distortion);
            s.high = s.treble_split.process(s.base);
        }

        // M/S introduces no delays or phase rotation. Applying it to both base
        // and HF commutes with their linear, linked-gain recombination;
        // doing it here lets the headroom detector include the widened peaks.
        // Only an explicitly stereo layout is widened (not arbitrary channel pairs).
        if self.states.len() == 2 && width != 1.0 {
            let (left, right) = self.states.split_at_mut(1);
            widen_pair(&mut left[0].base, &mut right[0].base, width);
            widen_pair(&mut left[0].high, &mut right[0].high, width);
        }

        // US20110002467A1: estimate the pseudo-output with the PREVIOUS gain,
        // measure headroom, then update a shared gain before recombination.
        // base + (G-1)*high is exactly (base-high) + G*high.
        let previous_extra = self.enhancement_gain - 1.0;
        let mut pseudo_peak = 0.0_f32;
        let mut safe_extra = max_gain - 1.0;
        for s in &self.states {
            pseudo_peak = pseudo_peak.max((s.base + previous_extra * s.high).abs());
            // Solve |base + extra*high| <= GC_CEILING for nonnegative extra.
            // This signed calculation handles cancellation better than |HF| alone.
            if s.base.abs() > GC_CEILING {
                safe_extra = 0.0;
            } else if s.high.abs() > 1.0e-12 {
                let bound = if s.high > 0.0 {
                    (GC_CEILING - s.base) / s.high
                } else {
                    (-GC_CEILING - s.base) / s.high
                };
                safe_extra = safe_extra.min(bound.max(0.0));
            }
        }
        let headroom = 1.0 - pseudo_peak;
        // Hysteresis avoids chasing a threshold; 2 ms reduction, 100 ms recovery.
        let target = if headroom < 0.05 {
            1.0 + safe_extra
        } else if headroom > 0.15 {
            max_gain
        } else {
            self.enhancement_gain.min(max_gain)
        };
        let speed = if target < self.enhancement_gain {
            self.attack
        } else {
            self.release
        };
        self.enhancement_gain = approach(self.enhancement_gain, target, speed).clamp(1.0, max_gain);

        let extra = self.enhancement_gain - 1.0;
        let mut peak = 0.0_f32;
        for (s, output) in self.states.iter().zip(samples.iter_mut()) {
            // Saturator in the enhancement path; normally inactive. The additive
            // form preserves unity when enhancement is zero, even on transients.
            let added_high = (extra * s.high).clamp(-INTERNAL_LIMIT, INTERNAL_LIMIT);
            *output = s.base + added_high;
            peak = peak.max(output.abs());
        }

        // Shared, zero-lookahead SAMPLE-peak limiter after bass/treble/width.
        // Instant reduction catches peaks that outrun the smoothed GC. Smooth
        // recovery avoids an independent limiter moving the stereo image.
        // This is not a true-peak limiter or loudspeaker excursion protection.
        let hard_required = if peak > 1.0 { 1.0 / peak } else { 1.0 };
        let booster_mix = self.booster_limiter.next();
        let soft_required = zr_soft_knee_gain(peak);
        // Preserve the original limiter with Booster off. Independent per-frame
        // envelopes avoid block-sized knee jumps when a chunk crosses a ramp end.
        let required =
            (1.0 - booster_mix) * hard_required + booster_mix * soft_required;
        self.limiter_gain = if required < self.limiter_gain {
            required
        } else {
            approach(self.limiter_gain, required, self.limiter_release)
        };
        for output in samples {
            *output = (*output * self.limiter_gain).clamp(-1.0, 1.0);
        }
    }
}

#[inline]
fn widen_pair(left: &mut f32, right: &mut f32, width: f32) {
    let mid = 0.5 * (*left + *right);
    let side = 0.5 * (*left - *right) * width;
    *left = mid + side;
    *right = mid - side;
}

#[inline]
fn approach(current: f32, target: f32, speed: f32) -> f32 {
    let next = current + speed * (target - current);
    // Avoid a permanent near-target offset from f32 rounding at high rates.
    if next == current {
        target
    } else {
        next
    }
}
