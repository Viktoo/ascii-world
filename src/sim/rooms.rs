//! Insides: placed shapes with doorways (from their anchors) are places a
//! body can be in. Walking between inside and out goes through a doorway,
//! between floors by the stairs; a shut door on the way is opened by anyone
//! who can work it. Nothing here knows what a house is: a cave with a
//! doorway, a tree-house with a ladder and a burrow are the same.

use super::{ActorId, Sim};
use crate::render::GpuInst;
use crate::world::WorldSnapshot;
use glam::Vec3;

/// One enterable shape.
#[derive(Clone, Debug)]
pub struct Room {
    pub g: GpuInst,
    /// Local half-extents (its bounds).
    pub half: Vec3,
    /// Each doorway: a step outside it, and a step inside.
    pub doors: Vec<(Vec3, Vec3)>,
    /// Each stair or ladder: foot and top.
    pub stairs: Vec<(Vec3, Vec3)>,
    pub beds: Vec<Vec3>,
    pub seats: Vec<Vec3>,
}

impl Room {
    /// Whether a point is inside it.
    pub fn holds(&self, p: Vec3) -> bool {
        let l = Vec3::from(self.g.to_local(p));
        l.x.abs() < self.half.x * 0.95 && l.z.abs() < self.half.z * 0.95 && l.y > -self.half.y - 0.5 && l.y < self.half.y
    }
}

/// The enterable shapes of a world version.
pub fn rooms_of(snap: &WorldSnapshot) -> Vec<Room> {
    let mut out = Vec::new();
    for p in &snap.instances {
        let Some(ty) = snap.type_of(p.type_id) else { continue };
        let anchors = &ty.ct.meta.anchors;
        if !anchors.iter().any(|a| a.kind == "door") {
            continue;
        }
        let g = p.gpu(ty, 1.0);
        let s = p.scale.max(0.1);
        let pt = |v: [f32; 3]| g.from_local(Vec3::from_array(v));
        let mut r = Room { g, half: Vec3::from_array(ty.ct.meta.bounds), doors: vec![], stairs: vec![], beds: vec![], seats: vec![] };
        for a in anchors {
            match a.kind.as_str() {
                "door" => {
                    let at = Vec3::from_array(a.at);
                    let out_dir = Vec3::from_array(a.out());
                    r.doors.push((g.from_local(at + out_dir * (1.0 / s)), g.from_local(at - out_dir * (1.0 / s))));
                }
                "stairs" => {
                    if let Some(to) = a.to {
                        r.stairs.push((pt(a.at), pt(to)));
                    }
                }
                "bed" => r.beds.push(pt(a.at)),
                "seat" => r.seats.push(pt(a.at)),
                _ => {}
            }
        }
        out.push(r);
    }
    out
}

const NEAR: f32 = 0.7;

impl Sim {
    /// The room a point is in, if any.
    pub fn room_at(&self, p: Vec3) -> Option<usize> {
        self.rooms.iter().position(|r| r.holds(p))
    }

    /// Where to walk next on the way to `dest`: through a doorway in or out,
    /// up or down the stairs, or straight there.
    pub fn route(&self, who: ActorId, dest: Vec3) -> Vec3 {
        let Some(me) = self.actor(who).map(|a| a.pos) else { return dest };
        if self.rooms.is_empty() {
            return dest;
        }
        let here = self.room_at(me);
        let there = self.room_at(dest);
        let flat = |a: Vec3, b: Vec3| Vec3::new(a.x - b.x, 0.0, a.z - b.z).length();
        // Between floors of one room: by the stairs.
        let by_stairs = |r: &Room, from: Vec3, to: Vec3| -> Option<Vec3> {
            let (foot, top) = r.stairs.iter().min_by(|a, b| flat(a.0, from).total_cmp(&flat(b.0, from)))?;
            if to.y > from.y + 1.2 {
                Some(if flat(from, *foot) > NEAR && from.y < foot.y + 0.6 { *foot } else { *top })
            } else if to.y < from.y - 1.2 {
                Some(if flat(from, *top) > NEAR && from.y > top.y - 0.6 { *top } else { *foot })
            } else {
                None
            }
        };
        match (here, there) {
            (Some(a), Some(b)) if a == b => by_stairs(&self.rooms[a], me, dest).unwrap_or(dest),
            (Some(a), _) => {
                let r = &self.rooms[a];
                let Some((out, inn)) = r.doors.iter().min_by(|x, y| flat(x.1, me).total_cmp(&flat(y.1, me))).copied() else { return dest };
                // Down to the doorway's floor first.
                if let Some(s) = by_stairs(r, me, inn) {
                    return s;
                }
                if flat(me, inn) > NEAR && flat(me, out) > flat(inn, out) { inn } else { out }
            }
            (None, Some(b)) => {
                let r = &self.rooms[b];
                let Some((out, inn)) = r.doors.iter().min_by(|x, y| flat(x.0, me).total_cmp(&flat(y.0, me))).copied() else { return dest };
                if flat(me, out) > NEAR && flat(me, inn) > flat(out, inn) { out } else { inn }
            }
            (None, None) => dest,
        }
    }

    /// A bed for a character in the house by their home, if there is one.
    pub fn bed_for(&self, cid: i64) -> Option<Vec3> {
        let home = self.cast.get(cid)?.def.home;
        let r = self.rooms.iter().filter(|r| !r.beds.is_empty()).min_by(|a, b| (a.g.pos() - home).length().total_cmp(&(b.g.pos() - home).length()))?;
        if (r.g.pos() - home).length() > r.half.x.max(r.half.z) * r.g.pos_scale[3] + 12.0 {
            return None;
        }
        Some(r.beds[(cid.unsigned_abs() as usize) % r.beds.len()])
    }

    /// Whether a body of its full height would stand clear at `p` (room to stand up).
    pub fn room_to_stand(&mut self, who: ActorId, p: Vec3) -> bool {
        let Some(h) = self.actor(who).map(|a| a.dims.height) else { return true };
        let mut cap = self.capsule(who);
        cap.height = h;
        let solids = self.solids_near(p, cap.radius + h + 1.0);
        let obs = crate::world::collide::Obstacles { solids: &solids, bodies: &[] };
        obs.dist_body(p.x, p.z, p.y, cap) >= cap.radius * 0.9
    }

    /// Whether a crouched body would get through at `p` where a standing one can't.
    pub fn crouch_fits(&mut self, who: ActorId, p: Vec3) -> bool {
        let Some(h) = self.actor(who).map(|a| a.dims.height) else { return false };
        let mut cap = self.capsule(who);
        let solids = self.solids_near(p, cap.radius + h + 1.0);
        let obs = crate::world::collide::Obstacles { solids: &solids, bodies: &[] };
        cap.height = h;
        let standing = obs.dist_body(p.x, p.z, p.y, cap);
        cap.height = h * (1.0 - super::actor::Actor::CROUCH_DROP);
        let low = obs.dist_body(p.x, p.z, p.y, cap);
        standing < cap.radius && low >= cap.radius
    }
}
