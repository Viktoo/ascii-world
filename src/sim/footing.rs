//! Footing: every body, the traveler's and every character's, stands on
//! whatever is under it: the ground, a floor, a stair, a roof, a rock. It
//! walks up what is lower than its step, falls off what is higher, jumps,
//! and crouches to get under what is low. One controller for all bodies,
//! sized by each body, so a cat and a giant meet the same world differently.

use super::{ActorId, Sim};
use crate::world::collide::{Capsule, MAX_WADE, step_for, support_under};
use glam::Vec3;

pub const GRAVITY: f32 = super::physics::GRAVITY;
/// A jump lifts a body this much of its own height.
const JUMP: f32 = 0.5;
/// A drop up to this many of one's own heights doesn't hurt.
const FALL_SAFE_HEIGHTS: f32 = 1.5;
/// Health a fall takes for each metre beyond that.
const FALL_HURT_PER_M: f32 = 0.12;
/// A swimmer afloat keeps its eyes this far over the water.
const SWIM_EYE_OVER: f32 = 0.12;
/// How fast a swimmer goes up, and down (m/s): diving is slower.
const SWIM_UP: f32 = 1.4;
const SWIM_DOWN: f32 = 0.8;
/// How fast a crouch goes down or comes up (per second, of the whole way).
const CROUCH_RATE: f32 = 5.0;

/// How wide a body walks: people keep the walking radius they always had;
/// other bodies their own.
pub fn walk_radius(id: ActorId, a: &super::actor::Actor) -> f32 {
    match id {
        ActorId::Player | ActorId::Traveler(_) => crate::world::collide::PLAYER_RADIUS,
        ActorId::Npc(_) if a.dims.arms => crate::world::collide::NPC_RADIUS,
        ActorId::Npc(_) => a.dims.radius.clamp(0.12, 4.0),
    }
}

impl Sim {
    /// The capsule a body walks in now (crouching makes it shorter).
    pub fn capsule(&self, id: ActorId) -> Capsule {
        let Some(a) = self.actor(id) else { return Capsule::of(1.75, crate::world::collide::NPC_RADIUS) };
        let (swims, needs_water) = match id {
            ActorId::Player | ActorId::Traveler(_) => (true, 0.0),
            ActorId::Npc(c) => match self.cast.get(c) {
                // A creature of the water keeps to water deep enough for it,
                // and one that dives deep to water half as deep as it dives
                // (a dolphin stays off the shallows and out of harbours).
                Some(n) if n.species.water_only() => (true, (a.dims.height * 0.5).max(n.species.depth[1] * 0.5).clamp(0.2, 4.0)),
                // Any body that swims goes in, unless it shuns being wet.
                Some(n) => (n.species.moves.swim > 0.0 && !n.species.shuns.iter().any(|p| p == "wet"), 0.0),
                None => (false, 0.0),
            },
        };
        Capsule { radius: walk_radius(id, a), height: a.stand_height(), step: step_for(a.dims.height), swims, needs_water }
    }

    /// Whether a body could be at `p`: a walker on dry land (at least
    /// `margin` over the water), a creature of the water in water it fits
    /// in, a swimmer either.
    pub fn can_be_at(&self, id: ActorId, p: Vec3, margin: f32) -> bool {
        let cap = self.capsule(id);
        let t = &self.snap.terrain;
        let wd = t.water(p.x, p.z) - t.height(p.x, p.z);
        if cap.needs_water > 0.0 { wd >= cap.needs_water } else { cap.swims || wd < -margin }
    }

    /// How far under the surface a swimmer holds its eyes now: the traveler
    /// as they swim (0); a creature somewhere in its species' depth, drifting
    /// slowly up and down through it.
    fn dive_of(&self, id: ActorId) -> f32 {
        let ActorId::Npc(c) = id else { return 0.0 };
        let Some(n) = self.cast.get(c) else { return 0.0 };
        let [lo, hi] = n.species.depth;
        let k = 0.5 + 0.5 * (self.t as f32 * 0.03 + c as f32 * 1.7).sin();
        lo + (hi - lo) * k
    }

