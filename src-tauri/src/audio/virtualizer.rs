//! Headphone virtualization: applies a per-device parametric EQ profile (AutoEQ format)
//! as a chain of RBJ biquads (transposed direct form II) per channel.
//! Same conventions as `eq.rs`: coefficients are [b0, b1, b2, a1, a2] divided by a0,
//! state is two delay registers per band per channel, and the preamp is a linear gain.
#![allow(dead_code)]
use std::f64::consts::PI;

/// Upper bound on bands per profile (AutoEQ presets use ~10). Extra bands are ignored.
pub const MAX_BANDS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    Peaking,
    LowShelf,
    HighShelf,
}

#[derive(Clone, Copy, Debug)]
pub struct EqBand {
    pub filter_type: FilterType,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Headphone,
    Earbud,
    Speaker,
}

#[derive(Debug)]
pub struct HeadphoneProfile {
    pub name: &'static str,
    pub category: Category,
    pub preamp_db: f32,
    pub bands: &'static [EqBand],
}

/// [b0, b1, b2, a1, a2], already divided by a0. Computed in f64, stored as f32.
fn design(band: &EqBand, rate: f32) -> [f32; 5] {
    let a = 10f64.powf(band.gain_db as f64 / 40.0);
    let w0 = 2.0 * PI * band.freq as f64 / rate as f64;
    let (sin, cos) = w0.sin_cos();
    let alpha = sin / (2.0 * band.q.max(0.05) as f64);
    let (b0, b1, b2, a0, a1, a2) = match band.filter_type {
        FilterType::Peaking => (
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        ),
        FilterType::LowShelf => {
            let t = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) - (a - 1.0) * cos + t),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - t),
                (a + 1.0) + (a - 1.0) * cos + t,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - t,
            )
        }
        FilterType::HighShelf => {
            let t = 2.0 * a.sqrt() * alpha;
            (
                a * ((a + 1.0) + (a - 1.0) * cos + t),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - t),
                (a + 1.0) - (a - 1.0) * cos + t,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - t,
            )
        }
    };
    [
        (b0 / a0) as f32,
        (b1 / a0) as f32,
        (b2 / a0) as f32,
        (a1 / a0) as f32,
        (a2 / a0) as f32,
    ]
}

#[inline]
fn tick(coefs: &[[f32; 5]], preamp: f32, z: &mut [[f32; 2]; MAX_BANDS], sample: f32) -> f32 {
    let mut x = sample * preamp;
    for (k, z) in coefs.iter().zip(z.iter_mut()) {
        let y = k[0] * x + z[0];
        z[0] = k[1] * x - k[3] * y + z[1];
        z[1] = k[2] * x - k[4] * y;
        x = y;
    }
    x
}

pub struct Virtualizer {
    channels: usize,
    enabled: bool,
    preamp: f32,
    count: usize,
    coefs: [[f32; 5]; MAX_BANDS],
    z: Vec<[[f32; 2]; MAX_BANDS]>,
}

impl Virtualizer {
    pub fn new(channels: usize) -> Self {
        Virtualizer {
            channels,
            enabled: false,
            preamp: 1.0,
            count: 0,
            coefs: [[0.0; 5]; MAX_BANDS],
            z: vec![[[0.0; 2]; MAX_BANDS]; channels],
        }
    }

    /// Select a profile (or `None` to bypass). Call again whenever the sample rate changes.
    pub fn set_profile(&mut self, rate: f32, profile: Option<&HeadphoneProfile>) {
        self.count = 0;
        let Some(p) = profile else {
            self.enabled = false;
            self.preamp = 1.0;
            return;
        };
        // AutoEQ preamps are <= 0 dB; never let a profile add gain on its own.
        self.preamp = 10f32.powf(p.preamp_db.min(0.0) / 20.0);
        for band in p.bands.iter().take(MAX_BANDS) {
            if band.freq <= 0.0 || band.freq >= rate * 0.45 {
                continue;
            }
            self.coefs[self.count] = design(band, rate);
            self.count += 1;
        }
        self.enabled = self.count > 0;
        self.reset();
    }

    pub fn reset(&mut self) {
        for z in self.z.iter_mut() {
            *z = [[0.0; 2]; MAX_BANDS];
        }
    }

