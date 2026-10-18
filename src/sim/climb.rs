//! Climbing: a stair too steep to walk (a ladder, rungs up a trunk, a rope
//! ladder, footholds up a rock face, a ladder down a hatch) is climbed. A
//! body walking into its foot goes up it, one walking off its top goes down
//! it, and one falling past it catches hold. On it, a body is held (no
//! falling) and moves along it; at either end it steps off. A shut hatch
//! over the top stops a climber until it is opened (people open it). Nothing
//! here knows what is climbed: it is a `stairs` anchor steeper than a body
//! walks.

use super::{ActorId, Sim};
use crate::world::WorldSnapshot;
use glam::{Vec2, Vec3};

/// One stair or ladder in the world.
#[derive(Clone, Debug)]
pub struct Climb {
    pub foot: Vec3,
    pub top: Vec3,
    /// From the foot toward the top in the ground plane (a ladder straight
    /// up: the way its anchor faces), unit length.
    pub d: Vec2,
    /// Too steep to walk: climbed.
    pub steep: bool,
}

/// Steeper than this (rise over run) is climbed.
const STEEP: f32 = 1.3;

impl Climb {
    /// Where on it a body at height `y` holds on (its line, in x, z).
    fn line(&self, y: f32) -> Vec2 {
        let t = ((y - self.foot.y) / (self.top.y - self.foot.y).max(0.1)).clamp(0.0, 1.0);
        Vec2::new(self.foot.x, self.foot.z).lerp(Vec2::new(self.top.x, self.top.z), t)
    }

    /// How far `p` is from it (in 3D).
    pub fn dist(&self, p: Vec3) -> f32 {
        let (a, b) = (self.foot, self.top);
        let h = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0);
        (p - (a + (b - a) * h)).length()
    }
}

/// Every stair and ladder of a world version.
pub fn climbs_of(snap: &WorldSnapshot) -> Vec<Climb> {
    let mut out = Vec::new();
    for p in &snap.instances {
        let Some(ty) = snap.type_of(p.type_id) else { continue };
        let g = p.gpu(ty, 1.0);
        for a in ty.ct.meta.anchors.iter().filter(|a| a.kind == "stairs") {
            let Some(to) = a.to else { continue };
            let (foot, top) = (g.from_local(Vec3::from_array(a.at)), g.from_local(Vec3::from_array(to)));
            if top.y < foot.y + 0.5 {
                continue;
            }
            let run = Vec2::new(top.x - foot.x, top.z - foot.z);
            let face = g.from_local(Vec3::from_array(a.at) + Vec3::from_array(a.out())) - foot;
            let d = if run.length() > 0.15 { run.normalize() } else { Vec2::new(face.x, face.z).normalize_or(Vec2::Y) };
            out.push(Climb { foot, top, d, steep: top.y - foot.y > run.length() * STEEP });
        }
    }
    out
}

impl Sim {
    /// The ladder a body could take hold of at `p` going `up` (or down):
    /// at its foot going up, at its top going down.
    fn ladder_at(&self, p: Vec3, r: f32, up: bool) -> Option<usize> {
        self.climbs.iter().position(|c| {
            if !c.steep {
                return false;
            }
            if up {
                p.y > c.foot.y - 0.5 && p.y < c.top.y - 0.3 && (Vec2::new(p.x, p.z) - c.line(p.y)).length() < r + 0.6
            } else {
                (p.y - c.top.y).abs() < 0.6 && (Vec2::new(p.x, p.z) - Vec2::new(c.top.x, c.top.z)).length() < r + 0.9
            }
        })
    }

    /// Walking on or onto a ladder: `delta` (x, z) as walked. The traveler
    /// goes up walking toward it, down walking away (or looking down);
    /// true when the step was spent climbing.
    pub fn climb_walk(&mut self, id: ActorId, delta: Vec3) -> bool {
        let Some(a) = self.actor(id) else { return false };
        let (p, r) = (a.pos, super::footing::walk_radius(id, a));
        let len = Vec2::new(delta.x, delta.z).length();
        let held = a.ladder;
        let dir = |c: &Climb| {
            let along = Vec2::new(delta.x, delta.z).dot(c.d) / len.max(1e-6);
            let down_look = id == ActorId::Player && self.look_pitch < -0.45;
            if along > 0.3 { if down_look { -1.0 } else { 1.0 } } else if along < -0.3 { -1.0 } else { 0.0 }
        };
        let i = match held {
            Some(i) => i as usize,
            None if len < 1e-5 => return false,
            None => {
                let up = self.ladder_at(p, r, true).filter(|i| dir(&self.climbs[*i]) > 0.0);
                let down = self.ladder_at(p, r, false).filter(|i| Vec2::new(delta.x, delta.z).dot(self.climbs[*i].d) < -0.5 * len);
                match up.or(down) {
                    Some(i) => i,
                    None => return false,
                }
            }
        };
        let Some(c) = self.climbs.get(i).cloned() else {
            self.off_ladder(id);
            return false;
        };
        let v = dir(&c);
        self.climb_by(id, i, v * len * 0.7);
        true
    }