    /// Settle a body on its footing for `dt`: stand on what is under it,
    /// step down stairs, fall from ledges, land; crouch or stand up.
    pub fn step_footing(&mut self, id: ActorId, dt: f32) {
        let Some(a) = self.actor(id) else { return };
        if a.carried() || a.alt > 0.0 || a.ladder.is_some() {
            return;
        }
        // Falling past a ladder: caught hold of.
        if self.catch_ladder(id) {
            return;
        }
        let Some(a) = self.actor(id) else { return };
        let (pos, mut vy, mut grounded) = (a.pos, a.vy, a.grounded);
        let full = a.dims.height;
        let (eye, swim, was_swimming, dive_to) = (a.dims.eye, a.swim, a.swimming, a.dive_to);
        let want = if a.crouching { 1.0 } else { 0.0 };
        let mut crouch = a.crouch;
        // Standing still on firm footing: nothing changes under it, unless
        // the ground itself did (a pit dug, a floor gone).
        if grounded && vy == 0.0 && a.moved < 1e-5 && crouch == want && id != ActorId::Player {
            let g = self.snap.terrain.floor(pos.x, pos.y + 0.3, pos.z);
            // (Under water, a swimmer rises from where it was set down.)
            if pos.y >= g - 0.01 && pos.y - g < 0.02 && self.snap.terrain.water(pos.x, pos.z) < pos.y + a.dims.eye {
                return;
            }
        }
        let cap = self.capsule(id);
        // What the last walk found around it, if it hasn't moved since.
        let support = a.support;
        // Set afresh by each step on the way (see `step_toward`).
        if dive_to.is_some() && let Some(a) = self.actor_mut(id) {
            a.dive_to = None;
        }
        let cached = support.filter(|(p, _)| (*p - pos).length_squared() < 1e-8 && crouch == want).map(|x| x.1);
        let solids = if cached.is_some() && grounded && vy == 0.0 { Vec::new() } else { self.solids_near(pos, cap.radius + full + 2.0) };
        // Crouch down at once; stand up only where there is room overhead.
        if crouch != want {
            let next = crouch + (want - crouch).clamp(-CROUCH_RATE * dt, CROUCH_RATE * dt);
            let blocked = next < crouch && {
                let head = pos.y + full * (1.0 - super::actor::Actor::CROUCH_DROP * next) - 0.05;
                solids.iter().any(|s| s.sdf(Vec3::new(pos.x, head, pos.z)) < cap.radius * 0.5)
            };
            if !blocked {
                crouch = next;
            }
        }
        let top = pos.y + if grounded && vy <= 0.0 { cap.step } else { 0.05 };
        // In the water, a swimmer finds footing a little over it (a step up
        // out of a harbour), as it pulls itself out onto a bank.
        let wl_here = self.snap.terrain.water(pos.x, pos.z);
        let mut out_top = None;
        if cap.swims && cap.needs_water <= 0.0 && !solids.is_empty() && pos.y < wl_here - 0.3 {
            out_top = Some(wl_here + 0.9);
        }
        let mut ground = match cached {
            Some(g) if grounded && vy == 0.0 => g,
            _ => support_under(&self.snap.terrain, &solids, pos, cap.radius, top),
        };
        // A being of the water is not lifted out onto a step or a ledge just
        // under the surface: it swims over it.
        if cap.needs_water > 0.0 && ground > wl_here - cap.needs_water && ground > self.snap.terrain.height(pos.x, pos.z) + 0.05 {
            ground = self.snap.terrain.height(pos.x, pos.z);
        }
        // Near the top only (a roof down on the bed is no step out).
        if let Some(t) = out_top {
            let up = support_under(&self.snap.terrain, &solids, pos, cap.radius, t);
            if up > wl_here - 1.0 {
                ground = ground.max(up);
            }
        }
        // Water too deep to stand in: a swimmer floats with its eyes just
        // over it, swims up or down, and comes up again when let be.
        if cap.swims {
            let t = &self.snap.terrain;
            let dry_cave = t.caves_near(pos.x, pos.z) && t.void(pos) < 0.0;
            let wl = if dry_cave { f32::MIN } else { t.water(pos.x, pos.z) };
            let float = wl + SWIM_EYE_OVER - eye;
            // A fish swims in any water it fits in; others where they can't wade.
            let deep = if cap.needs_water > 0.0 { wl - ground > cap.needs_water * 0.5 } else { wl - ground > MAX_WADE * (full / 1.75) };
            if deep && pos.y < float + 0.3 {
                // Let be, it rises to where it swims: the top, or its own
                // depth. Inside a house under the water a being keeps to its
                // floor (it doesn't float up through the roof), and on its
                // way in it makes for the doorway.
                let indoors = id != ActorId::Player && self.room_at(pos + Vec3::Y * 0.3).is_some();
                let hold = match dive_to {
                    Some(y) => y.min(float),
                    None if indoors => ground.min(float),
                    None => float - self.dive_of(id),
                };
                let fast = if dive_to.is_some() || indoors { SWIM_UP } else { 0.5 };
                let want = if swim > 0.0 { SWIM_UP } else if swim < 0.0 { -SWIM_DOWN } else { ((hold - pos.y) * 1.2).clamp(-SWIM_DOWN, fast) };
                vy += (want - vy) * (dt * 3.0).min(1.0);
                let mut feet = pos.y + vy * dt;
                if feet > float {
                    feet = float;
                    vy = vy.min(0.0);
                }
                // Diving, a swimmer lies flat: its eyes go down to just over the bed.
                let deepest = ground + SWIM_EYE_OVER * 3.0 - eye;
                if feet < deepest {
                    feet = deepest;
                    vy = vy.max(0.0);
                }
                if let Some(a) = self.actor_mut(id) {
                    a.pos.y = feet;
                    a.vy = vy;
                    a.grounded = false;
                    a.swimming = true;
                    a.crouching = false;
                    a.crouch = 0.0;
                    a.support = None;
                    a.perch = None;
                }
                if !was_swimming && vy < -2.0 {
                    // In with a splash.
                    let mass = self.actor(id).map(|a| a.dims.mass).unwrap_or(70.0);
                    self.cue(Vec3::new(pos.x, wl, pos.z), Some(id), crate::audio::Heard::Hit { mat: crate::audio::call::Material::WATER, mass: mass.min(400.0), speed: (-vy).min(12.0), by: None });
                }
                return;
            }
        }
        if was_swimming && let Some(a) = self.actor_mut(id) {
            a.swimming = false;
        }
        let mut feet = pos.y;
        let mut landed = 0.0f32;
        if grounded && vy <= 0.0 {
            if ground >= feet - cap.step * 1.2 {
                // Up a stair, down a stair, along a slope.
                feet = ground;
                vy = 0.0;
            } else {
                // Walked off an edge.
                grounded = false;
                vy = 0.0;
                self.note_perch(id);
            }
        }
        if !grounded {
            vy -= self.gravity() * dt;
            let mut next = feet + vy * dt;
            if vy > 0.0 {
                let head = next + cap.height;
                if solids.iter().any(|s| s.sdf(Vec3::new(pos.x, head, pos.z)) < 0.05) {
                    vy = 0.0;
                    next = feet;
                }
            }
            if next <= ground {
                landed = -vy;
                next = ground;
                vy = 0.0;
                grounded = true;
            }
            feet = next;
        }
        if let Some(a) = self.actor_mut(id) {
            a.pos.y = feet;
            a.vy = vy;
            a.grounded = grounded;
            a.crouch = crouch;
            a.support = None;
        }
        if landed > 0.0 {
            self.land_hard(id, landed);
        }
        if landed > 3.0 {
            let mass = self.actor(id).map(|a| a.dims.mass).unwrap_or(70.0);
            let by = Some((crate::audio::call::Material::FLESH, mass));
            self.cue(Vec3::new(pos.x, feet, pos.z), Some(id), crate::audio::Heard::Hit { mat: crate::audio::call::Material::SOIL, mass: mass.min(400.0), speed: landed.min(12.0), by });
        }
    }

