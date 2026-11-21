//! Static, allocation-free ReplayGain for interleaved f32 PCM.

#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct ReplayGainProcessor {
    linear_gain: f32,
    track_peak: Option<f32>,
    safe_gain: f32,
    enabled: bool,
}

impl ReplayGainProcessor {
    pub const fn new() -> Self {
        Self {
            linear_gain: 1.0,
            track_peak: None,
            safe_gain: 1.0,
            enabled: true,
        }
    }

    /// Non-finite metadata falls back to unity gain.
    pub fn set_track_gain(&mut self, gain_db: f32) {
        self.linear_gain = if gain_db.is_finite() {
            10.0_f32.powf(gain_db / 20.0).min(f32::MAX)
        } else {
            1.0
        };
        self.update_gain();
    }

    /// Peak is a linear amplitude, not dB. Invalid peaks clear peak limiting.
    pub fn set_track_peak(&mut self, peak: f32) {
        self.track_peak = if peak.is_finite() && peak >= 0.0 {
            Some(peak)
        } else {
            None
        };
        self.update_gain();
    }

    /// Applies identical gain to every channel, regardless of channel count.
    /// With peak metadata, a final clamp guards against rounding or bad metadata.
    /// Without peak metadata, boosted samples may exceed [-1.0, 1.0].
    pub fn apply(&self, frame: &mut [f32]) {
        if !self.enabled {
            return;
        }

        let gain = self.safe_gain;
        let limit_peak = self.track_peak.is_some();
        let scale = |sample: &mut f32| {
            let output = *sample * gain;
            *sample = if limit_peak {
                output.clamp(-1.0, 1.0)
            } else {
                output
            };
        };

        // Pairs are just processing batches, not an assumption of stereo.
        let paired_len = frame.len() - frame.len() % 2;
        let (pairs, tail) = frame.split_at_mut(paired_len);
        for pair in pairs.chunks_exact_mut(2) {
            scale(&mut pair[0]);
            scale(&mut pair[1]);
        }
        for sample in tail {
            scale(sample);
        }
    }

    /// Restores unity gain, absent peak metadata, and enabled processing.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Bypasses processing without changing gain or peak metadata.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn update_gain(&mut self) {
        self.safe_gain = match self.track_peak {
            Some(peak) if peak > 0.0 => self.linear_gain.min(1.0 / peak),
            // A zero peak represents silence; an absent peak does not limit gain.
            _ => self.linear_gain,
        };
    }
}

impl Default for ReplayGainProcessor {
    fn default() -> Self {
        Self::new()
    }
}
