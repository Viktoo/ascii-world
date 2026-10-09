//! `describe`: what is visible from a viewpoint, as JSON. Used by the CLI,
//! by Create mode ("that hill") and as dialogue context.

use super::scatter::ScatterCache;
use super::{Solid, WorldSnapshot, region_of};
use crate::render::{Camera, VIEW_DIST};
use crate::terrain::WATER_LEVEL;
use glam::Vec3;
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct Seen {
    pub name: String,
    pub kind: &'static str,
    pub distance: f32,
    pub third: &'static str,
    pub x: f32,
    pub z: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Target {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub distance: f32,
    pub terrain: &'static str,
    pub biome: String,
    pub slope_deg: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct View {
    pub at: [f32; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub biome: String,
    pub time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    pub visible: Vec<Seen>,
}

pub struct NpcView {
    pub id: i64,
    pub name: String,
    pub pos: Vec3,
}

/// Solids along a ray (deduplicated).
pub fn solids_along(snap: &WorldSnapshot, cache: &mut ScatterCache, ro: Vec3, rd: Vec3, tmax: f32) -> Vec<Solid> {
    let mut out: Vec<Solid> = Vec::new();
    let mut t = 0.0;
    while t <= tmax {
        for s in cache.solids_near(snap, ro + rd * t, 40.0) {
            let dup = out.iter().any(|o| o.inst.info[0] == s.inst.info[0] && o.inst.pos_scale == s.inst.pos_scale);
            if !dup {
                out.push(s);
            }
        }
        t += 40.0;
    }
    out
}

/// March terrain + solids. Returns (t, index of solid hit or None for terrain).
pub fn raycast(snap: &WorldSnapshot, solids: &[Solid], ro: Vec3, rd: Vec3, tmax: f32) -> Option<(f32, Option<usize>)> {
    let terrain = &snap.terrain;
    let mut t = 0.1;
    let mut tp = t;
    for _ in 0..300 {
        let p = ro + rd * t;
        let mut d = (p.y - terrain.height(p.x, p.z)) * 0.6;
        let mut hit = None;
        for (i, s) in solids.iter().enumerate() {
            let ds = s.sdf(p);
            if ds < d {
                d = ds;
                hit = Some(i);
            }
        }
        if d < 0.01 + 0.001 * t {
            if hit.is_none() {
                let (mut a, mut b) = (tp, t);
                for _ in 0..12 {
                    let m = (a + b) * 0.5;
                    let q = ro + rd * m;
                    if q.y > terrain.height(q.x, q.z) { a = m } else { b = m }
                }
                t = b;
            }
            return Some((t, hit));
        }
        tp = t;
        t += d.max(0.02);
        if t > tmax {
            return None;
        }
    }
    None
}

fn third(u: f32) -> &'static str {
    if u < -1.0 / 3.0 {
        "left"
    } else if u > 1.0 / 3.0 {
        "right"
    } else {
        "centre"
    }
}

fn terrain_kind(snap: &WorldSnapshot, p: Vec3) -> &'static str {
    let t = &snap.terrain;
    if p.y < WATER_LEVEL + 0.05 {
        return "water";
    }
    // A rock face (steeper than anyone climbs).
    if t.normal(p.x, p.z).y < (1.0 / (1.0 + crate::world::collide::MAX_CLIMB * crate::world::collide::MAX_CLIMB)).sqrt() {
        return "cliff";
    }
    let mut avg = 0.0;
    for i in 0..12 {
        let a = i as f32 / 12.0 * std::f32::consts::TAU;
        avg += t.height(p.x + a.cos() * 30.0, p.z + a.sin() * 30.0);
    }
    avg /= 12.0;
    let d = p.y - avg;
    if d > 5.0 {
        "hill"
    } else if d > 1.5 {
        "rise"
    } else if d < -4.0 {
        "hollow"
    } else if p.y < WATER_LEVEL + 1.5 {
        "shore"
    } else {
        "flat ground"
    }
}

fn visible_from(snap: &WorldSnapshot, eye: Vec3, p: Vec3) -> bool {
    let n = 24;
    for i in 1..n {
        let q = eye.lerp(p, i as f32 / n as f32);
        if snap.terrain.height(q.x, q.z) > q.y + 0.2 {
            return false;
        }
    }
    true
}

pub fn describe(snap: &WorldSnapshot, cache: &mut ScatterCache, npcs: &[NpcView], cam: &Camera, aspect: f32, t_game: f64) -> View {
    let (f, r, _) = cam.basis();
    let tan = (cam.fov_y * 0.5).tan();
    let mut seen: Vec<Seen> = Vec::new();
    let project = |c: Vec3| -> Option<(f32, f32)> {
        let v = c - cam.pos;
        let z = v.dot(f);
        if z <= 0.1 {
            return None;
        }
        let u = v.dot(r) / z / (tan * aspect);
        (u.abs() <= 1.1).then_some((u, v.length()))
    };
    // Story instances.
    for p in &snap.instances {
        if cache.overlay.hidden.contains(&p.id) {
            continue;
        }
        let Some(ty) = snap.type_of(p.type_id) else { continue };
        let top = p.pos + Vec3::Y * (ty.ct.meta.bounds[1] * p.scale * 0.5).min(4.0);
        let Some((u, d)) = project(p.pos) else { continue };
        if d > VIEW_DIST || !visible_from(snap, cam.pos, top) {
            continue;
        }
        seen.push(Seen { name: ty.name().to_string(), kind: "object", distance: round1(d), third: third(u), x: round1(p.pos.x), z: round1(p.pos.z), id: Some(p.id) });
    }
    // Nearby scatter (nearest few only).
    let mut scatter: Vec<Seen> = Vec::new();
    for s in cache.solids_near(snap, cam.pos, 50.0) {
        let Some((u, d)) = project(s.inst.pos()) else { continue };
        if d > 50.0 || !visible_from(snap, cam.pos, s.inst.pos() + Vec3::Y) {
            continue;
        }
        if snap.instances.iter().any(|p| p.pos == s.inst.pos()) {
            continue;
        }
        scatter.push(Seen { name: s.ty.name().to_string(), kind: "scenery", distance: round1(d), third: third(u), x: round1(s.inst.pos_scale[0]), z: round1(s.inst.pos_scale[2]), id: None });
    }
    scatter.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    scatter.truncate(12);
    seen.extend(scatter);
    for n in npcs {
        let head = n.pos + Vec3::Y * 1.6;
        let Some((u, d)) = project(head) else { continue };
        if d > 80.0 || !visible_from(snap, cam.pos, head) {
            continue;
        }
        seen.push(Seen { name: n.name.clone(), kind: "character", distance: round1(d), third: third(u), x: round1(n.pos.x), z: round1(n.pos.z), id: Some(n.id) });
    }
    seen.sort_by(|a, b| a.distance.total_cmp(&b.distance));

    let solids = solids_along(snap, cache, cam.pos, f, VIEW_DIST);
    let target = raycast(snap, &solids, cam.pos, f, VIEW_DIST).map(|(t, hit)| {
        let p = cam.pos + f * t;
        let n = snap.terrain.normal(p.x, p.z);
        let bi = snap.terrain.biome_at(p.x, p.z);
        Target {
            x: round1(p.x),
            y: round1(p.y),
            z: round1(p.z),
            distance: round1(t),
            terrain: terrain_kind(snap, p),
            biome: snap.terrain.biomes[bi].name.clone(),
            slope_deg: round1(n.y.clamp(-1.0, 1.0).acos().to_degrees()),
            object: hit.map(|i| solids[i].ty.name().to_string()),
        }
    });
    let here = snap.terrain.biome_at(cam.pos.x, cam.pos.z);
    View {
        at: [round1(cam.pos.x), round1(cam.pos.z), round1(cam.yaw.to_degrees().rem_euclid(360.0))],
        region: snap.region_name(region_of(cam.pos.x, cam.pos.z)).map(str::to_string),
        biome: snap.terrain.biomes[here].name.clone(),
        time: crate::render::sky::time_label(t_game).to_string(),
        target,
        visible: seen,
    }
}

fn round1(x: f32) -> f32 {
    (x * 10.0).round() / 10.0
}