    /// Landing from higher than 1.5 times one's own height hurts, more the further it
    /// was (a jump on flat ground never does; off a roof or out of a tree
    /// it does, and again and again it can kill).
    fn land_hard(&mut self, id: ActorId, speed: f32) {
        let Some(a) = self.actor_mut(id) else { return };
        let perch = a.perch.take();
        // What hurts is how fast they land: told as the drop it takes on Earth.
        let drop = speed * speed / (2.0 * GRAVITY);
        let over = drop - a.dims.height.max(1.0) * FALL_SAFE_HEIGHTS;
        if over <= 0.0 {
            return;
        }
        let (pos, fwd) = (a.pos, a.forward());
        let hurt = (over * FALL_HURT_PER_M).min(1.0);
        // From what, when it was off something (and fell most of the way from it).
        let from = perch.filter(|(y, _)| y - pos.y > drop * 0.4).map(|(_, n)| format!(" from {}", super::actions::the(&n))).unwrap_or_default();
        let how = format!("broken by a fall of {drop:.0} m{from}");
        let died = self.wound(id, hurt, id, &how);
        let name = self.actor_name(id);
        self.event("fell_hard", Some(id), None, format!("{name} fell {drop:.0} m and landed hard"), Some(pos), serde_json::json!({ "drop": (drop * 10.0).round() / 10.0, "hurt": (hurt * 100.0).round() / 100.0 }));
        if !died {
            self.jolt(id, fwd * 0.2 - Vec3::Y * 0.1, (0.15 + hurt).min(0.5));
        }
        match id {
            ActorId::Player if !died => self.notes.push(super::Note::Notable(if hurt > 0.3 { "You land badly. Something gives.".into() } else { "You land hard.".into() })),
            ActorId::Npc(c) => {
                self.note_near(pos, 30.0, super::Note::Ambient(format!("{} lands hard.", super::physics::cap(&name))));
                if died {
                    self.kill(c, &format!("{how}, and died"), None);
                }
            }
            _ => {}
        }
    }

