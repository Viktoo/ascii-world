//! What the world sounds like, read off the sim once a frame: the listener,
//! who is where, footsteps, brushing through growth, the sim's own cues
//! (calls, bumps, impacts), and the beds and chorus around (wind, water,
//! dread, the night's singers). Nothing here changes the sim.

use super::call::{Call, Kind, Material, Part};
use super::mix::{Ambience, At, Cmd, Play, Song};
use super::synth::Body;
use super::{Cue, Heard, actor_key};
use crate::sim::{ActorId, Sim};
use crate::terrain::WATER_LEVEL;
use glam::Vec3;
use std::collections::{HashMap, HashSet};

/// Footsteps and brushing further than this aren't worth a voice: they
/// are the most common sounds there are, and among the quietest.
const STEP_RANGE: f32 = 20.0;
/// How loud brushing through growth is heard (it happens all the time, so
/// it sits just above silence: a soft rustle, never a feature).
pub const UNDERFOOT: f32 = 0.12;
/// And footsteps.
const STEPS: f32 = 0.45;
/// Calls and knocks further than this are dropped.
const HEAR_RANGE: f32 = 140.0;

#[derive(Default)]
pub struct Foley {
    /// Per walker: where it was, how far it has gone since its last step,
    /// and when that step was.
    walk: HashMap<i64, (Vec3, f32, f32)>,
    /// Seconds of sound so far.
    clock: f32,
    /// Brushing textures sounding last frame.
    brushing: HashSet<u64>,
    mats: HashMap<u32, Material>,
    since_amb: f32,
    since_fire: f32,
    /// Fires crackling last time (texture keys).
    fires: HashSet<u64>,
    n: u64,
}

fn mix_seed(a: u64, b: u64) -> u64 {
    (a ^ b.rotate_left(29)).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (a >> 17)
}

impl Foley {
    /// The material of a scatter or thing type (cached).
    fn material(&mut self, sim: &mut Sim, type_id: u32) -> Option<Material> {
        if let Some(m) = self.mats.get(&type_id) {
            return Some(*m);
        }
        let ty = sim.snap.type_of(type_id)?.clone();
        let props = sim.type_props.get(&sim.vocab, &ty);
        let m = Material::of(&props, &ty.ct.meta.tags, Material::from_meta(&ty.ct.meta.sound).as_ref());
        self.mats.insert(type_id, m);
        Some(m)
    }

    /// A type's mass at scale 1.
    fn mass(&mut self, sim: &mut Sim, type_id: u32) -> f32 {
        let Some(ty) = sim.snap.type_of(type_id).cloned() else { return 1.0 };
        sim.type_props.get(&sim.vocab, &ty)[crate::sim::props::P_MASS]
    }

    /// One frame: everything the mixer needs to hear the world now.
    pub fn frame(&mut self, sim: &mut Sim, dt: f32, out: &mut Vec<Cmd>) {
        self.n += 1;
        self.clock += dt;
        let seed = sim.seed ^ self.n;
        let me = sim.player.pos;
        let eye = me + Vec3::Y * sim.player.dims.eye;
        out.push(Cmd::Listener { pos: eye, yaw: sim.player.yaw });
        // Who moves where (calls follow their makers).
        let mut em = Vec::new();
        for n in sim.cast.npcs.iter().filter(|n| n.here()) {
            if (n.a.pos - me).length() < HEAR_RANGE {
                em.push((actor_key(ActorId::Npc(n.def.id)), n.a.pos + Vec3::Y * n.a.dims.height * 0.75));
            }
        }
        out.push(Cmd::Emitters(em));
        // The sim's cues.
        for (i, c) in std::mem::take(&mut sim.sounds).into_iter().enumerate() {
            if (c.at - eye).length() > HEAR_RANGE {
                continue;
            }
            if let Some(p) = play_of(c, mix_seed(seed, i as u64)) {
                out.push(Cmd::Play(Box::new(p)));
            }
        }
        self.walkers(sim, dt, seed, out);
        self.since_fire += dt;
        if self.since_fire >= 0.25 {
            self.since_fire = 0.0;
            self.fires(sim, out);
        }
        self.since_amb += dt;
        if self.since_amb >= 0.5 {
            self.since_amb = 0.0;
            let a = self.ambience(sim);
            out.push(Cmd::Ambience(Box::new(a)));
        }
    }

