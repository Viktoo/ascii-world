//! Hinges: doors, gates, lids and trapdoors swing open and shut. A shape
//! with `meta.joint` turns about its hinge as far as it is open; its
//! collision turns with it, so an open door lets you through. Anyone with
//! hands works one, and so does a body big and clever enough to push it.

use super::actions::{ActErr, Outcome};
use super::things::{Thing, ThingId};
use super::{ActorId, Note, Sim, Target};
use crate::render::GpuInst;
use crate::world::TypeEntry;
use glam::{Quat, Vec3};
use serde_json::json;

/// Seconds a hinge takes to swing all the way.
pub const SWING_SECS: f32 = 0.8;

/// Turn a drawn instance about its type's hinge, as far as the thing is open.
pub fn pose(g: &mut GpuInst, t: &Thing, ty: &TypeEntry) {
    let Some(j) = &ty.ct.meta.joint else { return };
    let open = t.shape.open.clamp(0.0, 1.0);
    if open < 1e-4 {
        return;
    }
    let at = Vec3::from_array(j.at);
    let pivot = g.from_local(at);
    let h = Quat::from_axis_angle(Vec3::from_array(j.axis), j.open * open);
    g.set_tilt(g.tilt_q().unwrap_or(Quat::IDENTITY) * h);
    let p = g.pos() + (pivot - g.from_local(at));
    g.pos_scale[0] = p.x;
    g.pos_scale[1] = p.y;
    g.pos_scale[2] = p.z;
}

impl Sim {
    /// Whether a body can work a hinge: hands, or a mind and the size to push one.
    pub fn can_work_hinges(&self, who: ActorId) -> bool {
        match who {
            ActorId::Player => true,
            ActorId::Npc(c) => self.cast.get(c).is_some_and(|n| {
                n.a.dims.arms || (n.species.mind != crate::world::species::Mind::Instinct && n.a.dims.height >= 0.7)
            }),
        }
    }

    /// Whether a thing swings on a hinge.
    pub fn hinged(&self, id: ThingId) -> bool {
        self.things.get(id).and_then(|t| self.snap.type_of(t.type_id)).is_some_and(|ty| ty.ct.meta.joint.is_some())
    }

    /// Whether a target (live or still static) swings on a hinge.
    pub fn target_hinged(&self, t: &Target) -> bool {
        match t {
            Target::Thing(id) => self.hinged(*id),
            Target::Instance(i) => self.snap.instances.iter().find(|p| p.id == *i).and_then(|p| self.snap.type_of(p.type_id)).is_some_and(|ty| ty.ct.meta.joint.is_some()),
            _ => false,
        }
    }

    /// Open or shut a hinged thing (`shut`: None toggles).
    pub fn open(&mut self, who: ActorId, target: &Target, shut: Option<bool>) -> Result<Outcome, ActErr> {
        let r = self.resolve(target, who).ok_or_else(|| ActErr::Fail("there is nothing like that here".into()))?;
        if !self.target_hinged(&r.target) {
            return Err(ActErr::Fail(format!("{} doesn't open", super::actions::the(&r.name))));
        }
        if !self.can_work_hinges(who) {
            return Err(ActErr::Fail(format!("{} can't work {}", self.actor_name(who), super::actions::the(&r.name))));
        }
        let me = self.actor(who).map(|a| a.pos).unwrap_or_default();
        let near = r.nearest(me);
        if !self.in_reach(who, near) {
            return Err(ActErr::TooFar { at: r.pos, dist: (near - me).length() });
        }
        let id = self.liven(&r.target).ok_or_else(|| ActErr::Fail(format!("can't move {}", super::actions::the(&r.name))))?;
        if let Some(p) = self.vocab.id("locked") {
            if self.things.get(id).is_some_and(|t| t.props.get(p).is_some_and(|v| *v > 0.5)) {
                let msg = format!("{} is locked", super::actions::the(&r.name));
                self.note_near(r.pos, 12.0, Note::seen(format!("{}.", super::physics::cap(&msg)), who == ActorId::Player));
                return Err(ActErr::Fail(msg));
            }
        }
        let now_open = self.things.get(id).map(|t| t.shape.open_to > 0.5).unwrap_or(false);
        let want = match shut {
            Some(s) => !s,
            None => !now_open,
        };
        let name = self.actor_name(who);
        let what = if want { "opens" } else { "shuts" };
        let msg = format!("{name} {what} {}", super::actions::the(&r.name));
        if want == now_open {
            return Ok(Outcome::ok(format!("{} is already {}", super::actions::the(&r.name), if want { "open" } else { "shut" })));
        }
        if let Some(t) = self.things.get_mut(id) {
            t.shape.open_to = if want { 1.0 } else { 0.0 };
            t.dirty = true;
        }
        if let Some(a) = self.actor_mut(who) {
            a.face(r.pos - me, 1.0);
            // A push or a pull with the arm.
            a.pose[super::actor::R_FWD] = a.pose[super::actor::R_FWD].max(0.6);
        }
        let at = r.pos;
        self.event(if want { "opened" } else { "shut" }, Some(who), Some(format!("thing:{id}")), msg.clone(), Some(at), json!({}));
        if who != ActorId::Player {
            self.note_near(at, 15.0, Note::Ambient(format!("{}.", super::physics::cap(&msg))));
        }
        if let Some(ty) = self.things.get(id).and_then(|t| self.snap.type_of(t.type_id)).cloned() {
            let props = self.things.get(id).map(|t| t.props.clone()).unwrap_or_default();
            let mat = crate::audio::call::Material::of(&props, &ty.ct.meta.tags, crate::audio::call::Material::from_meta(&ty.ct.meta.sound).as_ref());
            self.cue(at, Some(who), crate::audio::Heard::Hit { mat, mass: props[super::props::P_MASS].min(20.0), speed: 1.4, by: None });
        }
        Ok(Outcome::ok(msg).thing(id))
    }

    /// Hinges swing towards where they are going.
    pub fn step_joints(&mut self, dt: f32) {
        let moving: Vec<ThingId> = self.things.live().filter(|t| t.shape.open != t.shape.open_to).map(|t| t.id).collect();
        let step = dt / SWING_SECS;
        for id in moving {
            if let Some(t) = self.things.get_mut(id) {
                let d = t.shape.open_to - t.shape.open;
                t.shape.open += d.clamp(-step, step);
                if (t.shape.open - t.shape.open_to).abs() < 1e-4 {
                    t.shape.open = t.shape.open_to;
                }
                t.dirty = true;
            }
        }
    }

    /// A closed hinged thing within `r` of `p` (a door in the way), if any.
    pub fn shut_door_near(&mut self, p: Vec3, r: f32) -> Option<Target> {
        let snap = self.snap.clone();
        for id in self.things.near(p, r + 3.0) {
            let Some(t) = self.things.get(id) else { continue };
            let Some(ty) = snap.type_of(t.type_id) else { continue };
            if ty.ct.meta.joint.is_some() && t.shape.open_to < 0.5 && (super::render::thing_inst(t, ty, [0.0; 4]).center() - p).length() < r + 1.2 {
                return Some(Target::Thing(id));
            }
        }
        for pl in snap.near_chunk(crate::world::chunk_of(p.x, p.z)) {
            if self.things.by_instance.contains_key(&pl.id) {
                continue;
            }
            let Some(ty) = snap.type_of(pl.type_id) else { continue };
            if ty.ct.meta.joint.is_some() && (pl.gpu(ty, 1.0).center() - p).length() < r + 1.2 {
                return Some(Target::Instance(pl.id));
            }
        }
        None
    }
}
