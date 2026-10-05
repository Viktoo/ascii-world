//! The mixer: every sounding voice placed around the listener's head, one
//! shared room, the ambience beds and the chorus of small singers. Runs on
//! the audio thread (or offline, for tests and `pocket listen`); it only
//! ever hears from the game through `Cmd`s.
//!
//! Hearing where a sound is, all cheap and per voice:
//! - distance: 1/d past a metre, and the far ones carry more room than sound;
//! - left/right: equal-power pan, the far ear a fraction of a millisecond
//!   late and duller (the head's shadow);
//! - front/back: behind is softer and duller, so turning to face a sound
//!   makes it clearer, not just centred;
//! - air: highs fade over distance.

use super::call::{Call, Kind, Material, Part};
use super::dsp::{Delay, OnePole, Rng, Svf, soft_clip};
use super::synth::{Body, Note, Texture};
use glam::Vec3;
use std::f32::consts::{FRAC_PI_4, TAU};

pub const BLOCK: usize = 128;
/// Voices rendered at once; quieter ones wait silently (their time still runs).
pub const MAX_VOICES: usize = 48;
/// Overall level (the soft clip keeps the loudest moments in bounds).
const MASTER: f32 = 2.8;
/// Within this distance a source is at its full loudness.
const REF_DIST: f32 = 2.0;
/// The traveler's own body (steps, brushing) against the world's sounds.
const SELF_GAIN: f32 = 0.22;
/// Below this gain at the listener a voice is skipped.
const INAUDIBLE: f32 = 0.0006;

/// Where an emitter is: a key the game keeps moving, or a fixed point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum At {
    Point(Vec3),
    /// A moving source (a being): starts at the point, follows `Emitters`.
    Follow(u64, Vec3),
    /// The listener's own body (footsteps, brushing): heard inside the head.
    Listener,
}

#[derive(Clone, Debug)]
pub struct Play {
    pub at: At,
    pub call: Call,
    pub body: Body,
    pub seed: u64,
    /// Extra loudness (1 = as written).
    pub gain: f32,
    /// Seconds before it starts.
    pub delay: f32,
}

/// A chorus singer's song and how loud it is.
#[derive(Clone, Debug)]
pub struct Song {
    pub part: Part,
    pub body: Body,
    pub gain: f32,
    /// Seconds between songs (it varies a little).
    pub every: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Ambience {
    /// 0..1: how much wind.
    pub wind: f32,
    /// The nearest open water, and how much of it (0..1).
    pub water: Option<(Vec3, f32)>,
    /// 0..1: something dreadful is near.
    pub dread: f32,
    /// What sings around the listener now (insects at night, small
    /// creatures by day); empty: silence.
    pub songs: Vec<Song>,
    /// How many singers at once.
    pub singers: usize,
    /// Places the singers fall quiet around (point, radius).
    pub hush: Vec<(Vec3, f32)>,
    /// 0..1: rain (anything that patters) falling round the listener, and
    /// whether they hear it from under a roof (a dull drumming).
    pub rain: f32,
    pub rain_roof: bool,
}

#[derive(Clone, Debug)]
pub enum Cmd {
    Listener { pos: Vec3, yaw: f32 },
    Emitters(Vec<(u64, Vec3)>),
    Play(Box<Play>),
    /// Start, move or change an endless texture (0 drive: it fades and ends).
    /// `roar`: a low rumble under the grains (fire), 0..1. `gain`: how loud
    /// it is heard at all (things underfoot are the quietest sounds there are).
    Texture { key: u64, at: At, mat: Material, size: f32, drive: f32, roar: f32, gain: f32 },
    Ambience(Box<Ambience>),
    Volume(f32),
    /// Fade everything out (a world closing).
    Clear,
}

/// Per-voice placement state.
#[derive(Clone)]
struct Space {
    itd: Delay,
    split: [OnePole; 2],
    air: [OnePole; 2],
    /// What reaches the room has crossed the air too (and the room is darker).
    room_air: OnePole,
    g: [f32; 2],
    k: [f32; 2],
    delay: [f32; 2],
    send: f32,
    first: bool,
}

impl Space {
    fn new(sr: f32) -> Space {
        Space { itd: Delay::new(64), split: [OnePole::new(sr, 1500.0); 2], air: [OnePole::new(sr, 18000.0); 2], room_air: OnePole::new(sr, 6000.0), g: [0.0; 2], k: [1.0; 2], delay: [0.0; 2], send: 0.0, first: true }
    }
}

/// What a voice sounds like from the listener: gains, brightness per ear,
/// interaural delay, air cutoff and room send.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub g: [f32; 2],
    pub k: [f32; 2],
    pub delay: [f32; 2],
    pub air: f32,
    pub send: f32,
    /// Overall loudness at the listener (for priority).
    pub level: f32,
}