    /// Footsteps and brushing through growth, for everyone walking near.
    fn walkers(&mut self, sim: &mut Sim, dt: f32, seed: u64, out: &mut Vec<Cmd>) {
        let me = sim.player.pos;
        let mut walkers: Vec<(ActorId, Vec3, crate::world::species::Dims, bool)> = Vec::new();
        let p = &sim.player;
        walkers.push((ActorId::Player, p.pos, p.dims, !p.carried() && p.alt < 0.3));
        for n in sim.cast.npcs.iter().filter(|n| n.here() && (n.a.pos - me).length() < STEP_RANGE) {
            let grounded = n.a.alt < 0.3 && !(n.a.dims.flies && n.a.alt > 0.0) && !n.a.carried();
            walkers.push((ActorId::Npc(n.def.id), n.a.pos, n.a.dims, grounded));
        }
        let mut brushing = HashSet::new();
        let snap = sim.snap.clone();
        for (who, pos, dims, grounded) in walkers {
            let code = who.code();
            let (last, acc, last_step) = self.walk.get(&code).copied().unwrap_or((pos, 0.0, 0.0));
            let moved = Vec3::new(pos.x - last.x, 0.0, pos.z - last.z).length();
            // A jump (teleport, load): not a step.
            let moved = if moved > 3.0 { 0.0 } else { moved };
            let speed = moved / dt.max(1e-3);
            let stride = std::f32::consts::PI / dims.stride.max(0.3);
            let mut acc = acc + moved;
            let at = if who == ActorId::Player { At::Listener } else { At::Point(pos) };
            let mass = dims.mass.max(1.0);
            let depth = WATER_LEVEL - snap.terrain.height(pos.x, pos.z);
            // Small fast feet patter: one sound for every few steps (a voice each is a waste).
            let mut last_step = last_step;
            if grounded && acc >= stride && self.clock - last_step >= 0.14 {
                acc = 0.0;
                last_step = self.clock;
                let ground = if depth > 0.05 { Material::WATER } else { Material::of_ground(&snap.terrain.biomes.get(snap.terrain.biome_at(pos.x, pos.z)).map(|b| b.name.clone()).unwrap_or_default()) };
                let heavy = (mass / 70.0).powf(0.35);
                let gain = if who == ActorId::Player { STEPS * 0.6 } else { STEPS };
                let s = mix_seed(seed, code as u64 ^ 0x57E9);
                // The footfall: a soft thud, its pitch from the body.
                let thud = Part { kind: Kind::Knock, len: 0.05, hard: (ground.hard * 0.6).min(0.5), dry: ground.dry, ring: 0.0, loud: (0.05 + 0.12 * heavy).min(0.6), ..Part::default() };
                // And what the ground does under it (crunch, squelch, splash).
                let texture = Part { kind: Kind::Noise, len: if depth > 0.05 { 0.3 } else { 0.11 }, hard: ground.hard, dry: ground.dry, swell: 0.05, loud: (0.04 + 0.1 * heavy + if depth > 0.05 { 0.25 } else { 0.0 }).min(0.7), ..Part::default() };
                let body = Body { mass, pitch: 1.0 };
                out.push(Cmd::Play(Box::new(Play { at, call: Call(vec![thud]), body, seed: s, gain, delay: 0.0 })));
                out.push(Cmd::Play(Box::new(Play { at, call: Call(vec![texture]), body, seed: s ^ 1, gain: gain * 0.8, delay: 0.0 })));
            }
            self.walk.insert(code, (pos, acc.min(stride * 2.0), last_step));
            // Brushing: how much growth is about the body, and how fast it is
            // pushed. Growth light enough to give way rustles (grass walked
            // through, a shrub walked past); a trunk doesn't.
            if speed > 0.2 && depth < 0.6 {
                let r = dims.radius + 0.45;
                // What grows underfoot (walked through), and what stands
                // about (scatter, placed or live: walked past).
                let soft: Vec<crate::render::GpuInst> = sim.cache.items_near(&snap, pos, r + 1.0).into_iter().filter(|it| !it.solid).map(|it| it.inst).collect();
                let solid: Vec<crate::render::GpuInst> = sim.solids_near(pos, r + 1.5).into_iter().map(|s| s.inst).collect();
                let mut amount = 0.0;
                let mut mat = None;
                for (inst, is_solid) in soft.iter().map(|i| (i, false)).chain(solid.iter().map(|i| (i, true))) {
                    let c = if is_solid { inst.center() } else { inst.pos() };
                    let d = Vec3::new(c.x - pos.x, 0.0, c.z - pos.z).length();
                    let reach = if is_solid { r + inst.radius() * 0.75 } else { r + inst.radius() * 0.5 };
                    if d > reach {
                        continue;
                    }
                    let Some(m) = self.material(sim, inst.info[0]) else { continue };
                    if m.leafy < 0.5 {
                        continue;
                    }
                    if is_solid {
                        let scale = inst.pos_scale[3].max(0.01);
                        if self.mass(sim, inst.info[0]) * scale.powi(3) > mass * 10.0 {
                            continue;
                        }
                        amount += 0.6 * (1.0 - (d - r) / (reach - r).max(0.1)).clamp(0.3, 1.0);
                    } else {
                        amount += 0.45 * inst.pos_scale[3].clamp(0.3, 2.0);
                    }
                    mat.get_or_insert(m);
                }
                if let Some(m) = mat {
                    let key = (2 << 48) | code as u64;
                    let body = (dims.height / 1.75).clamp(0.3, 3.0);
                    let drive = (amount.min(1.3) * (speed / 3.5).min(1.4) * body.powf(0.5)).min(1.5);
                    let at = if who == ActorId::Player { At::Listener } else { At::Follow(actor_key(who), pos) };
                    out.push(Cmd::Texture { key, at, mat: m, size: 0.5, drive, roar: 0.0, gain: UNDERFOOT });
                    brushing.insert(key);
                }
            }
        }
        // Those who stopped brushing: let their rustle die away.
        for key in self.brushing.difference(&brushing) {
            out.push(Cmd::Texture { key: *key, at: At::Point(me), mat: Material::default(), size: 0.5, drive: 0.0, roar: 0.0, gain: UNDERFOOT });
        }
        self.brushing = brushing;
        self.walk.retain(|k, _| *k == 0 || sim.cast.get(*k).is_some_and(|n| n.here()));
    }

