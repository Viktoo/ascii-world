//! Small DSP parts: a noise source, filters, a fractional delay. All of it
//! allocation-free once built, f32, and cheap enough for dozens of voices.

use std::f32::consts::{PI, TAU};

/// A fast deterministic generator (xorshift32): noise and per-play jitter.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let s = (seed ^ (seed >> 32)) as u32;
        Rng(if s == 0 { 0x9E37_79B9 } else { s })
    }
    #[inline]
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform in [0, 1).
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    /// Uniform in [-1, 1).
    #[inline]
    pub fn bi(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    /// Roughly normal (sum of three uniforms), mean 0, sd about `sd`.
    pub fn gauss(&mut self, sd: f32) -> f32 {
        (self.bi() + self.bi() + self.bi()) * 0.577 * sd
    }
}

/// Topology-preserving state-variable filter: low and band pass (unit peak)
/// from one set of coefficients. The cutoff may move every sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SvfOut {
    pub lp: f32,
    pub bp: f32,
}

impl Svf {
    pub fn new(sr: f32, f: f32, q: f32) -> Svf {
        let mut s = Svf::default();
        s.set(sr, f, q);
        s
    }
    #[inline]
    pub fn set(&mut self, sr: f32, f: f32, q: f32) {
        let g = (PI * f.clamp(10.0, sr * 0.45) / sr).tan();
        let k = 1.0 / q.max(0.05);
        self.k = k;
        self.a1 = 1.0 / (1.0 + g * (g + k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    #[inline]
    pub fn run(&mut self, x: f32) -> SvfOut {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        SvfOut { lp: v2, bp: v1 * self.k }
    }
    #[inline]
    pub fn bp(&mut self, x: f32) -> f32 {
        self.run(x).bp
    }
    #[inline]
    pub fn lp(&mut self, x: f32) -> f32 {
        self.run(x).lp
    }
}

/// One-pole low pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    pub y: f32,
    pub c: f32,
}

impl OnePole {
    pub fn new(sr: f32, f: f32) -> OnePole {
        let mut o = OnePole::default();
        o.set(sr, f);
        o
    }
    #[inline]
    pub fn set(&mut self, sr: f32, f: f32) {
        self.c = 1.0 - (-TAU * f.max(1.0) / sr).exp();
    }
    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        self.y += self.c * (x - self.y);
        self.y
    }
}

/// A short delay line read at a fractional position (interaural delay,
/// tube resonance).
#[derive(Clone, Debug)]
pub struct Delay {
    buf: Vec<f32>,
    w: usize,
}

impl Delay {
    pub fn new(len: usize) -> Delay {
        Delay { buf: vec![0.0; len.max(2).next_power_of_two()], w: 0 }
    }
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.w = (self.w + 1) & (self.buf.len() - 1);
        self.buf[self.w] = x;
    }
    /// The sample `d` samples ago (0 = the newest), linearly interpolated.
    #[inline]
    pub fn read(&self, d: f32) -> f32 {
        let m = self.buf.len() - 1;
        let d = d.clamp(0.0, (m - 1) as f32);
        let i = d as usize;
        let fr = d - i as f32;
        let a = self.buf[(self.w + self.buf.len() - i) & m];
        let b = self.buf[(self.w + self.buf.len() - i - 1) & m];
        a + (b - a) * fr
    }
}

/// Band-limited sawtooth step (polyBLEP) for a phase in [0, 1) advancing by `dt`.
#[inline]
pub fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// Smooth 0→1 over [0, 1].
#[inline]
pub fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Attack / sustain / release envelope over a note's normalised time `u`.
#[inline]
pub fn ar(u: f32, atk: f32, rel: f32) -> f32 {
    if !(0.0..1.0).contains(&u) {
        return 0.0;
    }
    if u < atk {
        let s = (0.5 * PI * u / atk.max(1e-4)).sin();
        s * s
    } else if u > 1.0 - rel {
        let c = (0.5 * PI * (u - (1.0 - rel)) / rel.max(1e-4)).cos();
        c * c
    } else {
        1.0
    }
}

/// Soft limiter for the master bus.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    if x.abs() < 0.6 {
        x
    } else {
        x.signum() * (0.6 + 0.4 * ((x.abs() - 0.6) / 0.4).tanh())
    }
}