    /// Climb ladder `i` by `dy` (m; up when positive), holding on.
    pub fn climb_by(&mut self, id: ActorId, i: usize, dy: f32) {
        let Some(c) = self.climbs.get(i).cloned() else { return };
        let Some(a) = self.actor(id) else { return };
        let (p, r, height) = (a.pos, super::footing::walk_radius(id, a), a.dims.height);
        let mut y = (p.y.clamp(c.foot.y - 0.5, c.top.y + 0.6) + dy).max(c.foot.y);
        // Going down from its top through a shut hatch: people open it first;
        // the traveler stays on top.
        if dy < 0.0 && p.y > c.top.y - 0.4 {
            if let Some(lid) = self.shut_door_near(c.top + Vec3::Y * 0.2, 0.6) {
                if id != ActorId::Player && self.can_work_hinges(id) {
                    let _ = self.open(id, &lid, Some(false));
                }
                return;
            }
        }
        // A shut hatch over its top: people open it; the traveler waits under it.
        if dy > 0.0 && y + height > c.top.y - 0.2 {
            if let Some(lid) = self.shut_door_near(c.top + Vec3::Y * 0.2, 0.6) {
                if id != ActorId::Player && self.can_work_hinges(id) {
                    let _ = self.open(id, &lid, Some(false));
                } else {
                    y = y.min(c.top.y - height - 0.15).max(p.y.min(c.top.y));
                }
            }
        }
        let (pos, on) = if y >= c.top.y - 0.02 {
            // Off at the top, onto what it reaches.
            (Vec3::new(c.top.x, c.top.y, c.top.z) + Vec3::new(c.d.x, 0.0, c.d.y) * 0.1, false)
        } else if y <= c.foot.y + 0.02 && dy <= 0.0 {
            // Off at the foot, stepping back from it.
            let f = Vec2::new(c.foot.x, c.foot.z) - c.d * (r + 0.1);
            (Vec3::new(f.x, c.foot.y, f.y), false)
        } else {
            let l = c.line(y) - c.d * (r + 0.08);
            (Vec3::new(l.x, y, l.y), true)
        };
        if let Some(a) = self.actor_mut(id) {
            a.moved = (pos - a.pos).length();
            a.pos = pos;
            a.vy = 0.0;
            a.grounded = true;
            a.support = None;
            a.ladder = on.then_some(i as u32);
            if on {
                // Facing the ladder.
                a.yaw = c.d.x.atan2(c.d.y);
            }
        }
    }

    pub fn off_ladder(&mut self, id: ActorId) {
        if let Some(a) = self.actor_mut(id) {
            a.ladder = None;
        }
    }

    /// Falling past a ladder: catch hold of it.
    pub fn catch_ladder(&mut self, id: ActorId) -> bool {
        let Some(a) = self.actor(id) else { return false };
        if a.ladder.is_some() || a.grounded || a.vy > -0.5 {
            return false;
        }
        let (p, r) = (a.pos, super::footing::walk_radius(id, a));
        let Some(i) = self.climbs.iter().position(|c| c.steep && p.y > c.foot.y + 0.3 && p.y < c.top.y + 0.2 && (Vec2::new(p.x, p.z) - c.line(p.y)).length() < r + 0.45) else { return false };
        self.climb_by(id, i, 0.0);
        true
    }

    /// A person on their way somewhere by a ladder: climb toward `p` (up or
    /// down to it), taking hold at its foot or top. None when no ladder
    /// is on the way.
    pub fn ladder_step(&mut self, who: ActorId, p: Vec3, step: f32) -> Option<bool> {
        let a = self.actor(who)?;
        let (me, r) = (a.pos, super::footing::walk_radius(who, a));
        let i = match a.ladder {
            Some(i) => i as usize,
            None => {
                let up = p.y > me.y + 1.0;
                let down = p.y < me.y - 1.0;
                if !up && !down {
                    return None;
                }
                let i = self.ladder_at(me, r, up)?;
                // Only a ladder that goes where it is going.
                if self.climbs[i].dist(p) > 1.8 {
                    return None;
                }
                i
            }
        };
        let c = self.climbs.get(i)?.clone();
        let dy = if p.y > me.y + 0.3 {
            step * 0.7
        } else if p.y < me.y - 0.3 {
            -step * 0.7
        } else if me.y - c.foot.y < c.top.y - me.y {
            -step
        } else {
            step
        };
        self.climb_by(who, i, dy);
        Some(true)
    }
}
