//! 10-band graphic equalizer: RBJ peaking biquads (transposed direct form II) per channel.
//! A preamp automatically cancels the largest boost so the EQ cannot clip on its own.

use std::f32::consts::PI;

pub const BANDS: usize = 10;
pub const FREQS: [f32; BANDS] = [31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
const Q: f32 = 1.2;

/// [b0, b1, b2, a1, a2], already divided by a0.
fn peaking(freq: f32, rate: f32, gain_db: f32) -> [f32; 5] {
    let a = 10f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq / rate;
    let alpha = w0.sin() / (2.0 * Q);
    let cosw = w0.cos();
    let a0 = 1.0 + alpha / a;
    [(1.0 + alpha * a) / a0, (-2.0 * cosw) / a0, (1.0 - alpha * a) / a0, (-2.0 * cosw) / a0, (1.0 - alpha / a) / a0]
}

pub struct Equalizer {
    channels: usize,
    enabled: bool,
    preamp: f32,
    coefs: [[f32; 5]; BANDS],
    active: [bool; BANDS],
    z: Vec<[[f32; 2]; BANDS]>,
}

impl Equalizer {
    pub fn new(channels: usize) -> Self {
        Equalizer {
            channels,
            enabled: false,
            preamp: 1.0,
            coefs: [[0.0; 5]; BANDS],
            active: [false; BANDS],
            z: vec![[[0.0; 2]; BANDS]; channels],
        }
    }

    pub fn configure(&mut self, rate: f32, enabled: bool, gains: &[f32; BANDS]) {
        let max_gain = gains.iter().cloned().fold(0.0f32, f32::max).min(12.0);
        self.preamp = 10f32.powf(-max_gain / 20.0);
        let mut any = false;
        for i in 0..BANDS {
            let g = gains[i].clamp(-12.0, 12.0);
            if g.abs() < 0.05 || FREQS[i] >= rate * 0.45 {
                self.active[i] = false;
                continue;
            }
            self.active[i] = true;
            any = true;
            self.coefs[i] = peaking(FREQS[i], rate, g);
        }
        self.enabled = enabled && any;
    }

    pub fn reset(&mut self) {
        for z in self.z.iter_mut() {
            *z = [[0.0; 2]; BANDS];
        }
    }

    pub fn process_frame(&mut self, frame: &mut [f32]) {
        if !self.enabled {
            return;
        }
        for (c, s) in frame.iter_mut().enumerate() {
            if c >= self.channels {
                break;
            }
            let mut x = *s * self.preamp;
            let z = &mut self.z[c];
            for b in 0..BANDS {
                if !self.active[b] {
                    continue;
                }
                let k = &self.coefs[b];
                let y = k[0] * x + z[b][0];
                z[b][0] = k[1] * x - k[3] * y + z[b][1];
                z[b][1] = k[2] * x - k[4] * y;
                x = y;
            }
            *s = x;
        }
    }
}
