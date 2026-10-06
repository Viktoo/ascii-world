//! Footing: every body, the traveler's and every character's, stands on
//! whatever is under it: the ground, a floor, a stair, a roof, a rock. It
//! walks up what is lower than its step, falls off what is higher, jumps,
//! and crouches to get under what is low. One controller for all bodies,
//! sized by each body, so a cat and a giant meet the same world differently.

use super::{ActorId, Sim};
use crate::world::collide::{Capsule, step_for, support_under};
use glam::Vec3;

pub const GRAVITY: f32 = super::physics::GRAVITY;
/// A jump lifts a body this much of its own height.
const JUMP: f32 = 0.5;
/// Health a fall takes for each metre beyond one's own height.
const FALL_HURT_PER_M: f32 = 0.12;
/// How fast a crouch goes down or comes up (per second, of the whole way).
const CROUCH_RATE: f32 = 5.0;

/// How wide a body walks: people keep the walking radius they always had;
/// other bodies their own.
pub fn walk_radius(id: ActorId, a: &super::actor::Actor) -> f32 {
    match id {
        ActorId::Player => crate::world::collide::PLAYER_RADIUS,
        ActorId::Npc(_) if a.dims.arms => crate::world::collide::NPC_RADIUS,
        ActorId::Npc(_) => a.dims.radius.clamp(0.12, 4.0),
    }
}

impl Sim {
    /// The capsule a body walks in now (crouching makes it shorter).
    pub fn capsule(&self, id: ActorId) -> Capsule {
        let Some(a) = self.actor(id) else { return Capsule::of(1.75, crate::world::collide::NPC_RADIUS) };
        Capsule { radius: walk_radius(id, a), height: a.stand_height(), step: step_for(a.dims.height) }
    }

    /// Settle a body on its footing for `dt`: stand on what is under it,
    /// step down stairs, fall from ledges, land; crouch or stand up.
    pub fn step_footing(&mut self, id: ActorId, dt: f32) {
        let Some(a) = self.actor(id) else { return };
        if a.carried() || a.alt > 0.0 {
            return;
        }
        let (pos, mut vy, mut grounded) = (a.pos, a.vy, a.grounded);
        let full = a.dims.height;
        let want = if a.crouching { 1.0 } else { 0.0 };
        let mut crouch = a.crouch;
        // Standing still on firm footing: nothing changes under it, unless
        // the ground itself did (a pit dug, a floor gone).
        if grounded && vy == 0.0 && a.moved < 1e-5 && crouch == want && id != ActorId::Player {
            let g = self.snap.terrain.height(pos.x, pos.z);
            if pos.y >= g - 0.01 && pos.y - g < 0.02 {
                return;
            }
        }
        let cap = self.capsule(id);
        // What the last walk found around it, if it hasn't moved since.
        let cached = a.support.filter(|(p, _)| (*p - pos).length_squared() < 1e-8 && crouch == want).map(|x| x.1);
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
        let ground = match cached {
            Some(g) if grounded && vy == 0.0 => g,
            _ => support_under(&self.snap.terrain, &solids, pos, cap.radius, top),
        };
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
            vy -= GRAVITY * dt;
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

    /// Landing from higher than one's own height hurts, more the further it
    /// was (a jump on flat ground never does; off a roof or out of a tree
    /// it does, and again and again it can kill).
    fn land_hard(&mut self, id: ActorId, speed: f32) {
        let Some(a) = self.actor_mut(id) else { return };
        let perch = a.perch.take();
        let drop = speed * speed / (2.0 * GRAVITY);
        let over = drop - a.dims.height.max(1.0);
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
            ActorId::Player => 0.0,
        };
        // Hurt badly, a run is no more than a hobble.
        let hurt_k = 1.0 - 0.55 * hurt;
        let run = 1.0 + (run - 1.0) * (1.0 - hurt);
        let stagger = if a.jolt.is_some_and(|j| j.staggers(self.t)) { 0.25 } else { 1.0 };
        run * (1.0 - 0.5 * a.crouch) * hurt_k * stagger
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