/// Place a source for a listener at `lis` facing `yaw` (0 = +z, the game's
/// convention: forward (sin yaw, 0, cos yaw), right (cos yaw, 0, -sin yaw)).
pub fn place(lis: Vec3, yaw: f32, src: Vec3, gain: f32, sr: f32) -> Placed {
    let d = src - lis;
    let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let dist = d.length().max(0.01);
    let flat = Vec3::new(d.x, 0.0, d.z);
    let az = flat.dot(right).atan2(flat.dot(fwd));
    // Very near: direction blurs into the head.
    let near = (dist / 1.2).min(1.0);
    let p = az.sin() * near;
    let behind = (-az.cos()).max(0.0) * near;
    let th = (p + 1.0) * FRAC_PI_4;
    // Full loudness within a couple of metres, then 1/d.
    let dgain = gain * REF_DIST / dist.max(REF_DIST);
    let face = 1.0 - 0.3 * behind;
    let g = [th.cos() * dgain * face, th.sin() * dgain * face];
    // Brightness per ear: behind is duller; the far ear duller still.
    let kb = 1.0 - 0.75 * behind;
    let shadow = 0.5 * p.abs();
    let k = if p > 0.0 { [kb * (1.0 - shadow), kb] } else { [kb, kb * (1.0 - shadow)] };
    let itd = 0.00065 * sr * p.abs();
    let delay = if p > 0.0 { [itd, 0.0] } else { [0.0, itd] };
    let air = (18000.0 / (1.0 + dist / 30.0)).max(1500.0);
    // The room: falls far slower than the direct sound, so far is wetter.
    let send = gain * 0.03 / (1.0 + dist / 60.0);
    Placed { g, k, delay, air, send, level: dgain }
}

enum Gen {
    Note(Note),
    Texture(Texture),
}

struct Voice {
    src: Gen,
    at: At,
    pos: Vec3,
    gain: f32,
    /// Samples to wait before it starts.
    wait: u32,
    space: Space,
    /// Texture key (endless voices).
    key: Option<u64>,
    level: f32,
}

/// A small room: four combs and two allpasses a side (Freeverb-lite).
struct Room {
    combs: [[(Vec<f32>, usize, f32); 4]; 2],
    aps: [[(Vec<f32>, usize); 2]; 2],
    damp: f32,
    fb: f32,
}

impl Room {
    fn new(sr: f32) -> Room {
        let s = sr / 44100.0;
        let mk = |n: usize| (vec![0.0; ((n as f32 * s) as usize).max(8)], 0usize, 0.0f32);
        let mka = |n: usize| (vec![0.0; ((n as f32 * s) as usize).max(8)], 0usize);
        let c = [1116, 1188, 1277, 1356];
        let a = [556, 441];
        Room {
            combs: [c.map(mk), c.map(|n| mk(n + 23))],
            aps: [a.map(mka), a.map(|n| mka(n + 23))],
            // Outdoors: the room's tail is dark.
            damp: 0.5,
            fb: 0.8,
        }
    }

