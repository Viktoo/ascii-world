//! The four kinds of part, made sample by sample: a throat (source through
//! formants), a whistle, noise grains and a struck, ringing knock. Plus the
//! endless textures (brushing, water) whose intensity the game moves.
//!
//! A `Note` plays one part with all its repeats; every repeat is a little
//! different (pitch, timing, loudness), drawn from the note's own seed.

use super::call::{Kind, Material, Part, formant_scale, voice_hz, whistle_hz};
use super::dsp::{Delay, OnePole, Rng, Svf, ar, blep, smooth};
use std::f32::consts::TAU;

/// Formants (Hz) of a grown person's vowels; scaled by body size.
fn vowel_formants(c: char) -> [f32; 4] {
    match c {
        'a' => [800.0, 1200.0, 2500.0, 3500.0],
        'e' => [500.0, 1850.0, 2500.0, 3500.0],
        'i' => [300.0, 2300.0, 3000.0, 3700.0],
        'o' => [500.0, 850.0, 2450.0, 3400.0],
        'u' => [330.0, 750.0, 2300.0, 3300.0],
        'm' => [250.0, 1100.0, 2300.0, 3300.0],
        'n' => [250.0, 1600.0, 2500.0, 3400.0],
        _ => [550.0, 1100.0, 2450.0, 3400.0],
    }
}

const FORMANT_GAIN: [f32; 4] = [1.0, 0.55, 0.3, 0.12];
const FORMANT_Q: [f32; 4] = [5.0, 7.0, 9.0, 10.0];

/// What the body adds to a part: its mass (register) and how far this one
/// individual sits from its kind (bigger: lower).
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub mass: f32,
    /// Pitch multiplier of this individual (1 = the species' own).
    pub pitch: f32,
}

impl Default for Body {
    fn default() -> Body {
        Body { mass: 60.0, pitch: 1.0 }
    }
}

const MAX_GRAINS: usize = 24;
const MODES: usize = 5;

#[derive(Clone, Copy, Default)]
struct Grain {
    left: u32,
    len: u32,
    atk: u32,
    amp: f32,
    crackle: bool,
}

/// Grain settings for a material (the hand-tuned mapping: soft stuff is
/// low, smooth and quiet; brittle stuff high, jumpy and crackling).
#[derive(Clone, Copy, Debug)]
pub struct GrainSet {
    pub band: f32,
    pub q: f32,
    pub glen: (f32, f32),
    pub attack: f32,
    pub spread: f32,
    pub crackle: f32,
    pub rate: f32,
    pub swish: f32,
    pub level: f32,
    pub cut: f32,
}

impl GrainSet {
    pub fn of(m: Material, size: f32) -> GrainSet {
        let (hard, dry) = (m.hard.clamp(0.0, 1.0), m.dry.clamp(0.0, 1.0));
        let size = size.clamp(0.0, 1.0);
        GrainSet {
            band: (600.0 + 3400.0 * hard.powf(1.2)) * (1.25 - 0.5 * size),
            q: 0.5 + 0.7 * dry,
            glen: (0.003 + 0.030 * (1.0 - hard), 0.008 + 0.045 * (1.0 - hard)),
            attack: 0.001 + 0.35 * (1.0 - hard),
            spread: 0.15 + 1.1 * dry,
            crackle: dry * dry * hard,
            rate: 500.0 + 700.0 * (1.0 - hard),
            swish: 0.6 * (1.0 - hard) + 0.1,
            level: 0.35 + 0.65 * hard.max(dry),
            cut: 2500.0 + 15000.0 * dry,
        }
    }
}

/// Noise in grains: shared by noise parts, footfalls and endless textures.
#[derive(Clone)]
struct Grains {
    set: GrainSet,
    grains: [Grain; MAX_GRAINS],
    band: [Svf; 2],
    crk: [Svf; 2],
    swish: [OnePole; 2],
    cut: [Svf; 2],
    /// Two independent streams (left, right) for sounds at the listener.
    wide: bool,
    /// Grains sounding now.
    live: usize,
}