    /// A creature of the water out of it (washed up, dug out, carried off)
    /// suffers, flops, and works its way to the deepest water near. True
    /// while it is stranded.
    pub fn stranded_tick(&mut self, cid: i64, dt: f32) -> bool {
        let id = ActorId::Npc(cid);
        let need = self.capsule(id).needs_water;
        let Some(a) = self.actor(id) else { return false };
        if need <= 0.0 {
            return false;
        }
        let (pos, carried, snap) = (a.pos, a.carried(), self.snap.clone());
        let depth = |x: f32, z: f32| snap.terrain.water(x, z) - snap.terrain.height(x, z);
        if depth(pos.x, pos.z) >= need * 0.5 {
            return false;
        }
        let died = self.wound(id, dt / 90.0, id, "stranded out of the water");
        if died {
            self.kill(cid, "stranded out of the water, and died", None);
            return true;
        }
        if carried {
            return true;
        }
        // The way the water lies deepest, near.
        let best = (0..8)
            .flat_map(|k| [3.0f32, 6.0].map(|r| (k, r)))
            .map(|(k, r)| {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                let d = Vec3::new(a.cos(), 0.0, a.sin());
                (d, depth(pos.x + d.x * r, pos.z + d.z * r))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        // A flop now and then.
        let flop = (self.t * 1.3 + cid as f64).fract() < (dt as f64 * 1.3);
        if let Some((d, _)) = best.filter(|b| b.1 > depth(pos.x, pos.z)) {
            if let Some(a) = self.actor_mut(id) {
                a.face(d, dt * 3.0);
            }
            self.walk(id, d * 0.5 * dt);
        }
        if flop {
            let side = self.actor(id).map(|a| a.right()).unwrap_or(Vec3::X);
            self.jolt(id, if cid % 2 == 0 { side } else { -side }, 0.3);
        }
        true
    }

    /// Leaving the ground: remember what it stood on, if anything (a roof,
    /// a tree), so a bad landing can say where it fell from.
    fn note_perch(&mut self, id: ActorId) {
        let Some(pos) = self.actor(id).map(|a| a.pos) else { return };
        let land = self.snap.terrain.height(pos.x, pos.z);
        let perch = if pos.y > land + 0.3 {
            let feet = pos - Vec3::Y * 0.05;
            self.solids_near(pos, 3.0).into_iter().map(|s| (s.sdf(feet), s.ty.name().to_string())).filter(|(d, _)| *d < 0.4).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, n)| (pos.y, n))
        } else {
            None
        };
        if let Some(a) = self.actor_mut(id) {
            a.perch = perch;
        }
    }