    /// Whatever burns near the traveler crackles: burning things and burning
    /// ground, gathered into a few fires (one voice each), each as loud as
    /// how much is burning there.
    fn fires(&mut self, sim: &mut Sim, out: &mut Vec<Cmd>) {
        use crate::sim::props::P_FIRE;
        const R: f32 = 70.0;
        const SPOT: f32 = 8.0;
        let me = sim.player.pos;
        // (fire amount, weighted position sum) per 8 m spot.
        let mut spots: HashMap<(i32, i32), (f32, Vec3)> = HashMap::new();
        let mut add = |p: Vec3, amount: f32| {
            let k = ((p.x / SPOT).floor() as i32, (p.z / SPOT).floor() as i32);
            let e = spots.entry(k).or_insert((0.0, Vec3::ZERO));
            e.0 += amount;
            e.1 += p * amount;
        };
        for id in sim.things.near(me, R) {
            if let Some(t) = sim.things.get(id) {
                let f = t.props.get(P_FIRE).copied().unwrap_or(0.0);
                if f > 0.05 {
                    add(t.pos, f * t.scale.clamp(0.3, 4.0));
                }
            }
        }
        for c in sim.field.cells.values() {
            let f = c.props.get(P_FIRE).copied().unwrap_or(0.0);
            if f > 0.05 && (c.pos - me).length() < R {
                add(c.pos, f * c.size.clamp(0.2, 4.0));
            }
        }
        let mut list: Vec<((i32, i32), f32, Vec3)> = spots.into_iter().map(|(k, (a, s))| (k, a, s / a.max(1e-3))).collect();
        list.sort_by(|a, b| (b.1 / (1.0 + (b.2 - me).length() / 10.0)).total_cmp(&(a.1 / (1.0 + (a.2 - me).length() / 10.0))));
        let mut now = HashSet::new();
        for (k, amount, at) in list.into_iter().take(4) {
            let key = (3 << 48) | (((k.0 as u32 as u64) << 20) ^ (k.1 as u32 as u64));
            let drive = (amount.sqrt() * 0.55).min(1.3);
            out.push(Cmd::Texture { key, at: At::Point(at + Vec3::Y * 0.8), mat: Material::FIRE, size: 0.3, drive, roar: 0.8, gain: 1.0 });
            now.insert(key);
        }
        for key in self.fires.difference(&now) {
            out.push(Cmd::Texture { key: *key, at: At::Point(me), mat: Material::FIRE, size: 0.3, drive: 0.0, roar: 0.0, gain: 1.0 });
        }
        self.fires = now;
    }