impl Grains {
    fn new(sr: f32, set: GrainSet, wide: bool) -> Grains {
        Grains {
            set,
            grains: [Grain::default(); MAX_GRAINS],
            band: [Svf::new(sr, set.band, set.q); 2],
            crk: [Svf::new(sr, 5200.0, 1.2); 2],
            swish: [OnePole::new(sr, set.band * 0.6); 2],
            cut: [Svf::new(sr, set.cut, 0.7); 2],
            wide,
            live: 0,
        }
    }

    /// One sample (left, right) at `drive` 0..1 (how hard it's pushed).
    #[inline]
    fn next(&mut self, sr: f32, rng: &mut Rng, drive: f32) -> (f32, f32) {
        let s = self.set;
        // New grains at a rate following the drive.
        let p = s.rate * drive / sr;
        if p > 0.0 && rng.unit() < p {
            if let Some(g) = self.grains.iter_mut().find(|g| g.left == 0) {
                let crackle = rng.unit() < s.crackle;
                let len = if crackle { rng.range(0.002, 0.006) } else { rng.range(s.glen.0, s.glen.1) };
                let len = ((len * sr) as u32).max(2);
                let amp = (rng.gauss(s.spread)).exp() * 0.25 * if crackle { 1.2 } else { 1.0 };
                let atk = ((len as f32 * if crackle { 0.05 } else { s.attack }) as u32).max(1);
                *g = Grain { left: len, len, atk, amp, crackle };
                self.live += 1;
            }
        }
        let mut env_b = [0.0f32; 2];
        let mut env_c = [0.0f32; 2];
        let mut live = 0;
        for (i, g) in self.grains.iter_mut().enumerate() {
            if live == self.live {
                break;
            }
            if g.left == 0 {
                continue;
            }
            live += 1;
            let k = g.len - g.left;
            let w = if k < g.atk {
                let x = k as f32 / g.atk as f32;
                (x * std::f32::consts::FRAC_PI_2).sin().powi(2)
            } else {
                (-((k - g.atk) as f32) / ((g.len - g.atk) as f32 / 3.0 + 1.0)).exp()
            };
            g.left -= 1;
            if g.left == 0 {
                self.live -= 1;
                live -= 1;
            }
            // Grains alternate sides when wide.
            let side = if self.wide { i & 1 } else { 0 };
            if g.crackle {
                env_c[side] += g.amp * w;
            } else {
                env_b[side] += g.amp * w;
            }
        }
        let mut out = [0.0f32; 2];
        let sides = if self.wide { 2 } else { 1 };
        for c in 0..sides {
            let b = self.band[c].bp(rng.bi()) * env_b[c];
            let k = self.crk[c].bp(rng.bi()) * env_c[c] * 1.2;
            let sw = self.swish[c].run(rng.bi()) * drive * s.swish * 0.3;
            out[c] = self.cut[c].lp(b + k + sw) * s.level;
        }
        if self.wide { (out[0], out[1]) } else { (out[0], out[0]) }
    }
}

/// One struck, ringing mode: a decaying rotation (cheap and exact).
#[derive(Clone, Copy, Default)]
struct Mode {
    re: f32,
    im: f32,
    c: f32,
    s: f32,
    decay: f32,
}

impl Mode {
    #[inline]
    fn next(&mut self) -> f32 {
        let re = self.re * self.c - self.im * self.s;
        let im = self.re * self.s + self.im * self.c;
        self.re = re * self.decay;
        self.im = im * self.decay;
        self.im
    }
}

/// The numbers of a part read every sample (copied out of it once).
#[derive(Clone, Copy)]
struct Knobs {
    kind: Kind,
    times: u32,
    gap: u32,
    rough: f32,
    breath: f32,
    nasal: f32,
    wobble: f32,
    swell: f32,
    dry: f32,
    hard: f32,
    ring: f32,
}

