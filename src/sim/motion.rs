//! Moving in other ways than walking: flying (bodies that fly cruise above
//! the land and what stands on it, and land to do anything else), and
//! riding (a body sits in another's seat and goes where it goes).

use super::actions::{ActErr, Outcome};
use super::{ActorId, Note, Sim};
use glam::Vec3;
use serde_json::json;

/// How high above the ground and what stands on it a flyer keeps.
const CLEARANCE: f32 = 3.0;
const CLIMB: f32 = 5.0;

impl Sim {
    /// Whether an actor can fly (its body flies and its species has wings for it).
    pub fn can_fly(&self, who: ActorId) -> bool {
        match who {
            ActorId::Npc(c) => self.cast.get(c).is_some_and(|n| n.a.dims.flies && n.species.moves.fly > 0.0),
            ActorId::Player => false,
        }
    }

    pub fn fly_speed(&self, who: ActorId) -> f32 {
        match who {
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.species.moves.fly).unwrap_or(0.0),
            ActorId::Player => 0.0,
        }
    }

    /// The height to keep over the land and anything standing on it, around `p`.
    fn cruise_height(&mut self, p: Vec3, dir: Vec3, height: f32) -> f32 {
        let mut top: f32 = crate::terrain::WATER_LEVEL;
        for k in 0..5 {
            let q = p + dir * (k as f32 * 4.0);
            top = top.max(self.snap.terrain.height(q.x, q.z));
        }
        let solids = self.solids_near(p + dir * 8.0, 14.0);
        for s in &solids {
            top = top.max(s.inst.center().y + s.inst.radius());
        }
        top + CLEARANCE + height * 0.5
    }

    /// Fly towards `p`: climb over what is ahead, cruise, and come down to
    /// land when close. True while it can go on.
    pub fn fly_toward(&mut self, who: ActorId, p: Vec3, speed: f32, dt: f32, land: bool) -> bool {
        let Some((me, height, radius)) = self.actor(who).map(|a| (a.pos, a.dims.height, a.dims.radius)) else { return false };
        let flat = Vec3::new(p.x - me.x, 0.0, p.z - me.z);
        let len = flat.length();
        let dir = if len > 1e-3 { flat / len } else { Vec3::ZERO };
        let ground = self.snap.terrain.height(me.x, me.z).max(crate::terrain::WATER_LEVEL - 0.2);
        let landing = land && len < 6.0 + height * 2.0 + radius;
        let want = if landing { self.snap.terrain.height(p.x, p.z).max(ground) } else { self.cruise_height(me, dir, height) };
        let dy = (want - me.y).clamp(-CLIMB * dt, CLIMB * dt);
        // Climb first when something is in the way; glide in to land.
        let below = want - me.y;
        let k = if below > 1.0 { 0.25 } else if landing { 0.6 } else { 1.0 };
        let step = dir * (speed * dt * k).min(len);
        let next = Vec3::new(me.x + step.x, me.y + dy, me.z + step.z);
        let floor = self.snap.terrain.height(next.x, next.z);
        if let Some(a) = self.actor_mut(who) {
            if dir != Vec3::ZERO {
                a.face(dir, dt * 4.0);
            }
            a.pos = Vec3::new(next.x, next.y.max(floor), next.z);
            a.alt = (a.pos.y - floor).max(0.0);
            a.moved = step.length() + dy.abs();
            a.phase += step.length() * 1.5;
            a.asleep = false;
            a.stuck = 0.0;
        }
        true
    }

    /// Settle a flyer that has nothing to do: down to the ground.
    pub fn settle(&mut self, who: ActorId, dt: f32) {
        let Some(a) = self.actor(who) else { return };
        if a.alt <= 0.0 || a.riding.is_some() {
            return;
        }
        let p = a.pos;
        self.fly_toward(who, p, 0.0, dt, true);
    }

    /// A mount of their own nearby, free to ride.
    pub fn own_mount(&self, who: ActorId, range: f32) -> Option<ActorId> {
        let p = self.actor(who)?.pos;
        self.cast
            .npcs
            .iter()
            .filter(|n| !n.dead && !n.a.asleep && n.a.dims.seat.is_some() && (n.a.pos - p).length() < range)
            .map(|n| ActorId::Npc(n.def.id))
            .find(|m| self.owner_of(match m { ActorId::Npc(c) => *c, _ => 0 }) == Some(who) && self.rider_of(*m).is_none())
    }

    /// Who rides this body, if anyone.
    pub fn rider_of(&self, mount: ActorId) -> Option<ActorId> {
        self.actor_ids().into_iter().find(|a| self.actor(*a).is_some_and(|x| x.riding == Some(mount)))
    }

    /// Get on another's back: it needs a seat, to be much bigger, and to agree.
    pub fn ride(&mut self, who: ActorId, mount: ActorId) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let mname = self.actor_name(mount);
        if who == mount {
            return Err(ActErr::Fail("you can't ride yourself".into()));
        }
        let (Some(me), Some(m)) = (self.actor(who).cloned(), self.actor(mount).cloned()) else { return Err(ActErr::Fail("no such one".into())) };
        if me.riding.is_some() {
            return Err(ActErr::Fail(format!("{name} is already riding")));
        }
        if m.dims.seat.is_none() {
            return Err(ActErr::Fail(format!("{mname} can't be ridden")));
        }
        if m.dims.mass < me.dims.mass * 3.0 || me.dims.mass > self.strength(mount) * 1.6 {
            return Err(ActErr::Fail(format!("{name} is too big for {mname} to carry")));
        }
        if self.rider_of(mount).is_some() {
            return Err(ActErr::Fail(format!("someone is already riding {mname}")));
        }
        let d = Vec3::new(m.pos.x - me.pos.x, 0.0, m.pos.z - me.pos.z).length();
        if d > self.reach_of(who) + m.dims.radius + 0.5 {
            return Err(ActErr::TooFar { at: m.pos, dist: d });
        }
        if let ActorId::Npc(c) = mount {
            let tame = self.cast.get(c).map(|n| n.species.temper.tame).unwrap_or(0.5);
            let aff = self.social.affection(mount, who) - self.wariness(c);
            if aff < 0.25 || tame < 0.2 {
                self.event("refused", Some(mount), Some(who.key()), format!("{mname} shies away from {name}"), Some(m.pos), json!({ "ride": true }));
                self.note_near(m.pos, 20.0, Note::Info(format!("{} shies away and won't be ridden.", super::physics::cap(&mname))));
                if !self.speaks(mount) {
                    self.make_noise(c, false);
                }
                return Err(ActErr::Fail(format!("{mname} won't let {name} on")));
            }
            if let Some(n) = self.cast.get_mut(c) {
                n.plan.clear();
                n.a.task = None;
                n.a.asleep = false;
            }
        }
        if let Some(a) = self.actor_mut(who) {
            a.riding = Some(mount);
            a.alt = 0.0;
        }
        self.update_riders();
        let msg = format!("{name} climbs onto {mname}");
        self.event("mounted", Some(who), Some(mount.key()), msg.clone(), Some(m.pos), json!({}));
        self.note_near(m.pos, 25.0, Note::Info(format!("{}.", super::physics::cap(&msg))));
        self.social.bond(who, mount, 0.03, self.t);
        Ok(Outcome::ok(msg))
    }

    /// Get down, beside the mount.
    pub fn dismount(&mut self, who: ActorId) -> Result<Outcome, ActErr> {
        let Some(mount) = self.actor(who).and_then(|a| a.riding) else { return Err(ActErr::Fail("not riding".into())) };
        self.set_down_rider(who, mount, 0.6);
        let msg = format!("{} gets down from {}", self.actor_name(who), self.actor_name(mount));
        let at = self.actor(who).map(|a| a.pos);
        self.event("dismounted", Some(who), Some(mount.key()), msg.clone(), at, json!({}));
        Ok(Outcome::ok(msg))
    }

    fn set_down_rider(&mut self, who: ActorId, mount: ActorId, gap: f32) {
        let (mp, right, r) = self.actor(mount).map(|m| (m.pos, m.right(), m.dims.radius)).unwrap_or_default();
        let p = mp + right * (r + gap);
        let p = Vec3::new(p.x, self.snap.terrain.height(p.x, p.z), p.z);
        if let Some(a) = self.actor_mut(who) {
            a.riding = None;
            a.pos = p;
            a.alt = 0.0;
        }
    }

    /// A bolting or rearing mount throws its rider off.
    pub fn throw_rider(&mut self, mount: ActorId) {
        let Some(rider) = self.rider_of(mount) else { return };
        self.set_down_rider(rider, mount, 1.4);
        let (rn, mn) = (self.actor_name(rider), self.actor_name(mount));
        let at = self.actor(rider).map(|a| a.pos);
        let msg = format!("{mn} throws {rn} off");
        self.event("thrown", Some(mount), Some(rider.key()), msg.clone(), at, json!({}));
        if let Some(p) = at {
            self.note_near(p, 30.0, Note::Info(format!("{}!", super::physics::cap(&msg))));
            self.witness(p, 25.0, &msg, 0.5, &[]);
        }
        let r = self.social.rel_mut(rider, mount);
        r.trust = (r.trust - 0.1).max(-1.0);
    }

    /// Riders sit in their mounts' seats.
    pub fn update_riders(&mut self) {
        let riders: Vec<(ActorId, ActorId)> = self.actor_ids().into_iter().filter_map(|a| self.actor(a).and_then(|x| x.riding).map(|m| (a, m))).collect();
        for (r, m) in riders {
            let Some((seat, yaw, alt)) = self.actor(m).and_then(|x| Some((x.seat()?, x.yaw, x.alt))) else {
                if let Some(a) = self.actor_mut(r) {
                    a.riding = None;
                }
                continue;
            };
            if let Some(a) = self.actor_mut(r) {
                // Hips in the seat.
                let hip = 0.67 * a.dims.height / 1.75;
                a.pos = seat - Vec3::Y * hip;
                a.alt = alt;
                a.task = match a.task.take() {
                    Some(t @ super::actor::Task::Goto { .. }) | Some(t @ super::actor::Task::Follow { .. }) => Some(t),
                    _ => None,
                };
                if r != ActorId::Player {
                    a.yaw = yaw;
                }
            }
        }
    }

    /// The traveller steers what they ride: walk keys move it at its pace,
    /// and on a flyer, looking up or down climbs or dives.
    pub fn drive(&mut self, dir: Vec3, pitch: f32, dt: f32) {
        let Some(m) = self.player.riding else { return };
        let (walk, run) = self.speeds(m);
        let flyer = self.can_fly(m);
        let speed = if flyer && self.actor(m).is_some_and(|a| a.alt > 1.0) { self.fly_speed(m) } else { run.max(walk) };
        if flyer && (pitch > 0.2 || self.actor(m).is_some_and(|a| a.alt > 0.5)) && dir != Vec3::ZERO {
            let Some((p, alt)) = self.actor(m).map(|a| (a.pos, a.alt)) else { return };
            let climb = (pitch * 2.0).clamp(-1.0, 1.0) * CLIMB * dt;
            let next = p + dir * speed * dt;
            let floor = self.snap.terrain.height(next.x, next.z).max(crate::terrain::WATER_LEVEL - 0.2);
            let y = (p.y + climb).max(floor);
            if let Some(a) = self.actor_mut(m) {
                a.face(dir, dt * 4.0);
                a.pos = Vec3::new(next.x, y, next.z);
                a.alt = (y - floor).max(0.0);
                a.phase += speed * dt * 1.5;
                let _ = alt;
            }
        } else if dir != Vec3::ZERO {
            let delta = dir * speed * dt;
            if let Some(a) = self.actor_mut(m) {
                a.face(dir, dt * 6.0);
            }
            self.walk(m, delta);
            if let Some(a) = self.actor_mut(m) {
                a.phase += a.moved * 3.2;
            }
        }
        self.update_riders();
    }
}
