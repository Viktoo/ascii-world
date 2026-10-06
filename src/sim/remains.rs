//! Remains. A being that dies leaves its body behind: a live thing of its
//! own body's shape, in its look, lying where it fell. Being a thing, the
//! world's rules go on acting on it (it cools, gets wet, burns, a curse
//! lingers), it can be struck, cut, dragged or carried by two, and others
//! come upon it. Nothing about it is written for swords: any death leaves one.
//!
//! How it lies is read from the body, not drawn for it: upright bodies
//! topple away from what struck them, the rest roll onto their side, and the
//! shape itself is probed for how low it reaches so it rests on the ground.

use super::actor::{L_RAISE, R_RAISE};
use super::props::*;
use super::things::{Origin, Thing, ThingId};
use super::{ActorId, Sim};
use crate::render::GpuInst;
use crate::render::sky::DAY_SECONDS;
use glam::{Quat, Vec3};

/// How long the fall takes (s).
pub const FALL_SECS: f32 = 0.8;
/// Game days a body lies before it is gone (taken only when nobody is near).
const REMAINS_DAYS: f64 = 3.0;
/// How far off the traveler must be for remains to go unseen (m).
const UNSEEN: f32 = 90.0;

impl Sim {
    /// What is left of a being that died: its body, fallen. `from` is
    /// where the blow came from (it falls away from it).
    pub fn leave_remains(&mut self, cid: i64, from: Option<Vec3>) -> Option<ThingId> {
        if self.things.len() >= self.cfg.max_things {
            return None;
        }
        let n = self.cast.get(cid)?;
        let figure = self.snap.figure_type.and_then(|f| self.snap.type_of(f));
        let ty = self.snap.type_of(n.body_ty).or(figure)?.clone();
        let gait = ty.ct.meta.body.as_ref().map(|b| b.gait.clone()).unwrap_or_else(|| "biped".into());
        let a = &n.a;
        // Its look, as it was drawn alive; limp, arms a little out.
        let g = n.gpu(&ty);
        let params = [g.k0[0], g.k0[1], g.k0[2], g.k0[3], g.k1[0], g.k1[1], g.k1[2], g.k1[3]];
        let mut state = [0.0f32; 8];
        if a.dims.arms {
            state[L_RAISE] = 0.12;
            state[R_RAISE] = 0.3;
        }
        let mut props = if n.props.len() == self.vocab.len() { n.props.clone() } else { self.body_base(n) };
        props[P_ALIVE] = 0.0;
        props[P_HEAT] = 0.0;
        props[P_HEALTH] = 1.0;
        props[P_GROWTH] = 1.0;
        props[P_FORCE] = 0.0;
        props[P_KIND] = 0.0;
        props[P_MASS] = a.dims.mass.max(0.5);
        // Where it stood: its feet's ground (a floor, or the land under a flier).
        let ground = if a.grounded && a.alt <= 0.0 { a.pos.y } else { self.snap.terrain.height(a.pos.x, a.pos.z) };
        let pos = Vec3::new(a.pos.x, ground, a.pos.z);
        let away = from.map(|f| Vec3::new(pos.x - f.x, 0.0, pos.z - f.z)).filter(|d| d.length_squared() > 1e-4).map(|d| d.normalize()).unwrap_or(-a.forward());
        let tilt = fall_tilt(&gait, a.yaw, a.right(), away);
        let (yaw, scale) = (a.yaw, a.dims.scale);
        let name = n.name().to_string();
        let id = self.things.alloc();
        let mut th = Thing::new(id, &ty, pos, yaw, scale, params, props, self.t);
        th.state = state;
        th.origin = Origin { remains: Some(cid), ..Default::default() };
        th.set_tilt(tilt);
        th.asleep = true;
        th.dirty = true;
        if let Some(low) = lowest_point(&super::render::thing_inst(&th, &ty, [0.0; 4]), &ty.ct) {
            th.pos.y += ground - low;
        }
        th.note_edit(self.t, &name, "died here");
        self.things.insert(th);
        Some(id)
    }

    /// What remains are called: "body of Oda".
    pub fn remains_name(&self, cid: i64) -> String {
        match self.cast.get(cid) {
            Some(n) => format!("body of {}", n.name()),
            None => "body".into(),
        }
    }

