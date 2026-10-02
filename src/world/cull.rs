//! Per-frame CPU culling: view distance + frustum, nearest first, plus the
//! coarse grid when many instances are visible.

use super::scatter::ScatterCache;
use super::{CHUNK, WorldSnapshot, chunk_of};
use crate::render::{Camera, GRID_THRESHOLD, GpuInst, Grid, VIEW_DIST};
use glam::Vec3;
use std::collections::HashMap;

pub const MAX_VISIBLE: usize = 1500;
/// Instances this close are kept even off-screen (they cast visible shadows).
const SHADOW_KEEP: f32 = 26.0;

pub struct Frustum {
    pos: Vec3,
    fwd: Vec3,
    cos_half: f32,
    sin_half: f32,
}

impl Frustum {
    pub fn new(cam: &Camera, aspect: f32) -> Frustum {
        let ty = (cam.fov_y * 0.5).tan();
        let tx = ty * aspect;
        let half = (tx * tx + ty * ty).sqrt().atan().min(1.5);
        Frustum { pos: cam.pos, fwd: cam.forward(), cos_half: half.cos(), sin_half: half.sin() }
    }

    /// Sphere vs. view cone (conservative).
    pub fn sees(&self, c: Vec3, r: f32) -> bool {
        let v = c - self.pos;
        let d = v.length();
        if d <= r {
            return true;
        }
        let along = v.dot(self.fwd);
        let perp = (d * d - along * along).max(0.0).sqrt();
        // signed distance from the sphere centre to the cone surface
        perp * self.cos_half - along * self.sin_half < r
    }
}

pub struct Culled {
    pub insts: Vec<GpuInst>,
    pub grid: Option<Grid>,
}

/// `fade` maps story instance id → fade (0..1) for recently appeared ones.
pub fn cull(snap: &WorldSnapshot, cache: &mut ScatterCache, cam: &Camera, aspect: f32, dynamic: &[GpuInst], fade: &HashMap<i64, f32>) -> Culled {
    let fr = Frustum::new(cam, aspect);
    let mut list: Vec<(f32, GpuInst)> = Vec::with_capacity(512);
    let push = |inst: GpuInst, max_dist: f32, list: &mut Vec<(f32, GpuInst)>| {
        let c = inst.center();
        let r = inst.radius();
        let d = (c - cam.pos).length();
        if d - r > VIEW_DIST.min(max_dist) {
            return;
        }
        if d - r > SHADOW_KEEP && !fr.sees(c, r) {
            return;
        }
        list.push((d, inst));
    };
    let cc = chunk_of(cam.pos.x, cam.pos.z);
    let n = (VIEW_DIST / CHUNK).ceil() as i32;
    for dz in -n..=n {
        for dx in -n..=n {
            let c = (cc.0 + dx, cc.1 + dz);
            // chunk-level distance reject
            let cx = (c.0 as f32 + 0.5) * CHUNK;
            let cz = (c.1 as f32 + 0.5) * CHUNK;
            let dist = ((cx - cam.pos.x).powi(2) + (cz - cam.pos.z).powi(2)).sqrt();
            if dist - CHUNK * 0.75 > VIEW_DIST + 20.0 {
                continue;
            }
            let items = cache.get(snap, c);
            for it in items.iter() {
                if !cache.overlay.shows(it.cell) {
                    continue;
                }
                let mut inst = it.inst;
                if let Some(fx) = cache.overlay.cell_fx.get(&it.cell) {
                    inst.fx = *fx;
                }
                push(inst, it.max_dist, &mut list);
            }
            if let Some(v) = snap.by_chunk.get(&c) {
                for &i in v {
                    let p = &snap.instances[i];
                    if cache.overlay.hidden.contains(&p.id) {
                        continue;
                    }
                    let Some(ty) = snap.type_of(p.type_id) else { continue };
                    let f = fade.get(&p.id).copied().unwrap_or(1.0);
                    if f <= 0.0 {
                        continue;
                    }
                    push(p.gpu(ty, f), f32::MAX, &mut list);
                }
            }
        }
    }
    for d in dynamic {
        push(*d, f32::MAX, &mut list);
    }
    list.sort_by(|a, b| a.0.total_cmp(&b.0));
    list.truncate(MAX_VISIBLE);
    let insts: Vec<GpuInst> = list.into_iter().map(|x| x.1).collect();
    let grid = (insts.len() > GRID_THRESHOLD).then(|| Grid::build(&insts, cam.pos, VIEW_DIST + 8.0));
    Culled { insts, grid }
}