    #[inline]
    fn run(&mut self, x: f32) -> (f32, f32) {
        let mut out = [0.0f32; 2];
        for ch in 0..2 {
            let mut y = 0.0;
            for (buf, i, f) in self.combs[ch].iter_mut() {
                let o = buf[*i];
                *f = o * (1.0 - self.damp) + *f * self.damp;
                buf[*i] = x + *f * self.fb;
                *i += 1;
                if *i == buf.len() {
                    *i = 0;
                }
                y += o;
            }
            for (buf, i) in self.aps[ch].iter_mut() {
                let b = buf[*i];
                buf[*i] = y + b * 0.5;
                y = b - y;
                *i += 1;
                if *i == buf.len() {
                    *i = 0;
                }
            }
            out[ch] = y;
        }
        (out[0], out[1])
    }
}

/// Endless beds: wind and the dread drone (not placed: all around).
struct Beds {
    wind: [Svf; 2],
    wind_lvl: f32,
    wind_target: f32,
    gust: f32,
    gust_v: f32,
    dread: f32,
    dread_target: f32,
    ph: [f32; 3],
    dread_noise: Svf,
}

/// A singer of the chorus: a spot near the listener and its song.
struct Singer {
    pos: Vec3,
    song: usize,
    next: f32,
    quiet_until: f32,
    seed: u64,
}

pub struct Mixer {
    pub sr: f32,
    voices: Vec<Voice>,
    lis: Vec3,
    yaw: f32,
    emitters: Vec<(u64, Vec3)>,
    room: Room,
    room_in: Vec<f32>,
    beds: Beds,
    amb: Ambience,
    singers: Vec<Singer>,
    rng: Rng,
    /// Seconds of audio made so far.
    clock: f32,
    volume: f32,
    vol_now: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    tmp: Vec<(f32, f32)>,
    water_key: u64,
    /// How much of the room is heard.
    pub room_gain: f32,
    /// Voices rendered in the last block (for tests and the debug line).
    pub rendered: usize,
}

const WATER_KEY: u64 = u64::MAX - 1;
const RAIN_KEY: u64 = u64::MAX - 2;
/// Rain in the open: fine soft patter, its top rolled off (no hiss).
const RAIN: Material = Material { hard: 0.28, dry: 0.0, ring: 0.0, leafy: 0.0 };
/// Rain on a roof overhead: low and muffled.
const RAIN_ROOF: Material = Material { hard: 0.06, dry: 0.0, ring: 0.0, leafy: 0.0 };

impl Mixer {
    pub fn new(sr: f32) -> Mixer {
        Mixer {
            sr,
            voices: Vec::with_capacity(MAX_VOICES * 2),
            lis: Vec3::ZERO,
            yaw: 0.0,
            emitters: Vec::new(),
            room: Room::new(sr),
            room_in: vec![0.0; BLOCK],
            beds: Beds { wind: [Svf::new(sr, 500.0, 0.8); 2], wind_lvl: 0.0, wind_target: 0.0, gust: 0.5, gust_v: 0.0, dread: 0.0, dread_target: 0.0, ph: [0.0; 3], dread_noise: Svf::new(sr, 90.0, 1.5) },
            amb: Ambience::default(),
            singers: Vec::new(),
            rng: Rng::new(0xA0D10),
            clock: 0.0,
            volume: 0.8,
            vol_now: 0.8,
            buf_l: vec![0.0; BLOCK],
            buf_r: vec![0.0; BLOCK],
            tmp: vec![(0.0, 0.0); BLOCK],
            water_key: WATER_KEY,
            room_gain: 0.08,
            rendered: 0,
        }
    }

    pub fn apply(&mut self, c: Cmd) {
        match c {
            Cmd::Listener { pos, yaw } => {
                self.lis = pos;
                self.yaw = yaw;
            }
            Cmd::Emitters(e) => self.emitters = e,
            Cmd::Play(p) => self.play(*p),
            Cmd::Texture { key, at, mat, size, drive, roar, gain } => self.texture(key, at, mat, size, drive, roar, gain),
            Cmd::Ambience(a) => {
                self.beds.wind_target = a.wind.clamp(0.0, 1.0);
                self.beds.dread_target = a.dread.clamp(0.0, 1.0);
                match a.water {
                    Some((p, amount)) => self.texture(self.water_key, At::Point(p), Material::WATER, 0.6, amount.clamp(0.0, 1.0) * 0.55, 0.0, 1.0),
                    None => self.texture(self.water_key, At::Point(self.lis), Material::WATER, 0.6, 0.0, 0.0, 1.0),
                }
                let (mat, size, k) = if a.rain_roof { (RAIN_ROOF, 0.9, 0.4) } else { (RAIN, 0.2, 0.5) };
                self.texture(RAIN_KEY, At::Listener, mat, size, a.rain.clamp(0.0, 1.0).sqrt() * k, 0.0, 0.8);
                self.amb = *a;
            }
            Cmd::Volume(v) => self.volume = v.clamp(0.0, 1.5),
            Cmd::Clear => {
                self.voices.clear();
                self.singers.clear();
                self.amb = Ambience::default();
                self.beds.wind_target = 0.0;
                self.beds.dread_target = 0.0;
            }
        }
    }