/// Plays one part (all its repeats) for one listener-side voice.
pub struct Note {
    sr: f32,
    part: Knobs,
    rng: Rng,
    /// Samples played so far and the note's whole length.
    t: u32,
    total: u32,
    // Per-repeat
    rep: u32,
    rep_start: u32,
    rep_len: u32,
    rep_hz: f32,
    rep_amp: f32,
    // Pitch
    base: [f32; 3],
    phase: f32,
    cyc: u32,
    jitter: f32,
    drift: f32,
    // Throat
    formants: [Svf; 4],
    vowels: Vec<[f32; 4]>,
    fscale: f32,
    body_lp: Svf,
    tube: Delay,
    tube_d: f32,
    breath_bp: Svf,
    // Noise / knock
    grains: Option<Grains>,
    modes: [Mode; MODES],
    click: Svf,
    click_left: u32,
    thud: Svf,
    thud_env: f32,
    thud_k: f32,
    tail: u32,
    pub level: f32,
}

impl Note {
    pub fn new(sr: f32, part: &Part, body: Body, seed: u64) -> Note {
        let mut rng = Rng::new(seed);
        let p = part.clone();
        let auto = match p.kind {
            Kind::Voice => voice_hz(body.mass),
            Kind::Whistle => whistle_hz(body.mass),
            Kind::Knock => (420.0 * (1.0 / body.mass.max(0.01)).powf(1.0 / 6.0) * (0.5 + p.hard)).clamp(60.0, 4000.0),
            Kind::Noise => 0.0,
        };
        let base = if p.hz[0] > 0.0 { p.hz } else { [auto; 3] };
        let base = base.map(|h| h * body.pitch);
        let fscale = formant_scale(body.mass) * body.pitch.powf(0.8) * rng.range(0.96, 1.04);
        let mut vowels: Vec<[f32; 4]> = p.vowel.chars().map(vowel_formants).collect();
        if vowels.is_empty() {
            vowels.push(vowel_formants('_'));
        }
        let rep_len = ((p.len * sr) as u32).max(2);
        let gap = (p.gap * sr) as u32;
        // A knock rings past its strike; its tail is part of it.
        let ring_s = 0.015 + 2.2 * p.ring * p.ring + 0.04 * (1.0 - p.hard);
        let tail = match p.kind {
            Kind::Knock => (ring_s * 4.0 * sr) as u32,
            Kind::Voice => (0.12 * sr) as u32,
            _ => (0.05 * sr) as u32,
        };
        let total = rep_len * p.times as u32 + gap * (p.times as u32).saturating_sub(1) + tail;
        let grains = match p.kind {
            Kind::Noise => {
                let m = Material { hard: p.hard, dry: p.dry, ring: p.ring, leafy: 0.0 };
                let mut set = GrainSet::of(m, 0.5);
                if p.hz[0] > 0.0 {
                    set.band = p.hz[0];
                }
                Some(Grains::new(sr, set, false))
            }
            _ => None,
        };
        let tube_d = sr / (fscale * 520.0).clamp(60.0, 4000.0);
        let mut n = Note {
            sr,
            rng,
            t: 0,
            total,
            rep: 0,
            rep_start: 0,
            rep_len,
            rep_hz: 1.0,
            rep_amp: 1.0,
            base,
            phase: 0.0,
            cyc: 0,
            jitter: 1.0,
            drift: 0.0,
            formants: [Svf::default(); 4],
            vowels,
            fscale,
            body_lp: Svf::new(sr, (base[0] * 4.0).max(120.0), 0.7),
            tube: Delay::new(tube_d as usize + 4),
            tube_d,
            breath_bp: Svf::new(sr, base[0].max(200.0), 4.0),
            grains,
            modes: [Mode::default(); MODES],
            click: Svf::new(sr, 1000.0 + 9000.0 * p.hard, 0.7),
            click_left: 0,
            thud: Svf::new(sr, 160.0, 0.8),
            thud_env: 0.0,
            thud_k: (-1.0 / (0.03 * sr)).exp(),
            tail,
            // Loudness spans about 30 dB: a purr is truly quiet, a roar is not.
            level: 0.05 + 1.6 * p.loud.powf(1.5),
            part: Knobs { kind: p.kind, times: p.times as u32, gap: (p.gap * sr) as u32, rough: p.rough, breath: p.breath, nasal: p.nasal, wobble: p.wobble, swell: p.swell, dry: p.dry, hard: p.hard, ring: p.ring },
        };
        n.set_formants(0.0);
        n.trigger();
        n
    }

    pub fn done(&self) -> bool {
        self.t >= self.total
    }