    /// Wind, water, dread and who sings, for where the traveler is now.
    fn ambience(&mut self, sim: &mut Sim) -> Ambience {
        let me = sim.player.pos;
        let snap = sim.snap.clone();
        let t = &snap.terrain;
        // Water: the nearest open water round about, and how much of it.
        let mut water: Option<(Vec3, f32)> = None;
        let mut wet = 0;
        for ring in [3.0f32, 7.0, 12.0, 20.0, 30.0, 42.0] {
            for k in 0..24 {
                let a = k as f32 / 24.0 * std::f32::consts::TAU;
                let (x, z) = (me.x + a.sin() * ring, me.z + a.cos() * ring);
                if t.height(x, z) < WATER_LEVEL - 0.15 {
                    wet += 1;
                    if water.is_none() {
                        water = Some((Vec3::new(x, WATER_LEVEL, z), 0.0));
                    }
                }
            }
        }
        if let Some((p, _)) = water {
            let d = (p - me).length();
            water = Some((p, ((1.0 - d / 50.0).max(0.0) * (0.4 + wet as f32 / 40.0)).min(1.0)));
        }
        // Wind: mostly on high, open ground; faint in hollows and woods.
        let wind = sim.exposure(me).powf(1.5);
        let growth = sim.cache.items_near(&snap, me, 22.0);
        // Dread: the nearest thing that harms, while it's dark.
        let dark = crate::sim::night::is_dark(sim.t);
        let mut dread: f32 = 0.0;
        let mut hush = Vec::new();
        for n in sim.cast.npcs.iter().filter(|n| n.here() && n.species.touch.harms()) {
            let d = (n.a.pos - me).length();
            // Small life falls quiet around it, wide when it's big.
            hush.push((n.a.pos, 22.0 * n.species.size.max(0.5).sqrt()));
            if dark && d < 60.0 {
                dread = dread.max((1.0 - d / 60.0).powf(1.5));
            }
        }
        // Who sings: insects at night where things grow; small creatures by day.
        let mut leafy = 0;
        for it in growth.iter().filter(|i| !i.solid).take(64) {
            if self.material(sim, it.inst.info[0]).is_some_and(|m| m.leafy > 0.5) {
                leafy += 1;
            }
        }
        let wooded = growth.iter().filter(|i| i.solid).take(64).filter(|it| self.material(sim, it.inst.info[0]).is_some_and(|m| m.leafy > 0.5)).count();
        let biome = t.biomes.get(t.biome_at(me.x, me.z)).map(|b| b.name.to_lowercase()).unwrap_or_default();
        let cold = ["snow", "ice", "frost", "glacier", "tundra", "frozen"].iter().any(|w| biome.contains(w));
        let night = crate::render::sky::is_night(sim.t) || dark;
        let mut songs = Vec::new();
        let mut singers = 0;
        if night && leafy >= 3 && !cold {
            songs = night_songs(sim.seed);
            singers = (6 + leafy / 2).min(22);
        } else if !night {
            songs = day_songs(sim, wooded);
            singers = if songs.is_empty() { 0 } else { (2 + wooded / 3).min(8) };
        }
        Ambience { wind, water, dread, songs, singers, hush }
    }
}

/// The world's insects: a few kinds, pitched by the world's own seed.
fn night_songs(seed: u64) -> Vec<Song> {
    let mut r = super::dsp::Rng::new(seed ^ 0xC41C);
    let mut v = Vec::new();
    for _ in 0..2 {
        let hz = r.range(3900.0, 5200.0);
        let pulses = 3 + (r.next() % 2) as u8;
        v.push(Song { part: Part { kind: Kind::Whistle, hz: [hz, hz * 0.99, hz * 0.975], len: r.range(0.013, 0.02), times: pulses, gap: r.range(0.012, 0.018), breath: 0.02, swell: 0.05, loud: 0.25, wobble: 0.0, rough: 0.0, ..Part::default() }, body: Body::default(), gain: 0.6, every: r.range(0.45, 0.9) });
    }
    // A trilling kind: long runs of pulses, then rest.
    let hz = r.range(3000.0, 4200.0);
    v.push(Song { part: Part { kind: Kind::Whistle, hz: [hz; 3], len: 0.011, times: 40, gap: 0.011, breath: 0.03, swell: 0.0, loud: 0.15, ..Part::default() }, body: Body::default(), gain: 0.45, every: 1.4 });
    v
}