    fn play(&mut self, p: Play) {
        if self.voices.len() > MAX_VOICES * 3 {
            return;
        }
        let (pos, at) = match p.at {
            At::Point(v) => (v, p.at),
            At::Follow(_, v) => (v, p.at),
            At::Listener => (self.lis, p.at),
        };
        let mut t = p.delay.max(0.0);
        for (i, part) in p.call.0.iter().enumerate() {
            let note = Note::new(self.sr, part, p.body, p.seed.wrapping_add(i as u64 * 0x9E37_79B9));
            self.voices.push(Voice { src: Gen::Note(note), at, pos, gain: p.gain, wait: (t * self.sr) as u32, space: Space::new(self.sr), key: None, level: 0.0 });
            // Parts follow one another, a breath apart (knocks run on).
            t += part.span() + if part.kind == Kind::Knock { 0.02 } else { 0.06 };
        }
    }

    fn texture(&mut self, key: u64, at: At, mat: Material, size: f32, drive: f32, roar: f32, gain: f32) {
        let pos = match at {
            At::Point(v) | At::Follow(_, v) => v,
            At::Listener => self.lis,
        };
        if let Some(v) = self.voices.iter_mut().find(|v| v.key == Some(key)) {
            let Gen::Texture(t) = &mut v.src else { return };
            t.target = drive.clamp(0.0, 1.5);
            // Fading out: it stays where it was, as it was.
            if drive <= 0.0 {
                return;
            }
            v.at = at;
            v.gain = gain;
            if !matches!(at, At::Follow(..)) {
                v.pos = pos;
            }
            t.roar = roar;
            t.set_material(mat, size);
            return;
        }
        if drive <= 0.0 {
            return;
        }
        let lfo = if key == self.water_key { 0.13 } else { 0.0 };
        let mut t = Texture::new(self.sr, mat, size, matches!(at, At::Listener), key ^ 0x51ED, lfo, roar);
        t.target = drive.clamp(0.0, 1.5);
        self.voices.push(Voice { src: Gen::Texture(t), at, pos, gain, wait: 0, space: Space::new(self.sr), key: Some(key), level: 0.0 });
    }

    /// The singers: kept near the listener, each singing now and then, and
    /// falling quiet near anything that hushes them.
    fn chorus(&mut self, dt: f32) {
        let want = if self.amb.songs.is_empty() { 0 } else { self.amb.singers.min(32) };
        // Too far: the singer is replaced by one nearer (a field is endless).
        let lis = self.lis;
        self.singers.retain(|s| (s.pos - lis).length() < 45.0);
        self.singers.truncate(want);
        while self.singers.len() < want {
            let a = self.rng.unit() * TAU;
            let r = self.rng.range(4.0, 32.0);
            let pos = lis + Vec3::new(a.sin() * r, self.rng.range(-1.2, 0.2), a.cos() * r);
            let song = (self.rng.next() as usize) % self.amb.songs.len().max(1);
            let every = self.amb.songs.get(song).map(|s| s.every).unwrap_or(1.0);
            let seed = self.rng.next() as u64;
            self.singers.push(Singer { pos, song, next: self.clock + self.rng.unit() * every, quiet_until: 0.0, seed });
        }
        let _ = dt;
        let now = self.clock;
        let mut plays = Vec::new();
        for s in self.singers.iter_mut() {
            let hushed = self.amb.hush.iter().any(|(p, r)| (s.pos - *p).length() < *r);
            if hushed {
                // Quiet at once, and for a while after the thing has gone.
                s.quiet_until = now + 8.0 + (s.seed % 7) as f32;
            }
            if now < s.next {
                continue;
            }
            let Some(song) = self.amb.songs.get(s.song % self.amb.songs.len().max(1)) else { continue };
            s.next = now + song.every * (0.9 + 0.2 * ((s.seed >> 3) % 100) as f32 / 100.0) + song.part.span();
            if now < s.quiet_until {
                continue;
            }
            s.seed = s.seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            // Each singer keeps its own pitch; each song still varies.
            let mut body = song.body;
            body.pitch *= 0.92 + 0.16 * ((s.seed >> 40) % 1000) as f32 / 1000.0;
            plays.push(Play { at: At::Point(s.pos), call: Call(vec![song.part.clone()]), body, seed: s.seed, gain: song.gain, delay: 0.0 });
        }
        for p in plays {
            self.play(p);
        }
    }