    fn set_formants(&mut self, u: f32) {
        let n = self.vowels.len();
        let f = if n == 1 {
            self.vowels[0]
        } else {
            let x = u.clamp(0.0, 0.999) * (n - 1) as f32;
            let i = x as usize;
            let k = smooth(x - i as f32);
            let (a, b) = (self.vowels[i], self.vowels[(i + 1).min(n - 1)]);
            [0, 1, 2, 3].map(|j| a[j] + (b[j] - a[j]) * k)
        };
        for (j, svf) in self.formants.iter_mut().enumerate() {
            svf.set(self.sr, f[j] * self.fscale, FORMANT_Q[j]);
        }
    }

    /// Start a repeat: each one a little different.
    fn trigger(&mut self) {
        let r = &mut self.rng;
        self.rep_hz = 1.0 + r.gauss(0.025);
        self.rep_amp = (1.0 + r.gauss(0.12)).clamp(0.6, 1.3);
        if self.part.kind == Kind::Knock {
            let ring = self.part.ring;
            let hard = self.part.hard;
            // Ratios between wood-like (near harmonic, short) and metal-like (inharmonic, long).
            let wood = [1.0, 2.32, 3.86, 5.21, 6.9];
            let metal = [1.0, 2.76, 5.40, 8.93, 13.3];
            let f0 = self.base[0] * self.rep_hz;
            let ring_s = 0.015 + 2.2 * ring * ring + 0.04 * (1.0 - hard);
            for i in 0..MODES {
                let ratio = wood[i] + (metal[i] - wood[i]) * ring;
                let f = (f0 * ratio * (1.0 + r.gauss(0.02))).min(self.sr * 0.45);
                let w = TAU * f / self.sr;
                let tau = ring_s / (1.0 + 0.7 * i as f32);
                let amp = (0.35 + 0.6 * hard).powi(i as i32) * self.rep_amp;
                let ph = r.unit() * TAU;
                self.modes[i] = Mode { re: amp * ph.cos(), im: amp * ph.sin(), c: w.cos(), s: w.sin(), decay: (-1.0 / (tau * self.sr)).exp() };
            }
            self.click_left = ((0.0005 + 0.006 * (1.0 - hard)) * self.sr) as u32;
            self.thud_env = (1.0 - 0.6 * hard) * self.rep_amp;
        }
    }

    #[inline]
    fn pitch(&self, u: f32) -> f32 {
        let b = self.base;
        let f = if u < 0.5 { b[0] + (b[1] - b[0]) * smooth(u * 2.0) } else { b[1] + (b[2] - b[1]) * smooth(u * 2.0 - 1.0) };
        f * self.rep_hz
    }