    /// A tilted thing come to rest is lifted out of the ground it sank
    /// into (physics sees it as a sphere about its upright centre).
    pub fn rest_tilted(&mut self, id: ThingId) {
        let Some(t) = self.things.get(id) else { return };
        if t.tilt() == Quat::IDENTITY {
            return;
        }
        let Some(ty) = self.snap.type_of(t.type_id).cloned() else { return };
        let g = super::render::thing_inst(t, &ty, [0.0; 4]);
        let Some(low) = lowest_point(&g, &ty.ct) else { return };
        let c = g.center();
        let ground = self.snap.terrain.height(c.x, c.z);
        if low < ground {
            if let Some(t) = self.things.get_mut(id) {
                t.pos.y += ground - low;
                t.dirty = true;
            }
        }
    }

    /// Remains as drawn right now: still falling for a moment after death.
    pub fn falling(&self, t: &Thing) -> Option<Thing> {
        let cid = t.origin.remains?;
        let u = ((self.t - t.born) as f32 / FALL_SECS).clamp(0.0, 1.0);
        if u >= 1.0 || t.holder.is_some() {
            return None;
        }
        // Gravity: slow to tip, quick to land.
        let k = u * u;
        let stand = self.cast.get(cid).map(|n| n.a.pos.y).unwrap_or(t.pos.y);
        let mut shown = t.clone();
        shown.set_tilt(Quat::IDENTITY.slerp(t.tilt(), k));
        shown.pos.y = stand + (t.pos.y - stand) * k;
        Some(shown)
    }

    /// Long-dead bodies are gone (out of sight); what they wore stays.
    pub fn step_remains(&mut self) {
        let old: Vec<(ThingId, i64, Vec3)> = self
            .things
            .live()
            .filter_map(|t| t.origin.remains.map(|c| (t.id, c, t.pos)))
            .filter(|(id, _, p)| self.things.get(*id).is_some_and(|t| self.t - t.born > REMAINS_DAYS * DAY_SECONDS && t.holder.is_none()) && self.dist_to_player(*p) > UNSEEN)
            .collect();
        for (id, cid, pos) in old {
            for w in self.worn_by(ActorId::Npc(cid)) {
                if let Some(t) = self.things.get_mut(w) {
                    t.worn = None;
                    t.pos = pos;
                    t.asleep = false;
                    t.dirty = true;
                }
            }
            self.things.remove(id);
        }
    }
}

/// How a body lies once it has fallen (a tilt after its yaw). Upright
/// walkers topple over away from the blow; four-legged, crawling and
/// floating bodies roll onto the side away from it.
pub fn fall_tilt(gait: &str, yaw: f32, right: Vec3, away: Vec3) -> Quat {
    const DOWN: f32 = std::f32::consts::FRAC_PI_2 * 0.97;
    if gait == "biped" {
        let axis = Vec3::Y.cross(away).normalize_or(Vec3::X);
        let local = Quat::from_rotation_y(yaw).inverse() * axis;
        // A little twist, so no two lie quite alike.
        let twist = (away.dot(right) * 0.35).clamp(-0.35, 0.35);
        Quat::from_axis_angle(local, DOWN) * Quat::from_rotation_y(twist)
    } else {
        let side = if away.dot(right) >= 0.0 { 1.0 } else { -1.0 };
        Quat::from_rotation_z(-side * DOWN)
    }
}

/// The lowest point of a drawn shape (world y), found by marching up
/// columns through its bounding sphere until each meets the surface.
pub fn lowest_point(g: &GpuInst, ct: &crate::lang::CompiledType) -> Option<f32> {
    const N: usize = 11;
    let c = g.center();
    let r = g.radius().max(0.05);
    let eps = (r * 0.004).max(0.002);
    let mut low: Option<f32> = None;
    for i in 0..N {
        for j in 0..N {
            let x = c.x + r * ((i as f32 + 0.5) / N as f32 * 2.0 - 1.0);
            let z = c.z + r * ((j as f32 + 0.5) / N as f32 * 2.0 - 1.0);
            let (dx, dz) = (x - c.x, z - c.z);
            if dx * dx + dz * dz > r * r {
                continue;
            }
            let top = low.unwrap_or(c.y + r).min(c.y + r);
            let mut y = c.y - r;
            for _ in 0..96 {
                if y >= top {
                    break;
                }
                let d = g.sdf(ct, Vec3::new(x, y, z));
                if !d.is_finite() {
                    break;
                }
                if d < eps {
                    low = Some(low.map_or(y, |l| l.min(y)));
                    break;
                }
                y += (d * 0.8).max(eps);
            }
        }
    }
    low
}
