//! Pointing at things: which thing, person, placed object or scatter item a
//! ray from the eye meets first. Used for the mouse, the centre reticle and
//! `pocket inspect --at`.

use super::{ActorId, Sim, Target};
use crate::render::GpuInst;
use crate::world::TypeEntry;
use glam::Vec3;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Picked {
    pub target: Target,
    pub pos: Vec3,
    pub dist: f32,
    pub name: String,
}

/// Things smaller than this (bounding radius, m) get a halo of HALO metres.
const SMALL: f32 = 0.35;
const HALO: f32 = 0.22;

enum Cand {
    Shape(Target, GpuInst, Arc<TypeEntry>),
    Body(ActorId, Vec3, f32),
}

fn sdf(c: &Cand, p: Vec3) -> f32 {
    match c {
        Cand::Shape(_, g, ty) => {
            let ds = (p - g.center()).length() - g.radius();
            if ds > 0.4 {
                return ds;
            }
            g.sdf(&ty.ct, p)
        }
        Cand::Body(_, feet, h) => {
            let y = p.y.clamp(feet.y + 0.3, feet.y + h - 0.2);
            (p - Vec3::new(feet.x, y, feet.z)).length() - 0.3
        }
    }
}

/// Ray vs bounding sphere: does the ray come within the sphere before `max`?
fn ray_hits(ro: Vec3, rd: Vec3, c: Vec3, r: f32, max: f32) -> bool {
    let oc = ro - c;
    let b = oc.dot(rd);
    let cc = oc.dot(oc) - r * r;
    let disc = b * b - cc;
    disc >= 0.0 && -b + disc.sqrt() >= 0.0 && -b - disc.sqrt() <= max
}

impl Sim {
    /// The first thing a ray from `ro` along `rd` meets within `max` metres.
    /// `except` is never picked (the one looking).
    pub fn pick(&mut self, ro: Vec3, rd: Vec3, max: f32, except: Option<ActorId>) -> Option<Picked> {
        let rd = rd.normalize_or_zero();
        let snap = self.snap.clone();
        let mut cands: Vec<Cand> = Vec::new();
        let steps = (max / 6.0).ceil() as i32;
        let mut seen_things = std::collections::HashSet::new();
        let mut seen_inst = std::collections::HashSet::new();
        let mut seen_cells = std::collections::HashSet::new();
        for s in 0..=steps {
            let p = ro + rd * (s as f32 * 6.0);
            for id in self.things.near(p, 8.0) {
                if !seen_things.insert(id) {
                    continue;
                }
                let Some(t) = self.things.get(id) else { continue };
                if except.is_some_and(|e| t.holder == Some(e)) {
                    continue;
                }
                let Some(ty) = snap.type_of(t.type_id) else { continue };
                let g = super::render::thing_inst(t, ty, [0.0; 4]);
                if ray_hits(ro, rd, g.center(), g.radius() + HALO, max) {
                    cands.push(Cand::Shape(Target::Thing(id), g, ty.clone()));
                }
            }
            for pl in snap.near_chunk(crate::world::chunk_of(p.x, p.z)) {
                if self.things.by_instance.contains_key(&pl.id) || !seen_inst.insert(pl.id) {
                    continue;
                }
                let Some(ty) = snap.type_of(pl.type_id) else { continue };
                let g = pl.gpu(ty, 1.0);
                if ray_hits(ro, rd, g.center(), g.radius(), max) {
                    cands.push(Cand::Shape(Target::Instance(pl.id), g, ty.clone()));
                }
            }
            for it in self.cache.items_near(&snap, p, 8.0) {
                if !seen_cells.insert(it.cell) {
                    continue;
                }
                let Some(ty) = snap.type_of(it.inst.info[0]) else { continue };
                if ty.has_tag("grass") {
                    continue;
                }
                let g = it.inst;
                if ray_hits(ro, rd, g.center(), g.radius() + HALO, max) {
                    cands.push(Cand::Shape(Target::Cell([it.cell.0, it.cell.1]), g, ty.clone()));
                }
            }
        }
        for a in self.actor_ids() {
            if Some(a) == except || a == ActorId::Player {
                continue;
            }
            let Some(x) = self.actor(a) else { continue };
            let h = self.actor_height(a);
            if ray_hits(ro, rd, x.pos + Vec3::Y * h * 0.5, h * 0.6, max) {
                cands.push(Cand::Body(a, x.pos, h));
            }
        }
        let terrain = &snap.terrain;
        let mut t = 0.15;
        for _ in 0..220 {
            let p = ro + rd * t;
            let mut d = (p.y - terrain.height(p.x, p.z)) * 0.6;
            let mut hit: Option<usize> = None;
            for (i, c) in cands.iter().enumerate() {
                let mut dc = sdf(c, p);
                if let Cand::Shape(_, g, _) = c {
                    if g.radius() < SMALL {
                        // Small things are hard to point at: a halo around them counts.
                        dc = dc.min((p - g.center()).length() - g.radius() - HALO);
                    }
                }
                if dc < d {
                    d = dc;
                    hit = Some(i);
                }
            }
            if d < 0.01 + 0.0015 * t {
                return Some(match hit {
                    Some(i) => {
                        let (target, name) = match &cands[i] {
                            Cand::Shape(tg, _, ty) => (tg.clone(), ty.name().to_string()),
                            Cand::Body(a, _, _) => (Target::Actor(*a), self.actor_name(*a)),
                        };
                        Picked { target, pos: p, dist: t, name }
                    }
                    None => Picked { target: Target::Point(p.to_array()), pos: p, dist: t, name: "the ground".into() },
                });
            }
            t += d.max(0.02);
            if t > max {
                return None;
            }
        }
        None
    }
}

impl Sim {
    /// The nearest small loose thing within reach in front of someone (for
    /// picking things up without aiming).
    pub fn nearest_in_front(&mut self, who: ActorId) -> Option<Target> {
        let a = self.actor(who)?.clone();
        let reach = super::actor::REACH;
        let ok = |p: Vec3| {
            let d = Vec3::new(p.x - a.pos.x, 0.0, p.z - a.pos.z);
            d.length() <= reach && d.normalize_or_zero().dot(a.forward()) > 0.3
        };
        let mut best: Option<(f32, Target)> = None;
        for id in self.things.near(a.pos, reach + 0.5) {
            let Some(t) = self.things.get(id) else { continue };
            if t.held() || !t.liftable(1) || !ok(t.pos) {
                continue;
            }
            let d = (t.pos - a.pos).length();
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Thing(id)));
            }
        }
        let snap = self.snap.clone();
        for it in self.cache.items_near(&snap, a.pos, reach + 0.5) {
            let Some(ty) = snap.type_of(it.inst.info[0]) else { continue };
            if !ty.has_tag("small") || !ok(it.inst.pos()) {
                continue;
            }
            let d = (it.inst.pos() - a.pos).length();
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Cell([it.cell.0, it.cell.1])));
            }
        }
        best.map(|b| b.1)
    }
}