    /// Jump, if standing on something. True if it left the ground.
    pub fn jump(&mut self, id: ActorId) -> bool {
        let Some(a) = self.actor(id) else { return false };
        if !a.grounded || a.carried() || a.alt > 0.0 || a.asleep {
            return false;
        }
        let h = a.dims.height * JUMP * (1.0 - 0.5 * a.crouch);
        let v = (2.0 * GRAVITY * h).sqrt();
        self.note_perch(id);
        if let Some(a) = self.actor_mut(id) {
            a.vy = v;
            a.grounded = false;
        }
        true
    }

    /// Crouch (or stand, or toggle with None). Returns whether it now wants to crouch.
    pub fn crouch(&mut self, id: ActorId, down: Option<bool>) -> bool {
        let Some(a) = self.actor_mut(id) else { return false };
        a.crouching = down.unwrap_or(!a.crouching);
        a.crouching
    }

    /// How much a body's gait slows or speeds it now: running, crouching,
    /// favouring a hurt, knocked off its step.
    pub fn gait_factor(&self, id: ActorId) -> f32 {
        let Some(a) = self.actor(id) else { return 1.0 };
        let run = if a.running && a.crouch < 0.3 { 1.7 } else { 1.0 };
        let hurt = match id {
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.hurt()).unwrap_or(0.0),
            ActorId::Player | ActorId::Traveler(_) => 0.0,
        };
        // Hurt badly, a run is no more than a hobble.
        let hurt_k = 1.0 - 0.55 * hurt;
        let run = 1.0 + (run - 1.0) * (1.0 - hurt);
        let stagger = if a.jolt.is_some_and(|j| j.staggers(self.t)) { 0.25 } else { 1.0 };
        // Swimming is slower than walking, and no faster for running; a
        // creature that swims goes at its own swimming pace (`base_speeds`).
        let own = matches!(id, ActorId::Npc(c) if self.cast.get(c).is_some_and(|n| n.species.moves.swim > 0.0));
        let swim = if a.swimming && !own { 0.55 / run } else { 1.0 };
        run * (1.0 - 0.5 * a.crouch) * hurt_k * stagger * swim
    }

    /// A body is rocked along `dir` (level, world) by `size` radians (see
    /// `Jolt`): a blow, a stumble, something heavy hitting it.
    pub fn jolt(&mut self, id: ActorId, dir: Vec3, size: f32) {
        let t = self.t;
        let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        let Some(a) = self.actor_mut(id) else { return };
        if dir == Vec3::ZERO || size <= 0.0 {
            return;
        }
        // A fresh blow on one still reeling adds to it, up to a limit.
        let left = a.jolt.map(|j| j.angle(t)).unwrap_or(0.0);
        a.jolt = Some(super::actor::Jolt { t0: t, dir, size: (size + left * 0.5).min(0.5) });
    }
}