/// By day: the world's own small creatures that have voices; failing
/// those, birds where trees stand.
fn day_songs(sim: &Sim, wooded: usize) -> Vec<Song> {
    let hour = sim.hour();
    let mut v = Vec::new();
    for sp in sim.snap.species.list.iter() {
        if sp.mass > 3.0 || sp.sounds.is_empty() || sp.touch.harms() || sp.speech == crate::world::species::Speech::None {
            continue;
        }
        let about = match sp.active {
            crate::world::species::Active::Night => hour < 6.0 || hour > 20.0,
            crate::world::species::Active::Day => (6.0..20.0).contains(&hour),
            crate::world::species::Active::Always => true,
        };
        if !about {
            continue;
        }
        for i in 0..sp.sounds.len().min(3) {
            if let Some(c) = super::call::species_call(sp, i) {
                for part in c.0.into_iter().take(2) {
                    v.push(Song { part, body: Body { mass: sp.mass, pitch: 1.0 }, gain: 0.35, every: 6.0 });
                }
            }
        }
    }
    if v.is_empty() && wooded >= 2 {
        let mut r = super::dsp::Rng::new(sim.seed ^ 0xB12D);
        for k in 0..4 {
            let hz = r.range(2400.0, 4200.0);
            let part = match k {
                0 => Part { kind: Kind::Whistle, hz: [hz, hz * 1.25, hz * 0.95], len: 0.11, times: 3, gap: 0.09, wobble: 0.05, swell: 0.1, loud: 0.45, breath: 0.03, ..Part::default() },
                1 => Part { kind: Kind::Whistle, hz: [hz * 1.5, hz * 1.1, hz * 0.95], len: 0.035, times: 16, gap: 0.022, swell: 0.05, loud: 0.35, breath: 0.02, ..Part::default() },
                2 => Part { kind: Kind::Whistle, hz: [hz * 1.05, hz, hz * 0.97], len: 0.3, times: 2, gap: 0.08, swell: 0.15, loud: 0.4, breath: 0.03, ..Part::default() },
                _ => Part { kind: Kind::Whistle, hz: [hz * 0.8, hz * 1.2, hz], len: 0.09, times: 5, gap: 0.07, rough: 0.25, wobble: 0.2, swell: 0.1, loud: 0.35, breath: 0.02, ..Part::default() },
            };
            v.push(Song { part, body: Body::default(), gain: 0.3, every: r.range(3.0, 7.0) });
        }
    }
    v
}

/// A cue as voices.
pub fn play_of(c: Cue, seed: u64) -> Option<Play> {
    let at = match c.from {
        Some(ActorId::Player) => At::Listener,
        Some(a) => At::Follow(actor_key(a), c.at),
        None => At::Point(c.at),
    };
    match c.what {
        Heard::Call { call, mass, pitch, gain } => Some(Play { at, call, body: Body { mass, pitch }, seed, gain, delay: 0.0 }),
        Heard::Hit { mat, mass, speed, by } => {
            // Two bodies meet. The softer one damps the strike (flesh on
            // stone is a dull thud), the lighter one moves and is mostly
            // what is heard, and heavy things barely sing.
            let (other, other_mass) = by.unwrap_or((mat, mass));
            let hard = mat.hard.min(other.hard);
            let light = mass.min(other_mass).max(0.01);
            let ring = mat.ring * (150.0 / mass.max(150.0)).sqrt();
            let energy = 0.5 * light * speed * speed;
            let loud = ((energy.max(1e-3).ln() + 2.0) / 12.0).clamp(0.0, 1.0) * (0.25 + 0.75 * hard);
            if loud < 0.03 {
                return None;
            }
            let mut parts = vec![Part { kind: Kind::Knock, len: 0.06, hard, dry: mat.dry, ring, loud, ..Part::default() }];
            if mat.leafy > 0.4 {
                // Into growth: its twigs snap (their own crispness), then the leaves.
                parts[0].times = 2 + (seed % 3) as u8;
                parts[0].gap = 0.05;
                parts[0].hard = mat.hard.max(0.6);
                parts[0].loud *= 0.5;
                parts.push(Part { kind: Kind::Noise, len: 0.4, hard: mat.hard, dry: mat.dry, swell: 0.1, loud: loud * 0.6, ..Part::default() });
            }
            Some(Play { at, call: Call(parts), body: Body { mass: light, pitch: 1.0 }, seed, gain: 1.0, delay: 0.0 })
        }
    }
}