    /// Fill `out` (interleaved stereo) with the next samples.
    pub fn process(&mut self, out: &mut [f32]) {
        let mut i = 0;
        while i < out.len() {
            let n = ((out.len() - i) / 2).min(BLOCK);
            if n == 0 {
                break;
            }
            self.block(n);
            for k in 0..n {
                out[i + 2 * k] = self.buf_l[k];
                out[i + 2 * k + 1] = self.buf_r[k];
            }
            i += n * 2;
        }
    }

    fn block(&mut self, n: usize) {
        let sr = self.sr;
        let dt = n as f32 / sr;
        self.clock += dt;
        self.chorus(dt);
        for x in self.buf_l[..n].iter_mut().chain(self.buf_r[..n].iter_mut()) {
            *x = 0.0;
        }
        for x in self.room_in[..n].iter_mut() {
            *x = 0.0;
        }
        // Moving emitters.
        for v in self.voices.iter_mut() {
            if let At::Follow(k, _) = v.at {
                if let Some((_, p)) = self.emitters.iter().find(|(e, _)| *e == k) {
                    v.pos = *p;
                }
            }
        }
        // Priority: how loud each is here.
        let (lis, yaw) = (self.lis, self.yaw);
        for v in self.voices.iter_mut() {
            v.level = match v.at {
                At::Listener => v.gain * SELF_GAIN,
                _ => place(lis, yaw, v.pos, v.gain, sr).level,
            } * match &v.src {
                Gen::Note(nt) => nt.level,
                Gen::Texture(t) => t.drive.max(t.target) + 0.01,
            };
        }
        let mut order: Vec<usize> = (0..self.voices.len()).collect();
        order.sort_by(|a, b| self.voices[*b].level.total_cmp(&self.voices[*a].level));
        let mut rendered = 0;
        for (rank, &vi) in order.iter().enumerate() {
            let v = &mut self.voices[vi];
            if v.wait > 0 {
                let w = (v.wait as usize).min(n);
                v.wait -= w as u32;
                if w == n {
                    continue;
                }
            }
            let audible = rank < MAX_VOICES && v.level > INAUDIBLE;
            // Make the samples (or just let time pass, when it can't be heard).
            let tmp = &mut self.tmp;
            match &mut v.src {
                Gen::Note(nt) => {
                    if audible {
                        for s in tmp[..n].iter_mut() {
                            let x = nt.next();
                            *s = (x, x);
                        }
                    } else {
                        for _ in 0..n {
                            nt.next();
                        }
                        continue;
                    }
                }
                Gen::Texture(t) => {
                    if !audible {
                        t.idle(n);
                        continue;
                    }
                    for s in tmp[..n].iter_mut() {
                        *s = t.next();
                    }
                }
            }
            rendered += 1;
            if v.at == At::Listener {
                // At the ears: no placing, a little room.
                for k in 0..n {
                    self.buf_l[k] += tmp[k].0 * v.gain * SELF_GAIN;
                    self.buf_r[k] += tmp[k].1 * v.gain * SELF_GAIN;
                    self.room_in[k] += (tmp[k].0 + tmp[k].1) * v.gain * 0.01;
                }
                continue;
            }
            let pl = place(lis, yaw, v.pos, v.gain, sr);
            let sp = &mut v.space;
            if sp.first {
                sp.g = pl.g;
                sp.k = pl.k;
                sp.delay = pl.delay;
                sp.send = pl.send;
                sp.first = false;
            }
            for e in 0..2 {
                sp.air[e].set(sr, pl.air);
            }
            sp.room_air.set(sr, pl.air * 0.4);
            let inv = 1.0 / n as f32;
            for k in 0..n {
                let f = k as f32 * inv;
                let x = tmp[k].0;
                sp.itd.push(x);
                for e in 0..2 {
                    let g = sp.g[e] + (pl.g[e] - sp.g[e]) * f;
                    let kk = sp.k[e] + (pl.k[e] - sp.k[e]) * f;
                    let d = sp.delay[e] + (pl.delay[e] - sp.delay[e]) * f;
                    let s = if d > 0.01 { sp.itd.read(d) } else { x };
                    let lo = sp.split[e].run(s);
                    let shaped = lo + (s - lo) * kk;
                    let y = sp.air[e].run(shaped) * g;
                    if e == 0 {
                        self.buf_l[k] += y;
                    } else {
                        self.buf_r[k] += y;
                    }
                }
                self.room_in[k] += sp.room_air.run(x) * (sp.send + (pl.send - sp.send) * f);
            }
            sp.g = pl.g;
            sp.k = pl.k;
            sp.delay = pl.delay;
            sp.send = pl.send;
        }
        self.rendered = rendered;
        self.voices.retain(|v| match &v.src {
            Gen::Note(nt) => !nt.done(),
            Gen::Texture(t) => !t.silent(),
        });
        // Beds, the room, the master.
        let b = &mut self.beds;
        for k in 0..n {
            b.wind_lvl += (b.wind_target - b.wind_lvl) * 0.00002;
            b.dread += (b.dread_target - b.dread) * 0.00001;
            if k == 0 {
                // Gusts: a slow random walk.
                b.gust_v = (b.gust_v + self.rng.bi() * 0.02) * 0.98;
                b.gust = (b.gust + b.gust_v * 0.05).clamp(0.15, 1.0);
                let f = 250.0 + 700.0 * b.gust;
                b.wind[0].set(sr, f, 0.9);
                b.wind[1].set(sr, f * 1.07, 0.9);
            }
            if b.wind_lvl > 0.001 {
                let w = b.wind_lvl * b.gust * 0.05;
                self.buf_l[k] += b.wind[0].bp(self.rng.bi()) * w;
                self.buf_r[k] += b.wind[1].bp(self.rng.bi()) * w;
            }
            if b.dread > 0.001 {
                for (j, f) in [41.2f32, 55.0, 61.9].iter().enumerate() {
                    b.ph[j] = (b.ph[j] + f / sr).fract();
                }
                let slow = (TAU * self.clock * 0.11).sin();
                let d = (b.ph[0] * TAU).sin() + 0.7 * (b.ph[1] * TAU + 0.3 * slow).sin() + 0.5 * (b.ph[2] * TAU).sin();
                let rumble = b.dread_noise.bp(self.rng.bi()) * (0.6 + 0.4 * slow);
                let y = (d * 0.05 + rumble * 0.2) * b.dread * b.dread;
                self.buf_l[k] += y;
                self.buf_r[k] += y * 0.97;
            }
        }
        for k in 0..n {
            let (l, r) = self.room.run(self.room_in[k] * self.room_gain);
            self.vol_now += (self.volume - self.vol_now) * 0.001;
            self.buf_l[k] = soft_clip((self.buf_l[k] + l) * self.vol_now * MASTER);
            self.buf_r[k] = soft_clip((self.buf_r[k] + r) * self.vol_now * MASTER);
        }
    }

