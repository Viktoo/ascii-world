//! Vehicles: things that carry a rider who steers them (`meta.drive`): a
//! cart or a car on land, a boat on water, a flying carpet in the air. The
//! rider sits at the shape's first seat anchor; the walk keys are throttle
//! and wheel. The engine knows no cars: a top speed, what it goes on, a seat.

use super::actions::{ActErr, Outcome, Resolved};
use super::render::thing_inst;
use super::things::ThingId;
use super::{ActorId, Note, Sim, Target};
use crate::lang::ir::Drive;
use crate::terrain::WATER_LEVEL;
use crate::world::collide::{Capsule, MAX_WADE, Obstacles, free_spot, support_under};
use crate::world::{Solid, TypeEntry};
use glam::{Quat, Vec2, Vec3};
use serde_json::json;
use std::sync::Arc;

/// How fast a vehicle turns at its best (rad/s).
const TURN: f32 = 1.3;
/// How fast a flyer climbs or dives (m/s).
const CLIMB: f32 = 4.0;
/// A boat out of the water (beached, launching) only crawls (m/s).
const AGROUND: f32 = 1.0;

fn fail<T>(s: impl Into<String>) -> Result<T, ActErr> {
    Err(ActErr::Fail(s.into()))
}

/// Where the rider sits, in the shape's own frame: its first seat anchor,
/// else the middle of its top.
fn seat_local(ty: &TypeEntry) -> (Vec3, f32) {
    match ty.ct.meta.anchors.iter().find(|a| a.kind == "seat") {
        Some(a) => (Vec3::from_array(a.at), a.face.to_radians()),
        None => (Vec3::new(0.0, ty.top, 0.0), 0.0),
    }
}

impl Sim {
    /// How a target moves when steered, if it is a vehicle.
    pub fn target_drive(&self, t: &Target) -> Option<Drive> {
        let (_, ty, _) = self.target_shape(t)?;
        ty.ct.meta.drive.clone()
    }

    /// Who is aboard a vehicle.
    pub fn aboard_of(&self, id: ThingId) -> Option<ActorId> {
        self.actor_ids().into_iter().find(|a| self.actor(*a).is_some_and(|x| x.aboard == Some(id)))
    }

    /// How fast the vehicle `who` is aboard is going (m/s), if any.
    pub fn vehicle_speed(&self, who: ActorId) -> Option<f32> {
        let id = self.actor(who)?.aboard?;
        Some(self.drives.get(&id).copied().unwrap_or(0.0))
    }