    /// The next sample (mono). Zero once done.
    pub fn next(&mut self) -> f32 {
        if self.t >= self.total {
            return 0.0;
        }
        let p = self.part;
        // Move to the next repeat when this one and its gap are over.
        if self.rep + 1 < p.times && self.t >= self.rep_start + self.rep_len + p.gap {
            self.rep += 1;
            self.rep_start = self.t;
            self.trigger();
        }
        let local = self.t - self.rep_start;
        self.t += 1;
        let u = local as f32 / self.rep_len as f32;
        let sr = self.sr;
        let out = match p.kind {
            Kind::Knock => {
                let mut s = 0.0;
                for m in self.modes.iter_mut() {
                    s += m.next();
                }
                let click = if self.click_left > 0 {
                    self.click_left -= 1;
                    self.click.lp(self.rng.bi()) * (0.4 + p.dry * 0.8)
                } else {
                    0.0
                };
                self.thud_env *= self.thud_k;
                let thud = self.thud.lp(self.rng.bi()) * self.thud_env * 2.0;
                (s * 0.6 + click + thud) * self.level * 0.4
            }
            Kind::Noise => {
                let env = ar(u, 0.03 + 0.4 * p.swell, 0.35) * self.rep_amp;
                let (l, _) = self.grains.as_mut().map(|g| g.next(sr, &mut self.rng, env)).unwrap_or((0.0, 0.0));
                l * self.level * 0.5
            }
            Kind::Whistle => {
                if u >= 1.0 {
                    return 0.0;
                }
                let tt = self.t as f32 / sr;
                let mut f = self.pitch(u) * (1.0 + p.wobble * 0.04 * (TAU * 6.0 * tt).sin());
                // Rough whistles buzz (a fast flutter, like a bird's buzzy note).
                f *= 1.0 + p.rough * 0.22 * (TAU * (70.0 + 50.0 * p.rough) * tt).sin();
                self.phase = (self.phase + f / sr).fract();
                let ph = self.phase * TAU;
                let tone = ph.sin() + 0.07 * (2.0 * ph).sin() + 0.015 * (3.0 * ph).sin();
                self.breath_bp.set(sr, f, 3.0);
                let air = self.breath_bp.bp(self.rng.bi()) * p.breath * 1.5;
                let env = ar(u, 0.04 + 0.4 * p.swell, 0.3) * self.rep_amp;
                (tone * (1.0 - 0.6 * p.breath) + air) * env * self.level * 0.5
            }
            Kind::Voice => {
                if u >= 1.0 + self.tail as f32 / self.rep_len as f32 {
                    return 0.0;
                }
                let tt = self.t as f32 / sr;
                if local % 32 == 0 {
                    self.set_formants(u);
                }
                // Pitch: the contour, vibrato, a slow wander and per-cycle jitter.
                self.drift = (self.drift + self.rng.bi() * 0.02) * 0.995;
                let f = self.pitch(u.min(1.0)) * (1.0 + p.wobble * 0.05 * (TAU * 5.5 * tt).sin()) * (1.0 + p.rough * 0.04 * self.drift) * self.jitter;
                let dt = (f / sr).min(0.5);
                self.phase += dt;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                    self.cyc += 1;
                    self.jitter = 1.0 + self.rng.gauss(0.012 + 0.03 * p.rough);
                }
                let saw = 2.0 * self.phase - 1.0 - blep(self.phase, dt);
                // Roughness: every other cycle weaker (period doubling).
                let src = saw * if self.cyc & 1 == 1 { 1.0 - 0.6 * p.rough } else { 1.0 };
                let noise = self.rng.bi();
                let exc = src * (1.0 - p.breath) + noise * p.breath * 2.2 * (1.0 + 0.5 * p.rough * src.abs());
                let mut y = 0.0;
                for (j, svf) in self.formants.iter_mut().enumerate() {
                    y += svf.bp(exc) * FORMANT_GAIN[j];
                }
                // Long tracts (horns, trunks, whines): a tube resonance.
                if p.nasal > 0.0 {
                    let fb = self.tube.read(self.tube_d);
                    self.tube.push(y + fb * 0.55 * p.nasal);
                    y += fb * p.nasal * 0.8;
                }
                // Very low voices keep their body (a rumble felt more than heard).
                let body = self.body_lp.lp(src) * (1.0 - p.breath) * 0.5;
                let env = ar(u, 0.03 + 0.45 * p.swell, 0.3) * self.rep_amp;
                ((y * 1.6 + body) * env) * self.level * 0.6
            }
        };
        if out.is_finite() { out } else { 0.0 }
    }
}

/// An endless texture (brushing through growth, lapping water, a crackling
/// fire) whose drive the game sets every frame.
pub struct Texture {
    sr: f32,
    grains: Grains,
    rng: Rng,
    pub drive: f32,
    pub target: f32,
    /// Slow swell for water and wind-like textures (0: none).
    lfo: f32,
    lfo_ph: f32,
    /// A low rumble under the grains (a fire's roar), 0..1.
    pub roar: f32,
    roar_lp: [Svf; 2],
    flicker: f32,
}

impl Texture {
    pub fn new(sr: f32, m: Material, size: f32, wide: bool, seed: u64, lfo: f32, roar: f32) -> Texture {
        let mut set = GrainSet::of(m, size);
        if roar > 0.0 {
            Self::burning(&mut set);
        }
        Texture { sr, grains: Grains::new(sr, set, wide), rng: Rng::new(seed), drive: 0.0, target: 0.0, lfo, lfo_ph: 0.0, roar, roar_lp: [Svf::new(sr, 220.0, 0.6), Svf::new(sr, 240.0, 0.6)], flicker: 1.0 }
    }

