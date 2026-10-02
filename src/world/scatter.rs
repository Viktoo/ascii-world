//! Deterministic scatter (trees, rocks, bushes, grass…) per 64 m chunk, from
//! (seed, chunk). Never stored; story instances suppress scatter under them.

use super::{CHUNK, Solid, WorldSnapshot, chunk_of};
use crate::noise::{hseq, hseqf, u2f};
use crate::render::GpuInst;
use crate::terrain::WATER_LEVEL;
use glam::Vec3;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const CELL: f32 = 4.0;
const CELLS: i32 = (CHUNK / CELL) as i32;

#[derive(Clone)]
pub struct ScatterItem {
    pub inst: GpuInst,
    pub max_dist: f32,
    pub solid: bool,
}

pub fn max_dist_for(tags: &[String]) -> f32 {
    if tags.iter().any(|t| t == "grass") {
        24.0
    } else if tags.iter().any(|t| t == "bush" || t == "flower" || t == "small") {
        110.0
    } else {
        f32::MAX
    }
}

fn scale_range(tag: &str) -> (f32, f32) {
    match tag {
        "tree" | "pine" => (0.7, 1.3),
        "rock" => (0.5, 1.6),
        "grass" => (0.8, 1.3),
        _ => (0.75, 1.25),
    }
}

/// Generate one chunk's scatter.
pub fn generate(snap: &WorldSnapshot, cx: i32, cz: i32) -> Vec<ScatterItem> {
    let t = &snap.terrain;
    let seed = t.seed ^ 0x5CA7_7E12;
    let mut out = Vec::new();
    if snap.scatter.is_empty() {
        return out;
    }
    // Story footprints in and around this chunk suppress scatter.
    let blockers: Vec<(Vec3, f32)> = snap
        .near_chunk((cx, cz))
        .filter_map(|p| snap.type_of(p.type_id).map(|ty| (p.pos, ty.ct.meta.bounds[0].max(ty.ct.meta.bounds[2]) * p.scale + 1.5)))
        .collect();
    for j in 0..CELLS {
        for i in 0..CELLS {
            let gx = cx * CELLS + i;
            let gz = cz * CELLS + j;
            let x = (gx as f32 + 0.1 + 0.8 * hseqf(seed, gx, gz, 1)) * CELL;
            let z = (gz as f32 + 0.1 + 0.8 * hseqf(seed, gx, gz, 2)) * CELL;
            let w = t.weights(x, z);
            // Pick a biome in proportion to its weight, so borders blend.
            let mut r = hseqf(seed, gx, gz, 3);
            let mut bi = 0;
            for (k, wk) in w.iter().enumerate().take(t.biomes.len()) {
                if r < *wk {
                    bi = k;
                    break;
                }
                r -= *wk;
                bi = k;
            }
            let biome = &t.biomes[bi];
            let roll = hseqf(seed, gx, gz, 4);
            let mut acc = 0.0;
            let mut chosen: Option<&String> = None;
            let total: f32 = biome.scatter.values().map(|d| d * CELL * CELL / 100.0).sum();
            let norm = total.max(1.0);
            for (tag, dens) in &biome.scatter {
                acc += dens * CELL * CELL / 100.0 / norm;
                if roll < acc {
                    chosen = Some(tag);
                    break;
                }
            }
            let Some(tag) = chosen else { continue };
            let Some(cands) = snap.scatter.get(tag) else { continue };
            if cands.is_empty() {
                continue;
            }
            let tid = cands[(hseq(seed, gx, gz, 5) as usize) % cands.len()];
            let Some(ty) = snap.type_of(tid) else { continue };
            let h = t.height(x, z);
            if h < WATER_LEVEL + 0.35 {
                continue;
            }
            let big = tag != "grass";
            if big {
                let n = t.normal(x, z);
                if n.y < 0.78 {
                    continue;
                }
            }
            let pos2 = Vec3::new(x, 0.0, z);
            if (pos2 - Vec3::new(snap.spawn.x, 0.0, snap.spawn.z)).length() < 4.0 {
                continue;
            }
            if blockers.iter().any(|(c, r)| (Vec3::new(c.x, 0.0, c.z) - pos2).length() < *r) {
                continue;
            }
            let (s0, s1) = scale_range(tag);
            let scale = s0 + (s1 - s0) * hseqf(seed, gx, gz, 6);
            let rot = hseqf(seed, gx, gz, 7) * std::f32::consts::TAU;
            let sink = if ty.has_tag("rock") { 0.25 } else { 0.08 };
            let y = h - ty.bottom * scale - sink * scale;
            let pseed = (u2f(hseq(seed, gx, gz, 8)) * 97.0).floor();
            let k = [pseed, scale, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5];
            let inst = GpuInst {
                pos_scale: [x, y, z, scale],
                rot: [rot.cos(), rot.sin(), ty.radius() * scale, 1.0],
                k0: [k[0], k[1], k[2], k[3]],
                k1: [k[4], k[5], k[6], k[7]],
                info: ty.gpu_info(),
            };
            out.push(ScatterItem { inst, max_dist: max_dist_for(&ty.ct.meta.tags), solid: ty.solid });
        }
    }
    out
}

/// Per-chunk scatter cache, invalidated per chunk when its inputs change.
#[derive(Default)]
pub struct ScatterCache {
    chunks: HashMap<(i32, i32), (u64, Arc<Vec<ScatterItem>>)>,
}

fn signature(snap: &WorldSnapshot, c: (i32, i32)) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    snap.scatter_epoch.hash(&mut h);
    for p in snap.near_chunk(c) {
        p.id.hash(&mut h);
    }
    h.finish()
}

impl ScatterCache {
    pub fn get(&mut self, snap: &WorldSnapshot, c: (i32, i32)) -> Arc<Vec<ScatterItem>> {
        let sig = signature(snap, c);
        if let Some((s, v)) = self.chunks.get(&c) {
            if *s == sig {
                return v.clone();
            }
        }
        let v = Arc::new(generate(snap, c.0, c.1));
        self.chunks.insert(c, (sig, v.clone()));
        v
    }

    /// Drop chunks far from the player.
    pub fn trim(&mut self, center: Vec3, keep: f32) {
        let cc = chunk_of(center.x, center.z);
        let r = (keep / CHUNK).ceil() as i32 + 1;
        self.chunks.retain(|k, _| (k.0 - cc.0).abs() <= r && (k.1 - cc.1).abs() <= r);
    }

    /// Solid scatter + story instances whose bounds come within `radius` of `p`.
    pub fn solids_near(&mut self, snap: &WorldSnapshot, p: Vec3, radius: f32) -> Vec<Solid> {
        let mut out = Vec::new();
        let c = chunk_of(p.x, p.z);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let cc = (c.0 + dx, c.1 + dz);
                let items = self.get(snap, cc);
                for it in items.iter() {
                    if !it.solid {
                        continue;
                    }
                    if (it.inst.center() - p).length() - it.inst.radius() > radius {
                        continue;
                    }
                    if let Some(ty) = snap.type_of(it.inst.info[0]) {
                        out.push(Solid { inst: it.inst, ty: ty.clone() });
                    }
                }
                if let Some(v) = snap.by_chunk.get(&cc) {
                    for &i in v {
                        let pl = &snap.instances[i];
                        let Some(ty) = snap.type_of(pl.type_id) else { continue };
                        if !ty.solid {
                            continue;
                        }
                        let gi = pl.gpu(ty, 1.0);
                        if (gi.center() - p).length() - gi.radius() > radius {
                            continue;
                        }
                        out.push(Solid { inst: gi, ty: ty.clone() });
                    }
                }
            }
        }
        out
    }
}
