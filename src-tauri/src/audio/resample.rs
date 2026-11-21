//! Streaming windowed-sinc resampler (Blackman window, 16 taps per side at unity ratio).
//! Used only when a file's sample rate differs from the output device's. When downsampling,
//! the kernel is widened and its cutoff lowered so nothing aliases.

use std::f64::consts::PI;

const RES: usize = 256; // kernel table entries per input sample
pub const MAX_CH: usize = 32;

pub struct Resampler {
    ch: usize,
    step: f64,   // input frames advanced per output frame
    taps: usize, // kernel half-width in input frames
    table: Vec<f32>,
    buf: Vec<f32>, // interleaved input history + new input
    pos: f64,      // position of the next output, in frames within `buf`
}

fn kernel(x: f64, cutoff: f64, w: f64) -> f64 {
    if x.abs() >= w {
        return 0.0;
    }
    let sinc = if x.abs() < 1e-9 {
        1.0
    } else {
        let a = PI * cutoff * x;
        a.sin() / a
    };
    let u = x / w;
    let window = 0.42 + 0.5 * (PI * u).cos() + 0.08 * (2.0 * PI * u).cos();
    cutoff * sinc * window
}

impl Resampler {
    pub fn new(in_rate: u32, out_rate: u32, ch: usize) -> Self {
        let cutoff = (out_rate as f64 / in_rate as f64).min(1.0) * 0.975;
        let taps = (16.0 / cutoff).ceil() as usize;
        let w = taps as f64;
        let n = 2 * taps * RES + 2;
        let table = (0..n).map(|j| kernel(j as f64 / RES as f64 - w, cutoff, w) as f32).collect();
        Resampler {
            ch,
            step: in_rate as f64 / out_rate as f64,
            taps,
            table,
            buf: vec![0.0; taps * ch], // leading silence so the first output has full kernel support
            pos: taps as f64,
        }
    }

    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.buf.extend_from_slice(input);
        self.run(out);
    }

    /// Flush the tail at end of stream.
    pub fn finish(&mut self, out: &mut Vec<f32>) {
        self.buf.extend(std::iter::repeat(0.0).take(self.taps * self.ch));
        self.run(out);
    }

    fn run(&mut self, out: &mut Vec<f32>) {
        let ch = self.ch;
        let taps = self.taps as isize;
        let w = self.taps as f64;
        let frames = self.buf.len() / ch;
        let mut acc = [0f32; MAX_CH];

        loop {
            let ip = self.pos.floor() as isize;
            if (ip + taps) as usize >= frames {
                break;
            }
            acc[..ch].iter_mut().for_each(|a| *a = 0.0);
            let mut wsum = 0f32;
            for k in 0..(2 * taps) {
                let i = ip - taps + 1 + k;
                let t = (i as f64 - self.pos + w) * RES as f64;
                let j = t as usize;
                let fr = (t - j as f64) as f32;
                let wt = self.table[j] + (self.table[j + 1] - self.table[j]) * fr;
                wsum += wt;
                let base = i as usize * ch;
                for c in 0..ch {
                    acc[c] += self.buf[base + c] * wt;
                }
            }
            let inv = if wsum.abs() > 1e-6 { 1.0 / wsum } else { 1.0 };
            for c in 0..ch {
                out.push(acc[c] * inv);
            }
            self.pos += self.step;
        }

        // Drop input frames the kernel no longer needs.
        let drop = (self.pos.floor() as isize - taps + 1).max(0) as usize;
        if drop > 0 {
            self.buf.drain(..drop * ch);
            self.pos -= drop as f64;
        }
    }
}