    /// Burning: sparse sharp pops and a soft hiss over the roar, not a
    /// dense crackle (fire spits now and then; it doesn't fizz).
    fn burning(set: &mut GrainSet) {
        set.rate = 45.0;
        set.crackle = 0.85;
        set.spread = 1.2;
        set.swish = 0.12;
        set.level = 1.1;
    }

    pub fn set_material(&mut self, m: Material, size: f32) {
        let mut set = GrainSet::of(m, size);
        if self.roar > 0.0 {
            Self::burning(&mut set);
        }
        self.grains.set = set;
        for c in 0..2 {
            self.grains.band[c].set(self.sr, set.band, set.q);
            self.grains.cut[c].set(self.sr, set.cut, 0.7);
            self.grains.swish[c].set(self.sr, set.band * 0.6);
        }
    }

    #[inline]
    pub fn next(&mut self) -> (f32, f32) {
        self.drive += (self.target - self.drive) * 0.0015;
        let mut d = self.drive;
        if self.lfo > 0.0 {
            self.lfo_ph = (self.lfo_ph + self.lfo / self.sr).fract();
            d *= 0.55 + 0.45 * (TAU * self.lfo_ph).sin();
        }
        let (l, r) = self.grains.next(self.sr, &mut self.rng, d);
        if self.roar > 0.0 {
            // Flames: a breathing low roar that flickers with the crackle.
            self.flicker = (self.flicker + self.rng.bi() * 0.004).clamp(0.4, 1.3);
            let k = self.roar * d * self.flicker * 0.9;
            let (a, b) = (self.rng.bi(), self.rng.bi());
            return (l * 1.6 + self.roar_lp[0].lp(a) * k, r * 1.6 + self.roar_lp[1].lp(b) * k);
        }
        (l * 1.6, r * 1.6)
    }

    /// Let `n` samples pass unheard (only the drive moves).
    pub fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.drive += (self.target - self.drive) * 0.0015;
        }
    }

    pub fn silent(&self) -> bool {
        self.target <= 0.0 && self.drive < 0.002
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::call::guess;

    fn render(part: &Part, body: Body, seed: u64) -> Vec<f32> {
        let mut n = Note::new(48000.0, part, body, seed);
        let mut v = Vec::new();
        while !n.done() {
            v.push(n.next());
        }
        v
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    #[test]
    fn every_kind_sounds_finite_and_not_silent() {
        for kind in [Kind::Voice, Kind::Whistle, Kind::Noise, Kind::Knock] {
            for mass in [0.1, 4.5, 70.0, 5000.0] {
                let p = Part { kind, ..Part::default() };
                let v = render(&p, Body { mass, pitch: 1.0 }, 7);
                assert!(v.iter().all(|x| x.is_finite()), "{kind:?} {mass}");
                let r = rms(&v);
                assert!(r > 1e-3 && r < 2.0, "{kind:?} at {mass} kg: rms {r}");
            }
        }
    }

    #[test]
    fn seeds_vary_a_call_and_repeat_exactly() {
        let c = guess("meow", 4.5, false).unwrap();
        let a = render(&c.0[0], Body::default(), 1);
        let b = render(&c.0[0], Body::default(), 1);
        let d = render(&c.0[0], Body::default(), 2);
        assert_eq!(a, b);
        assert_ne!(a, d);
    }
}

#[cfg(test)]
mod calibrate {
    use super::*;

    #[test]
    #[ignore]
    fn levels_per_kind() {
        for kind in [Kind::Voice, Kind::Whistle, Kind::Noise, Kind::Knock] {
            for mass in [4.5f32, 70.0, 2000.0] {
                let p = Part { kind, loud: 0.5, ..Part::default() };
                let mut n = Note::new(48000.0, &p, Body { mass, pitch: 1.0 }, 3);
                let mut v = Vec::new();
                while !n.done() {
                    v.push(n.next());
                }
                let pk = v.iter().fold(0.0f32, |a, x| a.max(x.abs()));
                let rms = (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
                eprintln!("{kind:?} {mass}: peak {:.1} dB rms {:.1} dB", 20.0 * pk.log10(), 20.0 * rms.log10());
            }
        }
    }
}
