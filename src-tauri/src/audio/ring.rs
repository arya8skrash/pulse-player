//! Single-producer / single-consumer ring buffer of f32 samples, lock-free and unsafe-free
//! (samples are stored as atomic u32 bit patterns). The pipeline thread pushes, the cpal
//! callback pops. Counters are in *samples* (frames * channels).

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub struct Ring {
    buf: Vec<AtomicU32>,
    cap: usize,
    head: AtomicUsize, // read position (consumer)
    tail: AtomicUsize, // write position (producer)
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        Ring {
            buf: (0..cap).map(|_| AtomicU32::new(0)).collect(),
            cap,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.tail.load(Ordering::Acquire).wrapping_sub(self.head.load(Ordering::Acquire))
    }

    pub fn free(&self) -> usize {
        self.cap - self.len()
    }

    /// Producer: all-or-nothing, so the consumer always sees whole frames.
    pub fn push(&self, data: &[f32]) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if self.cap - tail.wrapping_sub(head) < data.len() {
            return false;
        }
        for (i, s) in data.iter().enumerate() {
            self.buf[(tail + i) % self.cap].store(s.to_bits(), Ordering::Relaxed);
        }
        self.tail.store(tail.wrapping_add(data.len()), Ordering::Release);
        true
    }

    /// Consumer: fills as much of `out` as is available, returns samples written.
    pub fn pop(&self, out: &mut [f32]) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let n = out.len().min(tail.wrapping_sub(head));
        for (i, o) in out.iter_mut().take(n).enumerate() {
            *o = f32::from_bits(self.buf[(head + i) % self.cap].load(Ordering::Relaxed));
        }
        self.head.store(head.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer: drop everything currently buffered. Returns samples discarded.
    pub fn discard_all(&self) -> usize {
        let tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Relaxed);
        self.head.store(tail, Ordering::Release);
        tail.wrapping_sub(head)
    }
}