    /// Render `secs` offline (tests, `pocket listen`).
    pub fn render(&mut self, secs: f32) -> Vec<f32> {
        let mut out = vec![0.0; ((secs * self.sr) as usize) * 2];
        self.process(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::call::guess;

    const SR: f32 = 48000.0;

    /// Energy per ear and brightness (share of energy above ~2 kHz).
    fn measure(st: &[f32]) -> (f32, f32, f32) {
        let (mut l, mut r) = (0.0, 0.0);
        let mut hi = 0.0;
        let mut tot = 0.0;
        let mut lp = [0.0f32; 4];
        let c = 1.0 - (-TAU * 2500.0 / SR).exp();
        for f in st.chunks(2) {
            l += f[0] * f[0];
            r += f[1] * f[1];
            let m = (f[0] + f[1]) * 0.5;
            // Four one-pole high passes: what is above ~2.5 kHz.
            let mut h = m;
            for p in lp.iter_mut() {
                *p += c * (h - *p);
                h -= *p;
            }
            hi += h * h;
            tot += m * m;
        }
        (l, r, hi / tot.max(1e-12))
    }

    /// The same call, heard from a listener at the origin facing `yaw`,
    /// from a source at `src`.
    fn hear(src: Vec3, yaw: f32) -> (f32, f32, f32) {
        let mut m = Mixer::new(SR);
        m.apply(Cmd::Listener { pos: Vec3::ZERO, yaw });
        let call = guess("a long bleat", 60.0, false).unwrap();
        m.apply(Cmd::Play(Box::new(Play { at: At::Point(src), call, body: Body::default(), seed: 3, gain: 1.0, delay: 0.0 })));
        measure(&m.render(1.6))
    }

    #[test]
    fn behind_then_facing_then_walking_up_gets_louder_and_clearer() {
        // An animal calls 30 m behind (the listener faces +z, it is at -z, a little left).
        let src = Vec3::new(-6.0, 0.0, -30.0);
        let behind = hear(src, 0.0);
        // Turn round to face it.
        let facing = hear(src, std::f32::consts::PI);
        // Walk up to it: 15 m, then 5 m.
        let mid = hear(src * 0.5, std::f32::consts::PI);
        let close = hear(src / 6.0, std::f32::consts::PI);
        let e = |m: (f32, f32, f32)| m.0 + m.1;
        assert!(e(facing) > e(behind) * 1.15, "turning to face it makes it louder: {} vs {}", e(facing), e(behind));
        assert!(facing.2 > behind.2 * 1.3, "and brighter: {} vs {}", facing.2, behind.2);
        assert!(e(mid) > e(facing) * 2.0 && e(close) > e(mid) * 2.0, "walking up, ever louder");
        // Behind and to the left, it is louder in the left ear; once turned round, in the right.
        assert!(behind.0 > behind.1, "left ear first");
        assert!(facing.1 > facing.0, "after turning round, it is on the right");
    }

    #[test]
    fn a_sound_to_the_right_is_in_the_right_ear() {
        let r = hear(Vec3::new(8.0, 0.0, 0.0), 0.0);
        assert!(r.1 > r.0 * 3.0, "{r:?}");
    }

    #[test]
    fn far_sounds_carry_more_room_than_near_ones() {
        let send = |d: f32| {
            let p = place(Vec3::ZERO, 0.0, Vec3::new(0.0, 0.0, d), 1.0, SR);
            p.send / p.level
        };
        assert!(send(40.0) > send(4.0) * 4.0);
    }

    #[test]
    fn a_dense_night_runs_faster_than_real_time() {
        let mut m = Mixer::new(SR);
        m.apply(Cmd::Listener { pos: Vec3::ZERO, yaw: 0.0 });
        let cricket = Part { kind: Kind::Whistle, hz: [4600.0; 3], len: 0.016, times: 4, gap: 0.016, ..Part::default() };
        m.apply(Cmd::Ambience(Box::new(Ambience { wind: 0.6, dread: 0.5, water: Some((Vec3::new(20.0, 0.0, 0.0), 1.0)), songs: vec![Song { part: cricket, body: Body::default(), gain: 0.3, every: 0.6 }], singers: 24, hush: vec![], ..Default::default() })));
        // A herd calling and running through tall grass, with the night all round.
        for i in 0..20u64 {
            let call = guess(if i % 2 == 0 { "a low growl" } else { "two sharp chirps" }, 30.0, false).unwrap();
            m.apply(Cmd::Play(Box::new(Play { at: At::Point(Vec3::new(i as f32 - 10.0, 0.0, 10.0)), call, body: Body::default(), seed: i, gain: 1.0, delay: i as f32 * 0.15 })));
        }
        for i in 0..10u64 {
            m.apply(Cmd::Texture { key: i, at: At::Point(Vec3::new(0.0, 0.0, i as f32 * 2.0)), mat: Material { hard: 0.2, dry: 0.3, ring: 0.0, leafy: 1.0 }, size: 0.5, drive: 0.8, roar: if i == 0 { 0.8 } else { 0.0 }, gain: 1.0 });
        }
        let t0 = std::time::Instant::now();
        let out = m.render(4.0);
        let took = t0.elapsed().as_secs_f32();
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
        // Under a tenth of one core in release (debug builds are far slower).
        let limit = if cfg!(debug_assertions) { 20.0 } else { 0.6 };
        assert!(took < limit, "4 s of a crowded scene took {took:.2} s");
    }

    #[test]
    fn hushed_singers_fall_quiet() {
        let songs = |hush: Vec<(Vec3, f32)>| {
            let mut m = Mixer::new(SR);
            m.apply(Cmd::Listener { pos: Vec3::ZERO, yaw: 0.0 });
            let cricket = Part { kind: Kind::Whistle, hz: [4600.0; 3], len: 0.016, times: 4, gap: 0.016, ..Part::default() };
            m.apply(Cmd::Ambience(Box::new(Ambience { songs: vec![Song { part: cricket, body: Body::default(), gain: 0.3, every: 0.5 }], singers: 20, hush, ..Ambience::default() })));
            let v = m.render(3.0);
            v.iter().map(|x| x * x).sum::<f32>()
        };
        let all = songs(vec![]);
        let hushed = songs(vec![(Vec3::ZERO, 60.0)]);
        assert!(all > 0.0 && hushed < all * 0.05, "{hushed} vs {all}");
    }

    /// Where a sound's energy sits (Hz), from its slope against its level:
    /// high means hiss.
    fn brightness(v: &[f32]) -> (f32, f32) {
        let l: Vec<f32> = v.iter().step_by(2).copied().collect();
        let e: f32 = l.iter().map(|x| x * x).sum::<f32>() / l.len() as f32;
        let d: f32 = l.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>() / l.len() as f32;
        (SR / std::f32::consts::TAU * (d / e.max(1e-12)).sqrt(), e.sqrt())
    }

    #[test]
    fn rain_patters_softly_and_drums_dully_on_a_roof() {
        let hear = |rain: f32, roof: bool| {
            let mut m = Mixer::new(SR);
            m.apply(Cmd::Listener { pos: Vec3::ZERO, yaw: 0.0 });
            m.apply(Cmd::Ambience(Box::new(Ambience { rain, rain_roof: roof, ..Ambience::default() })));
            let v = m.render(4.0);
            brightness(&v[v.len() / 2..])
        };
        let (open, loud) = hear(0.8, false);
        let (roof, _) = hear(0.8, true);
        let (_, soft) = hear(0.15, false);
        let (_, none) = hear(0.0, false);
        let shore = {
            let mut m = Mixer::new(SR);
            m.apply(Cmd::Listener { pos: Vec3::ZERO, yaw: 0.0 });
            m.apply(Cmd::Ambience(Box::new(Ambience { water: Some((Vec3::new(8.0, 0.0, 0.0), 1.0)), ..Ambience::default() })));
            let v = m.render(4.0);
            brightness(&v[v.len() / 2..]).0
        };
        eprintln!("rain open {open:.0} Hz, on a roof {roof:.0} Hz, shore {shore:.0} Hz; loudness {loud:.4} vs light {soft:.4}");
        assert!(loud > soft * 1.3 && soft > none, "more rain is louder");
        assert!(open < 2200.0, "no hiss: {open:.0} Hz");
        assert!(roof < open * 0.7, "a roof dulls it: {roof:.0} vs {open:.0}");
    }
}