    /// Get into (or onto) a vehicle within reach.
    pub fn board(&mut self, who: ActorId, r: &Resolved) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let Some(drive) = self.target_drive(&r.target) else { return fail(format!("{name} can't ride the {}", r.name)) };
        if who != ActorId::Player {
            return fail(format!("{name} doesn't know how to drive the {}", r.name));
        }
        let me = self.actor(who).cloned().ok_or(ActErr::Fail("no such actor".into()))?;
        if me.seated() {
            return fail(format!("{name} is already riding"));
        }
        self.reach(who, r)?;
        let id = self.liven(&r.target).ok_or(ActErr::Fail(format!("{name} can't get into the {}", r.name)))?;
        if self.aboard_of(id).is_some() {
            return fail(format!("someone is already in the {}", r.name));
        }
        let Some((pos, yaw, held)) = self.things.get(id).map(|t| (t.pos, t.yaw, t.held())) else { return fail("it's gone") };
        if held {
            return fail(format!("the {} is being carried", r.name));
        }
        if let Some(t) = self.things.get_mut(id) {
            t.anchored = false;
            t.asleep = true;
            t.vel = Vec3::ZERO;
            t.dirty = true;
        }
        self.drives.insert(id, 0.0);
        let face = self.things.get(id).and_then(|t| self.snap.type_of(t.type_id)).map(|ty| seat_local(ty).1).unwrap_or(0.0);
        if let Some(a) = self.actor_mut(who) {
            a.aboard = Some(id);
            a.task = None;
            a.crouching = false;
            a.crouch = 0.0;
            a.vy = 0.0;
            a.yaw = yaw + face;
        }
        self.update_aboard();
        let tname = self.thing_name(id);
        let how = match drive.on.as_str() {
            "water" => "W/S row or sail, A/D steer",
            "air" => "W/S speed, A/D steer, Space up, c down (or look up or down while flying)",
            _ => "W/S speed and brake, A/D steer",
        };
        let msg = format!("{name} climbs into the {tname}");
        self.event("boarded", Some(who), Some(format!("thing:{id}")), msg.clone(), Some(pos), json!({ "on": drive.on }));
        self.note_near(pos, 25.0, Note::seen(format!("{}.", super::physics::cap(&msg)), who == ActorId::Player));
        Ok(Outcome::ok(format!("{msg} ({how}, e to get out)")))
    }

    /// Get out of the vehicle, beside it.
    pub fn leave_vehicle(&mut self, who: ActorId) -> Result<Outcome, ActErr> {
        let Some(id) = self.actor(who).and_then(|a| a.aboard) else { return fail("not in anything") };
        let name = self.actor_name(who);
        let tname = self.thing_name(id);
        let spot = match self.things.get(id).cloned().zip(self.things.get(id).and_then(|t| self.snap.type_of(t.type_id)).cloned()) {
            Some((t, ty)) => {
                let b = ty.ct.meta.bounds;
                let (seat, _) = seat_local(&ty);
                let side = if seat.x >= 0.0 { 1.0 } else { -1.0 };
                let gi = thing_inst(&t, &ty, [0.0; 4]);
                let p = gi.from_local(Vec3::new(side * (b[0] + 0.6 / t.scale.max(0.1)), 0.0, seat.z));
                let solids = self.solids_near(p, 6.0);
                let obs = Obstacles { solids: &solids, bodies: &[] };
                let q = free_spot(&self.snap.terrain, &obs, Vec3::new(p.x, self.snap.terrain.height(p.x, p.z), p.z), crate::world::collide::PLAYER_RADIUS, 4.0);
                Vec3::new(q.x, self.snap.terrain.height(q.x, q.z), q.z)
            }
            None => self.actor(who).map(|a| a.pos).unwrap_or_default(),
        };
        if self.snap.terrain.height(spot.x, spot.z) < WATER_LEVEL - MAX_WADE {
            return fail(format!("the water is too deep to get out of the {tname} here"));
        }
        if let Some(a) = self.actor_mut(who) {
            a.aboard = None;
            a.pos = spot;
            a.vy = 0.0;
            a.grounded = true;
            a.alt = 0.0;
            a.support = None;
        }
        self.drives.remove(&id);
        if let Some(t) = self.things.get_mut(id) {
            t.vel = Vec3::ZERO;
            t.dirty = true;
        }
        let msg = format!("{name} gets out of the {tname}");
        self.event("left_vehicle", Some(who), Some(format!("thing:{id}")), msg.clone(), Some(spot), json!({}));
        Ok(Outcome::ok(msg))
    }

    /// Riders sit in their vehicles' seats.
    pub(super) fn update_aboard(&mut self) {
        let riders: Vec<(ActorId, ThingId)> = self.actor_ids().into_iter().filter_map(|a| self.actor(a).and_then(|x| x.aboard).map(|v| (a, v))).collect();
        for (r, v) in riders {
            let seat = self.things.get(v).and_then(|t| {
                let ty = self.snap.type_of(t.type_id)?;
                ty.ct.meta.drive.as_ref()?;
                let (at, face) = seat_local(ty);
                Some((thing_inst(t, ty, [0.0; 4]).from_local(at), t.yaw + face))
            });
            let Some((seat, yaw)) = seat else {
                if let Some(a) = self.actor_mut(r) {
                    a.aboard = None;
                }
                self.drives.remove(&v);
                continue;
            };
            if let Some(a) = self.actor_mut(r) {
                let hip = 0.67 * a.dims.height / 1.75;
                a.pos = seat - Vec3::Y * hip;
                a.task = None;
                if r != ActorId::Player {
                    a.yaw = yaw;
                }
            }
        }
    }

    /// The vehicle's own solid, as others meet it.
    fn vehicle_solid(&self, id: ThingId) -> Option<Solid> {
        let t = self.things.get(id)?;
        let ty = self.snap.type_of(t.type_id)?.clone();
        Some(Solid { inst: thing_inst(t, &ty, [0.0; 4]), ty })
    }

    /// How far to lift the traveler's eye so it isn't inside the vehicle
    /// they sit in (a closed cab): just over the part above the seat.
    pub fn eye_lift(&self, eye: Vec3) -> f32 {
        let Some(s) = self.player.aboard.and_then(|id| self.vehicle_solid(id)) else { return 0.0 };
        let top = s.inst.center().y + s.inst.radius();
        let mut lift = 0.0;
        while s.sdf(eye + Vec3::Y * lift) < 0.12 && eye.y + lift < top + 0.3 {
            lift += 0.12;
        }
        lift
    }

    /// Steer the vehicle `who` is in: `throttle` forward (−1 back … 1),
    /// `steer` right (−1 left … 1), and for a flyer `climb` (−1 down … 1
    /// up; it hovers at 0). Land vehicles keep to the ground and can't turn standing
    /// still; boats keep to the water; flyers climb over what is ahead.
    pub fn steer(&mut self, who: ActorId, throttle: f32, steer: f32, climb: f32, dt: f32) {
        let Some(id) = self.actor(who).and_then(|a| a.aboard) else { return };
        let Some(t) = self.things.get(id).cloned() else { return };
        let Some(ty) = self.snap.type_of(t.type_id).cloned() else { return };
        let Some(drive) = ty.ct.meta.drive.clone() else { return };
        let (water, air) = (drive.on == "water", drive.on == "air");
        let max = drive.speed.max(0.5);
        let b = ty.ct.meta.bounds;
        let sc = t.scale.max(0.05);
        let radius = (b[0].min(b[2]) * sc).clamp(0.2, 3.0);
        let height = ((ty.top - ty.bottom) * sc).max(0.3);
        let cap = Capsule { radius, height, step: (height * 0.25).clamp(0.15, 0.6) };
        let feet = t.pos.y + ty.bottom * sc;
        let ground_here = self.snap.terrain.height(t.pos.x, t.pos.z);
        let alt = (feet - ground_here.max(if water { WATER_LEVEL } else { f32::MIN })).max(0.0);
        let airborne = air && alt > 0.5;

        // Speed: throttle pushes, the brake is the throttle against it, and
        // it coasts down when let be.
        let mut v = self.drives.get(&id).copied().unwrap_or(0.0);
        let accel = (max * 0.5).clamp(1.5, 8.0);
        if throttle.abs() > 0.01 {
            let k = if throttle * v >= 0.0 { 1.0 } else { 2.5 };
            v += throttle * accel * k * dt;
        } else {
            v -= v.signum() * v.abs().min((1.0 + 0.25 * v.abs()) * dt);
        }
        v = v.clamp(-max * 0.35, max);
        if water && ground_here > WATER_LEVEL - 0.3 {
            v = v.clamp(-AGROUND, AGROUND);
        }

        // Turning: wheels and keels turn only when moving; flyers in place.
        let grip = if air { 1.0 } else { (v.abs() / 2.0).min(1.0) * v.signum() };
        let turn = steer * TURN * grip * (1.0 - 0.4 * (v.abs() / max).min(1.0));
        let yaw = t.yaw + turn * dt;
        let fwd = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let delta = fwd * v * dt;

        // Moving: blocked by what stands in the way, at its height.
        let from = Vec3::new(t.pos.x, feet, t.pos.z);
        let own = thing_inst(&t, &ty, [0.0; 4]).center();
        let solids: Vec<Solid> = self.solids_near(from, radius + delta.length() + 4.0).into_iter().filter(|s| !(Arc::ptr_eq(&s.ty, &ty) && (s.inst.center() - own).length() < 0.05)).collect();
        let bodies: Vec<(Vec3, f32)> = self
            .actor_ids()
            .into_iter()
            .filter(|a| *a != who)
            .filter_map(|a| self.actor(a).filter(|x| !x.carried() && x.alt < 1.0 && (x.pos - from).length() < radius + delta.length() + 4.0).map(|x| (x.pos, super::footing::walk_radius(a, x))))
            .collect();
        let obs = Obstacles { solids: &solids, bodies: &bodies };
        let terrain = &self.snap.terrain;
        let goes = |x: f32, z: f32| {
            let g = terrain.height(x, z);
            if airborne {
                true
            } else if water {
                g < WATER_LEVEL + 0.2
            } else {
                g > WATER_LEVEL - MAX_WADE
            }
        };
        let free = |x: f32, z: f32| goes(x, z) && (obs.dist_body(x, z, feet, cap) >= radius || obs.clear(x, z, feet, cap));
        let mut to = from;
        if delta.length() > 1e-5 {
            let stuck = obs.dist_body(from.x, from.z, feet, cap) < radius * 0.5 && !obs.clear(from.x, from.z, feet, cap);
            let tries = [Vec2::new(delta.x, delta.z), Vec2::new(delta.x, 0.0), Vec2::new(0.0, delta.z)];
            for (i, d) in tries.iter().enumerate() {
                let (x, z) = (from.x + d.x, from.z + d.y);
                if (stuck && goes(x, z)) || free(x, z) {
                    to = Vec3::new(x, feet, z);
                    if i > 0 {
                        v *= 0.9;
                    }
                    break;
                }
            }
            if to == from {
                // Ran into something: it stops dead.
                if v.abs() > 3.0 {
                    self.event("crashed", Some(who), Some(format!("thing:{id}")), format!("the {} runs into something", ty.name()), Some(from), json!({ "speed": v.abs() }));
                }
                v = 0.0;
            }
        }

        // Height: on the ground, afloat, or flying.
        let g = self.snap.terrain.height(to.x, to.z);
        let support = support_under(&self.snap.terrain, &solids, to, radius * 0.7, feet + cap.step);
        let draft = (height * 0.25).clamp(0.1, 1.0);
        let bob = (self.t as f32 * 1.3 + id as f32).sin() * 0.04;
        let new_feet = if water {
            if g < WATER_LEVEL - draft { WATER_LEVEL - draft + bob } else { support.max(WATER_LEVEL - draft) }
        } else if air {
            // Up or down as asked, in place or moving; let be, it hovers.
            (feet + climb.clamp(-1.0, 1.0) * CLIMB * dt).max(support).min(g.max(WATER_LEVEL) + 150.0)
        } else {
            // Down a slope at once; up only what it can roll over.
            support
        };
        let afloat = water && g < WATER_LEVEL - draft;

        // Lean with the ground under it (or into the turn in the air).
        let tilt = if air && new_feet - g.max(WATER_LEVEL) > 0.5 {
            let rise = (new_feet - feet) / (v.abs() * dt).max(0.05);
            Quat::from_rotation_x(-rise.atan().clamp(-0.4, 0.4)) * Quat::from_rotation_z(-steer * 0.25)
        } else if afloat {
            Quat::from_rotation_z((self.t as f32 * 1.1 + id as f32).sin() * 0.03 - steer * 0.05 * (v.abs() / max))
        } else {
            let (hz, hx) = (b[2] * sc * 0.8, b[0] * sc * 0.8);
            let right = Vec3::new(fwd.z, 0.0, -fwd.x);
            let h = |p: Vec3| self.snap.terrain.height(p.x, p.z).max(if water { WATER_LEVEL - draft } else { f32::MIN });
            let pitch = ((h(to + fwd * hz) - h(to - fwd * hz)) / (2.0 * hz).max(0.1)).atan().clamp(-0.5, 0.5);
            let roll = ((h(to + right * hx) - h(to - right * hx)) / (2.0 * hx).max(0.1)).atan().clamp(-0.5, 0.5);
            Quat::from_rotation_x(-pitch) * Quat::from_rotation_z(roll)
        };
        let tilt = t.tilt().slerp(tilt, (dt * 8.0).min(1.0));

        let dyaw = yaw - t.yaw;
        if let Some(th) = self.things.get_mut(id) {
            th.pos = Vec3::new(to.x, new_feet - ty.bottom * sc, to.z);
            th.yaw = yaw;
            th.set_tilt(tilt);
            th.vel = Vec3::ZERO;
            th.asleep = true;
            th.dirty = true;
        }
        self.things.moved();
        self.drives.insert(id, v);
        if let Some(a) = self.actor_mut(who) {
            a.yaw += dyaw;
            a.moved = (to - from).length();
            a.alt = (new_feet - g.max(WATER_LEVEL)).max(0.0);
        }
        self.update_aboard();
    }
}