    /// Interleaved path, same shape as `Equalizer::process_frame`.
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        if !self.enabled {
            return;
        }
        let coefs = &self.coefs[..self.count];
        for (i, s) in frame.iter_mut().enumerate() {
            // Use the remainder instead of breaking here to handle the remaining samples.
            let c = i % self.channels; 
            *s = tick(coefs, self.preamp, &mut self.z[c], *s);
        }
    }

    /// Planar path: processes left and right in place (channels 0 and 1).
    pub fn apply(&mut self, left: &mut [f32], right: &mut [f32]) {
        if !self.enabled || self.channels < 2 {
            return;
        }
        let coefs = &self.coefs[..self.count];
        let (zl, zr) = self.z.split_at_mut(1);
        for s in left.iter_mut() {
            *s = tick(coefs, self.preamp, &mut zl[0], *s);
        }
        for s in right.iter_mut() {
            *s = tick(coefs, self.preamp, &mut zr[0], *s);
        }
    }
}

pub fn find_profile(name: &str) -> Option<&'static HeadphoneProfile> {
    PROFILES.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

// ---- BEGIN GENERATED PROFILES ----
pub static PROFILES: &[HeadphoneProfile] = &[
    HeadphoneProfile {
        name: "Sony WH-1000XM5", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.2,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -3.2, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 2448.0, gain_db: 6.9, q: 2.46 },
            EqBand { filter_type: FilterType::Peaking, freq: 173.0, gain_db: -5.6, q: 0.96 },
            EqBand { filter_type: FilterType::Peaking, freq: 3028.0, gain_db: -5.4, q: 2.03 },
            EqBand { filter_type: FilterType::Peaking, freq: 1327.0, gain_db: 3.3, q: 0.58 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: 4.9, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 6110.0, gain_db: -2.3, q: 5.81 },
            EqBand { filter_type: FilterType::Peaking, freq: 875.0, gain_db: -1.2, q: 4.07 },
            EqBand { filter_type: FilterType::Peaking, freq: 1197.0, gain_db: 1.0, q: 3.28 },
            EqBand { filter_type: FilterType::Peaking, freq: 63.0, gain_db: 0.4, q: 2.13 },
        ],
    },
    HeadphoneProfile {
        name: "Sony WH-1000XM4", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.1,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -4.2, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 143.0, gain_db: -5.2, q: 1.10 },
            EqBand { filter_type: FilterType::Peaking, freq: 2289.0, gain_db: 6.1, q: 1.57 },
            EqBand { filter_type: FilterType::Peaking, freq: 56.0, gain_db: 1.2, q: 1.19 },
            EqBand { filter_type: FilterType::Peaking, freq: 5144.0, gain_db: -3.2, q: 6.00 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -1.0, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 407.0, gain_db: 1.7, q: 3.14 },
            EqBand { filter_type: FilterType::Peaking, freq: 6715.0, gain_db: 3.0, q: 5.99 },
            EqBand { filter_type: FilterType::Peaking, freq: 1007.0, gain_db: 1.0, q: 3.41 },
            EqBand { filter_type: FilterType::Peaking, freq: 576.0, gain_db: -1.2, q: 3.55 },
        ],
    },
    HeadphoneProfile {
        name: "Apple AirPods Max", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -4.7,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -3.0, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 7273.0, gain_db: 3.6, q: 2.41 },
            EqBand { filter_type: FilterType::Peaking, freq: 218.0, gain_db: -2.9, q: 1.41 },
            EqBand { filter_type: FilterType::Peaking, freq: 1031.0, gain_db: -3.2, q: 0.99 },
            EqBand { filter_type: FilterType::Peaking, freq: 3185.0, gain_db: 3.1, q: 0.56 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -5.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 9508.0, gain_db: 2.7, q: 2.20 },
            EqBand { filter_type: FilterType::Peaking, freq: 66.0, gain_db: 0.6, q: 1.65 },
            EqBand { filter_type: FilterType::Peaking, freq: 4045.0, gain_db: 2.1, q: 5.82 },
            EqBand { filter_type: FilterType::Peaking, freq: 4834.0, gain_db: -1.8, q: 6.00 },
        ],
    },
    HeadphoneProfile {
        name: "Sennheiser HD 600", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.3,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 6.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 125.0, gain_db: -2.7, q: 0.55 },
            EqBand { filter_type: FilterType::Peaking, freq: 8445.0, gain_db: 3.3, q: 1.61 },
            EqBand { filter_type: FilterType::Peaking, freq: 522.0, gain_db: 0.7, q: 1.02 },
            EqBand { filter_type: FilterType::Peaking, freq: 1298.0, gain_db: -1.2, q: 2.14 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -3.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 3158.0, gain_db: -1.8, q: 3.67 },
            EqBand { filter_type: FilterType::Peaking, freq: 2166.0, gain_db: 0.9, q: 3.32 },
            EqBand { filter_type: FilterType::Peaking, freq: 6639.0, gain_db: 2.2, q: 5.82 },
            EqBand { filter_type: FilterType::Peaking, freq: 5433.0, gain_db: -1.2, q: 5.70 },
        ],
    },
    HeadphoneProfile {
        name: "Sennheiser HD 650", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.1,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 6.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 8800.0, gain_db: 5.1, q: 1.42 },
            EqBand { filter_type: FilterType::Peaking, freq: 118.0, gain_db: -3.1, q: 0.50 },
            EqBand { filter_type: FilterType::Peaking, freq: 37.0, gain_db: 0.7, q: 3.96 },
            EqBand { filter_type: FilterType::Peaking, freq: 3169.0, gain_db: -1.7, q: 3.89 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -2.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 1227.0, gain_db: -1.2, q: 2.53 },
            EqBand { filter_type: FilterType::Peaking, freq: 2055.0, gain_db: 1.2, q: 3.23 },
            EqBand { filter_type: FilterType::Peaking, freq: 587.0, gain_db: 0.4, q: 1.19 },
            EqBand { filter_type: FilterType::Peaking, freq: 5332.0, gain_db: -1.1, q: 5.75 },
        ],
    },
    HeadphoneProfile {
        name: "Sennheiser HD 800 S", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.2,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 6.6, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 142.0, gain_db: -2.3, q: 0.30 },
            EqBand { filter_type: FilterType::Peaking, freq: 1959.0, gain_db: 2.5, q: 0.88 },
            EqBand { filter_type: FilterType::Peaking, freq: 5679.0, gain_db: -4.4, q: 4.36 },
            EqBand { filter_type: FilterType::Peaking, freq: 212.0, gain_db: -0.5, q: 2.36 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -4.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 1043.0, gain_db: -1.0, q: 2.89 },
            EqBand { filter_type: FilterType::Peaking, freq: 1445.0, gain_db: 1.0, q: 3.43 },
            EqBand { filter_type: FilterType::Peaking, freq: 2640.0, gain_db: -0.9, q: 4.01 },
            EqBand { filter_type: FilterType::Peaking, freq: 3421.0, gain_db: 1.3, q: 5.47 },
        ],
    },
    HeadphoneProfile {
        name: "Beyerdynamic DT 770 PRO (250 Ohm)", // AutoEQ: crinacle/GRAS 43AG-7 over-ear
        category: Category::Headphone,
        preamp_db: -3.7,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -3.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 221.0, gain_db: 4.0, q: 2.08 },
            EqBand { filter_type: FilterType::Peaking, freq: 142.0, gain_db: -4.2, q: 3.61 },
            EqBand { filter_type: FilterType::Peaking, freq: 6113.0, gain_db: -8.3, q: 1.22 },
            EqBand { filter_type: FilterType::Peaking, freq: 3884.0, gain_db: 6.9, q: 1.43 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -5.0, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 87.0, gain_db: 4.1, q: 3.59 },
            EqBand { filter_type: FilterType::Peaking, freq: 43.0, gain_db: -1.4, q: 1.19 },
            EqBand { filter_type: FilterType::Peaking, freq: 115.0, gain_db: -2.1, q: 4.78 },
            EqBand { filter_type: FilterType::Peaking, freq: 72.0, gain_db: 1.2, q: 2.89 },
        ],
    },
    HeadphoneProfile {
        name: "Beyerdynamic DT 990 PRO", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.7,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 10.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 541.0, gain_db: 3.1, q: 0.52 },
            EqBand { filter_type: FilterType::Peaking, freq: 64.0, gain_db: -7.4, q: 0.38 },
            EqBand { filter_type: FilterType::Peaking, freq: 7731.0, gain_db: -4.6, q: 1.06 },
            EqBand { filter_type: FilterType::Peaking, freq: 640.0, gain_db: 0.6, q: 2.47 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -9.8, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 7136.0, gain_db: 3.1, q: 2.08 },
            EqBand { filter_type: FilterType::Peaking, freq: 5897.0, gain_db: -4.8, q: 5.30 },
            EqBand { filter_type: FilterType::Peaking, freq: 4193.0, gain_db: 1.1, q: 1.51 },
            EqBand { filter_type: FilterType::Peaking, freq: 9436.0, gain_db: 2.7, q: 3.53 },
        ],
    },
    HeadphoneProfile {
        name: "Audio-Technica ATH-M50x", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -3.1,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 0.6, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 156.0, gain_db: -5.2, q: 0.73 },
            EqBand { filter_type: FilterType::Peaking, freq: 326.0, gain_db: 5.3, q: 1.59 },
            EqBand { filter_type: FilterType::Peaking, freq: 7077.0, gain_db: 2.8, q: 2.22 },
            EqBand { filter_type: FilterType::Peaking, freq: 3483.0, gain_db: 2.1, q: 5.82 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -4.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 45.0, gain_db: -1.1, q: 1.90 },
            EqBand { filter_type: FilterType::Peaking, freq: 66.0, gain_db: 1.4, q: 3.59 },
            EqBand { filter_type: FilterType::Peaking, freq: 787.0, gain_db: -0.5, q: 1.79 },
            EqBand { filter_type: FilterType::Peaking, freq: 1640.0, gain_db: 0.9, q: 3.41 },
        ],
    },
    HeadphoneProfile {
        name: "HiFiMan Sundara", // AutoEQ: crinacle/GRAS 43AG-7 over-ear
        category: Category::Headphone,
        preamp_db: -6.1,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 6.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 127.0, gain_db: -2.0, q: 0.29 },
            EqBand { filter_type: FilterType::Peaking, freq: 2009.0, gain_db: 6.5, q: 2.96 },
            EqBand { filter_type: FilterType::Peaking, freq: 4085.0, gain_db: -4.4, q: 1.19 },
            EqBand { filter_type: FilterType::Peaking, freq: 7580.0, gain_db: 2.3, q: 0.66 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -1.6, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 63.0, gain_db: -1.2, q: 2.40 },
            EqBand { filter_type: FilterType::Peaking, freq: 38.0, gain_db: 0.9, q: 2.27 },
            EqBand { filter_type: FilterType::Peaking, freq: 789.0, gain_db: -1.6, q: 4.10 },
            EqBand { filter_type: FilterType::Peaking, freq: 5821.0, gain_db: 1.9, q: 6.00 },
        ],
    },
    HeadphoneProfile {
        name: "AKG K371", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -5.6,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -2.7, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 182.0, gain_db: -2.3, q: 1.23 },
            EqBand { filter_type: FilterType::Peaking, freq: 4038.0, gain_db: 5.0, q: 3.60 },
            EqBand { filter_type: FilterType::Peaking, freq: 67.0, gain_db: 3.1, q: 1.41 },
            EqBand { filter_type: FilterType::Peaking, freq: 1066.0, gain_db: -0.8, q: 2.57 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: 2.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 5564.0, gain_db: -1.6, q: 3.89 },
            EqBand { filter_type: FilterType::Peaking, freq: 4232.0, gain_db: 0.9, q: 5.03 },
            EqBand { filter_type: FilterType::Peaking, freq: 524.0, gain_db: 0.3, q: 1.66 },
            EqBand { filter_type: FilterType::Peaking, freq: 2048.0, gain_db: 0.5, q: 4.67 },
        ],
    },
    HeadphoneProfile {
        name: "AKG K702", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.1,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 7.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 119.0, gain_db: -2.7, q: 0.23 },
            EqBand { filter_type: FilterType::Peaking, freq: 9459.0, gain_db: 3.2, q: 3.19 },
            EqBand { filter_type: FilterType::Peaking, freq: 730.0, gain_db: 3.2, q: 1.22 },
            EqBand { filter_type: FilterType::Peaking, freq: 2241.0, gain_db: -3.5, q: 3.80 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -1.7, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 3721.0, gain_db: 3.4, q: 2.02 },
            EqBand { filter_type: FilterType::Peaking, freq: 5483.0, gain_db: -4.7, q: 4.79 },
            EqBand { filter_type: FilterType::Peaking, freq: 2633.0, gain_db: -2.0, q: 5.29 },
            EqBand { filter_type: FilterType::Peaking, freq: 56.0, gain_db: -0.6, q: 2.62 },
        ],
    },
    HeadphoneProfile {
        name: "Focal Clear Mg", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.8,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 5.9, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 8641.0, gain_db: 6.3, q: 1.17 },
            EqBand { filter_type: FilterType::Peaking, freq: 1198.0, gain_db: -4.0, q: 1.73 },
            EqBand { filter_type: FilterType::Peaking, freq: 94.0, gain_db: -4.2, q: 0.30 },
            EqBand { filter_type: FilterType::Peaking, freq: 4530.0, gain_db: 5.5, q: 3.15 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -3.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 2223.0, gain_db: 1.7, q: 4.29 },
            EqBand { filter_type: FilterType::Peaking, freq: 3196.0, gain_db: -1.7, q: 3.02 },
            EqBand { filter_type: FilterType::Peaking, freq: 7343.0, gain_db: 1.5, q: 5.95 },
            EqBand { filter_type: FilterType::Peaking, freq: 3986.0, gain_db: 1.4, q: 6.00 },
        ],
    },
    HeadphoneProfile {
        name: "Meze 109 Pro", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -5.3,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 7.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 72.0, gain_db: -4.2, q: 0.25 },
            EqBand { filter_type: FilterType::Peaking, freq: 2028.0, gain_db: 5.6, q: 1.63 },
            EqBand { filter_type: FilterType::Peaking, freq: 2856.0, gain_db: -1.9, q: 4.04 },
            EqBand { filter_type: FilterType::Peaking, freq: 83.0, gain_db: -1.2, q: 1.11 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -4.7, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 8692.0, gain_db: 1.6, q: 2.63 },
            EqBand { filter_type: FilterType::Peaking, freq: 825.0, gain_db: -0.9, q: 2.44 },
            EqBand { filter_type: FilterType::Peaking, freq: 475.0, gain_db: 0.4, q: 1.32 },
            EqBand { filter_type: FilterType::Peaking, freq: 4705.0, gain_db: 1.3, q: 6.00 },
        ],
    },
    HeadphoneProfile {
        name: "Philips Fidelio X2HR", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.4,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 7.8, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 64.0, gain_db: -7.1, q: 0.68 },
            EqBand { filter_type: FilterType::Peaking, freq: 4982.0, gain_db: 4.2, q: 0.79 },
            EqBand { filter_type: FilterType::Peaking, freq: 5096.0, gain_db: -8.2, q: 2.97 },
            EqBand { filter_type: FilterType::Peaking, freq: 29.0, gain_db: 1.2, q: 4.24 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -1.2, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 1660.0, gain_db: -2.7, q: 4.61 },
            EqBand { filter_type: FilterType::Peaking, freq: 261.0, gain_db: -1.0, q: 5.02 },
            EqBand { filter_type: FilterType::Peaking, freq: 2026.0, gain_db: 0.9, q: 4.49 },
            EqBand { filter_type: FilterType::Peaking, freq: 2996.0, gain_db: 1.2, q: 6.00 },
        ],
    },
    HeadphoneProfile {
        name: "Razer BlackShark V2 Pro", // AutoEQ: crinacle/GRAS 43AG-7 over-ear
        category: Category::Headphone,
        preamp_db: -5.4,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 1.9, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 153.0, gain_db: -7.6, q: 0.72 },
            EqBand { filter_type: FilterType::Peaking, freq: 625.0, gain_db: 5.9, q: 0.85 },
            EqBand { filter_type: FilterType::Peaking, freq: 3612.0, gain_db: -4.0, q: 4.82 },
            EqBand { filter_type: FilterType::Peaking, freq: 398.0, gain_db: 2.5, q: 2.18 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -1.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 2525.0, gain_db: 2.7, q: 3.01 },
            EqBand { filter_type: FilterType::Peaking, freq: 1373.0, gain_db: -1.5, q: 2.52 },
            EqBand { filter_type: FilterType::Peaking, freq: 1953.0, gain_db: 1.0, q: 3.23 },
            EqBand { filter_type: FilterType::Peaking, freq: 3166.0, gain_db: -1.8, q: 5.42 },
        ],
    },
    HeadphoneProfile {
        name: "Audeze LCD-5", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -6.0,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 6.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 1241.0, gain_db: -5.3, q: 1.03 },
            EqBand { filter_type: FilterType::Peaking, freq: 6732.0, gain_db: 7.7, q: 0.18 },
            EqBand { filter_type: FilterType::Peaking, freq: 121.0, gain_db: -2.1, q: 0.31 },
            EqBand { filter_type: FilterType::Peaking, freq: 3769.0, gain_db: -8.3, q: 1.07 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -6.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 2190.0, gain_db: 1.1, q: 3.60 },
            EqBand { filter_type: FilterType::Peaking, freq: 2968.0, gain_db: -0.9, q: 3.35 },
            EqBand { filter_type: FilterType::Peaking, freq: 4074.0, gain_db: 1.3, q: 4.83 },
            EqBand { filter_type: FilterType::Peaking, freq: 4981.0, gain_db: -1.1, q: 6.00 },
        ],
    },
    HeadphoneProfile {
        name: "Dan Clark Audio Stealth", // AutoEQ: oratory1990/over-ear
        category: Category::Headphone,
        preamp_db: -0.9,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -2.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 966.0, gain_db: -1.1, q: 1.90 },
            EqBand { filter_type: FilterType::Peaking, freq: 115.0, gain_db: -0.9, q: 2.71 },
            EqBand { filter_type: FilterType::Peaking, freq: 4046.0, gain_db: 1.0, q: 3.60 },
            EqBand { filter_type: FilterType::Peaking, freq: 58.0, gain_db: 1.4, q: 1.69 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: 0.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 5296.0, gain_db: -1.5, q: 6.00 },
            EqBand { filter_type: FilterType::Peaking, freq: 179.0, gain_db: 0.8, q: 3.99 },
            EqBand { filter_type: FilterType::Peaking, freq: 2131.0, gain_db: 0.4, q: 3.51 },
            EqBand { filter_type: FilterType::Peaking, freq: 261.0, gain_db: -0.4, q: 2.87 },
        ],
    },
    HeadphoneProfile {
        name: "Apple AirPods Pro (1st Gen)", // AutoEQ: crinacle/711 in-ear
        category: Category::Earbud,
        preamp_db: -6.0,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 2.6, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 514.0, gain_db: -4.4, q: 0.68 },
            EqBand { filter_type: FilterType::Peaking, freq: 8903.0, gain_db: 6.0, q: 1.66 },
            EqBand { filter_type: FilterType::Peaking, freq: 183.0, gain_db: 2.0, q: 0.77 },
            EqBand { filter_type: FilterType::Peaking, freq: 4613.0, gain_db: 3.4, q: 2.46 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -0.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 1517.0, gain_db: -1.0, q: 2.33 },
            EqBand { filter_type: FilterType::Peaking, freq: 929.0, gain_db: 1.2, q: 2.75 },
            EqBand { filter_type: FilterType::Peaking, freq: 44.0, gain_db: -0.5, q: 2.16 },
            EqBand { filter_type: FilterType::Peaking, freq: 618.0, gain_db: -0.5, q: 2.86 },
        ],
    },
    HeadphoneProfile {
        name: "Samsung Galaxy Buds2 Pro", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -2.3,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -2.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 7526.0, gain_db: 2.2, q: 2.78 },
            EqBand { filter_type: FilterType::Peaking, freq: 266.0, gain_db: -0.8, q: 1.35 },
            EqBand { filter_type: FilterType::Peaking, freq: 3426.0, gain_db: -2.3, q: 3.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 64.0, gain_db: 2.2, q: 1.18 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -3.8, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 8817.0, gain_db: 1.6, q: 2.28 },
            EqBand { filter_type: FilterType::Peaking, freq: 2283.0, gain_db: 1.2, q: 3.75 },
            EqBand { filter_type: FilterType::Peaking, freq: 4955.0, gain_db: -1.2, q: 5.11 },
            EqBand { filter_type: FilterType::Peaking, freq: 977.0, gain_db: 1.0, q: 5.18 },
        ],
    },
    HeadphoneProfile {
        name: "Sony WF-1000XM5", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -2.6,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 5.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 68.0, gain_db: -4.8, q: 0.27 },
            EqBand { filter_type: FilterType::Peaking, freq: 1377.0, gain_db: 2.6, q: 0.31 },
            EqBand { filter_type: FilterType::Peaking, freq: 1806.0, gain_db: -3.5, q: 1.16 },
            EqBand { filter_type: FilterType::Peaking, freq: 6875.0, gain_db: 1.8, q: 3.49 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: 2.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 3757.0, gain_db: 0.3, q: 2.56 },
            EqBand { filter_type: FilterType::Peaking, freq: 5057.0, gain_db: -0.7, q: 5.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 449.0, gain_db: 0.3, q: 4.15 },
            EqBand { filter_type: FilterType::Peaking, freq: 677.0, gain_db: -0.2, q: 2.75 },
        ],
    },
    HeadphoneProfile {
        name: "Sony WF-1000XM4", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -7.5,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -0.6, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 176.0, gain_db: -4.2, q: 0.60 },
            EqBand { filter_type: FilterType::Peaking, freq: 9973.0, gain_db: 3.7, q: 1.01 },
            EqBand { filter_type: FilterType::Peaking, freq: 6316.0, gain_db: 4.4, q: 0.91 },
            EqBand { filter_type: FilterType::Peaking, freq: 498.0, gain_db: -1.4, q: 1.46 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: 3.7, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 1732.0, gain_db: 1.2, q: 2.28 },
            EqBand { filter_type: FilterType::Peaking, freq: 3942.0, gain_db: -0.8, q: 3.10 },
            EqBand { filter_type: FilterType::Peaking, freq: 978.0, gain_db: -0.3, q: 1.49 },
            EqBand { filter_type: FilterType::Peaking, freq: 38.0, gain_db: -0.2, q: 2.02 },
        ],
    },
    HeadphoneProfile {
        name: "Sennheiser Momentum True Wireless 4", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -6.5,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -0.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 5605.0, gain_db: 5.8, q: 2.82 },
            EqBand { filter_type: FilterType::Peaking, freq: 123.0, gain_db: -2.8, q: 0.23 },
            EqBand { filter_type: FilterType::Peaking, freq: 3977.0, gain_db: 3.9, q: 2.27 },
            EqBand { filter_type: FilterType::Peaking, freq: 70.0, gain_db: 2.1, q: 0.97 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -5.2, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 1511.0, gain_db: -1.3, q: 1.73 },
            EqBand { filter_type: FilterType::Peaking, freq: 877.0, gain_db: 0.8, q: 2.46 },
            EqBand { filter_type: FilterType::Peaking, freq: 7841.0, gain_db: 2.1, q: 5.59 },
            EqBand { filter_type: FilterType::Peaking, freq: 3002.0, gain_db: 0.7, q: 3.65 },
        ],
    },
    HeadphoneProfile {
        name: "Moondrop Aria", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -3.4,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -0.2, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 164.0, gain_db: -2.6, q: 0.92 },
            EqBand { filter_type: FilterType::Peaking, freq: 6460.0, gain_db: 3.5, q: 2.43 },
            EqBand { filter_type: FilterType::Peaking, freq: 659.0, gain_db: 1.4, q: 1.59 },
            EqBand { filter_type: FilterType::Peaking, freq: 3108.0, gain_db: 0.5, q: 1.51 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -5.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 8665.0, gain_db: 2.1, q: 2.79 },
            EqBand { filter_type: FilterType::Peaking, freq: 1408.0, gain_db: -0.5, q: 2.30 },
            EqBand { filter_type: FilterType::Peaking, freq: 54.0, gain_db: 0.9, q: 5.32 },
            EqBand { filter_type: FilterType::Peaking, freq: 31.0, gain_db: -0.4, q: 2.21 },
        ],
    },
    HeadphoneProfile {
        name: "Moondrop Blessing 3", // AutoEQ: crinacle/711 in-ear
        category: Category::Earbud,
        preamp_db: -2.9,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 2.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 10000.0, gain_db: 2.8, q: 0.97 },
            EqBand { filter_type: FilterType::Peaking, freq: 3076.0, gain_db: -2.1, q: 3.52 },
            EqBand { filter_type: FilterType::Peaking, freq: 237.0, gain_db: -0.5, q: 1.04 },
            EqBand { filter_type: FilterType::Peaking, freq: 1479.0, gain_db: -1.1, q: 3.10 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -8.7, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 9596.0, gain_db: 3.9, q: 2.54 },
            EqBand { filter_type: FilterType::Peaking, freq: 863.0, gain_db: 0.8, q: 2.44 },
            EqBand { filter_type: FilterType::Peaking, freq: 451.0, gain_db: -0.2, q: 1.54 },
            EqBand { filter_type: FilterType::Peaking, freq: 6947.0, gain_db: 1.4, q: 5.99 },
        ],
    },
    HeadphoneProfile {
        name: "7Hz Salnotes Zero", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -2.5,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -0.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 7148.0, gain_db: 2.5, q: 1.39 },
            EqBand { filter_type: FilterType::Peaking, freq: 186.0, gain_db: -1.3, q: 1.35 },
            EqBand { filter_type: FilterType::Peaking, freq: 65.0, gain_db: 1.7, q: 1.11 },
            EqBand { filter_type: FilterType::Peaking, freq: 1677.0, gain_db: -1.1, q: 1.82 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -1.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 857.0, gain_db: 0.9, q: 1.64 },
            EqBand { filter_type: FilterType::Peaking, freq: 9824.0, gain_db: 0.9, q: 1.98 },
            EqBand { filter_type: FilterType::Peaking, freq: 3316.0, gain_db: -0.6, q: 2.50 },
            EqBand { filter_type: FilterType::Peaking, freq: 1261.0, gain_db: -0.7, q: 3.34 },
        ],
    },
    HeadphoneProfile {
        name: "Linsoul 7Hz Timeless", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -3.9,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -0.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 4515.0, gain_db: 4.0, q: 1.92 },
            EqBand { filter_type: FilterType::Peaking, freq: 164.0, gain_db: -2.2, q: 0.87 },
            EqBand { filter_type: FilterType::Peaking, freq: 670.0, gain_db: 1.8, q: 1.01 },
            EqBand { filter_type: FilterType::Peaking, freq: 1958.0, gain_db: -1.9, q: 1.89 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -7.3, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 6184.0, gain_db: 2.3, q: 4.39 },
            EqBand { filter_type: FilterType::Peaking, freq: 34.0, gain_db: -0.2, q: 2.05 },
            EqBand { filter_type: FilterType::Peaking, freq: 63.0, gain_db: 0.2, q: 2.01 },
            EqBand { filter_type: FilterType::Peaking, freq: 3040.0, gain_db: 0.5, q: 5.97 },
        ],
    },
    HeadphoneProfile {
        name: "Truthear x Crinacle Zero:RED", // AutoEQ: oratory1990/in-ear
        category: Category::Earbud,
        preamp_db: -2.1,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: -0.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 6163.0, gain_db: 2.2, q: 2.44 },
            EqBand { filter_type: FilterType::Peaking, freq: 341.0, gain_db: -1.0, q: 1.46 },
            EqBand { filter_type: FilterType::Peaking, freq: 70.0, gain_db: 1.0, q: 0.61 },
            EqBand { filter_type: FilterType::Peaking, freq: 1284.0, gain_db: -0.6, q: 2.04 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -1.8, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 2432.0, gain_db: 0.7, q: 2.98 },
            EqBand { filter_type: FilterType::Peaking, freq: 3588.0, gain_db: -0.6, q: 3.06 },
            EqBand { filter_type: FilterType::Peaking, freq: 8541.0, gain_db: 1.1, q: 3.84 },
            EqBand { filter_type: FilterType::Peaking, freq: 769.0, gain_db: 0.2, q: 3.01 },
        ],
    },
    HeadphoneProfile {
        name: "Dunu Titan S", // AutoEQ: crinacle/711 in-ear
        category: Category::Earbud,
        preamp_db: -5.2,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 10.1, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 5993.0, gain_db: 13.5, q: 0.87 },
            EqBand { filter_type: FilterType::Peaking, freq: 4479.0, gain_db: -9.3, q: 0.51 },
            EqBand { filter_type: FilterType::Peaking, freq: 40.0, gain_db: -7.2, q: 0.34 },
            EqBand { filter_type: FilterType::Peaking, freq: 762.0, gain_db: 2.2, q: 1.04 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -5.4, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 9592.0, gain_db: 2.8, q: 2.45 },
            EqBand { filter_type: FilterType::Peaking, freq: 1411.0, gain_db: -0.8, q: 2.84 },
            EqBand { filter_type: FilterType::Peaking, freq: 2335.0, gain_db: 0.3, q: 1.89 },
            EqBand { filter_type: FilterType::Peaking, freq: 1030.0, gain_db: 0.5, q: 3.30 },
        ],
    },
    HeadphoneProfile {
        name: "Final Audio A4000", // AutoEQ: crinacle/711 in-ear
        category: Category::Earbud,
        preamp_db: -2.9,
        bands: &[
            EqBand { filter_type: FilterType::LowShelf, freq: 105.0, gain_db: 0.5, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 831.0, gain_db: 3.0, q: 1.09 },
            EqBand { filter_type: FilterType::Peaking, freq: 177.0, gain_db: -1.6, q: 1.02 },
            EqBand { filter_type: FilterType::Peaking, freq: 6348.0, gain_db: -3.7, q: 2.04 },
            EqBand { filter_type: FilterType::Peaking, freq: 2022.0, gain_db: 2.0, q: 3.11 },
            EqBand { filter_type: FilterType::HighShelf, freq: 10000.0, gain_db: -3.9, q: 0.70 },
            EqBand { filter_type: FilterType::Peaking, freq: 9639.0, gain_db: 2.1, q: 2.54 },
            EqBand { filter_type: FilterType::Peaking, freq: 3246.0, gain_db: -2.0, q: 3.75 },
            EqBand { filter_type: FilterType::Peaking, freq: 4630.0, gain_db: 0.9, q: 3.78 },
            EqBand { filter_type: FilterType::Peaking, freq: 2482.0, gain_db: 0.9, q: 5.02 },
        ],
    },
];
// ---- END GENERATED PROFILES ----