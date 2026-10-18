//! Landforms: the land's large shape, where a world says it (a real place:
//! the coast where it runs, the bay, the mountains at their distance and
//! height). Each is an outline or a line in world metres that makes the land
//! inside it one of the world's biomes (its ground, plants and hills), with
//! a soft border `edge` metres wide either side (half and half on the outline
//! itself), and may raise it by `rise` metres where it is wholly itself,
//! deeper inside than `edge` (a range's mass, over which the biome's own
//! hills stand). Later landforms lie over earlier ones, rise and all (the sea laid
//! over a range's foot takes it down to the sea bed). Everything else is the
//! seed's land as ever.
//!
//! They are baked once into a coarse map (`LandMap`, a few tens of metres a
//! sample) that the terrain reads everywhere, near or far, on the CPU here
//! and on the GPU (`lands_at` in `shaders/common.wgsl`, the twin: keep the
//! decoding and blending identical).

use crate::ground::poly_sdf;
use crate::terrain::{Biome, MAX_BIOMES, smoothstep};
use serde::{Deserialize, Serialize};

/// At most this many landforms, each of at most this many points.
pub const MAX_FORMS: usize = 24;
pub const MAX_POINTS: usize = 256;
/// The map is at most this many samples a side, and at least this fine.
const MAX_N: usize = 640;
const MIN_CELL: f32 = 48.0;
/// Rise is kept in tenths of a metre over this floor.
const RISE_FLOOR: f32 = -2000.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Landform {
    #[serde(default)]
    pub name: String,
    /// The biome it makes of the land, by name (or its number, from 0).
    #[serde(default)]
    pub biome: String,
    /// Inside an outline (x, z points)…
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outline: Vec<[f32; 2]>,
    /// …or along a line, `width` metres wide.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line: Vec<[f32; 2]>,
    #[serde(default)]
    pub width: f32,
    /// How wide its border is either side of the shape (m).
    #[serde(default = "d_edge")]
    pub edge: f32,
    /// How far it raises the land at its heart (m; below 0 it sinks it).
    #[serde(default)]
    pub rise: f32,
    /// How broken its rise is, 0 (a smooth mass) … 1 (ridges and canyons a
    /// kilometre or two apart, peaks higher than `rise`, canyon floors far
    /// lower): a mountain range's.
    #[serde(default)]
    pub rugged: f32,
}

fn d_edge() -> f32 {
    300.0
}

impl Landform {
    /// Signed distance to its shape (negative inside).
    fn dist(&self, x: f32, z: f32) -> f32 {
        if self.outline.len() >= 3 {
            return poly_sdf(&self.outline, x, z);
        }
        let half = self.width * 0.5;
        if self.line.len() == 1 {
            return ((x - self.line[0][0]).powi(2) + (z - self.line[0][1]).powi(2)).sqrt() - half;
        }
        let mut best = f32::MAX;
        for w in self.line.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (bx, bz) = (b[0] - a[0], b[1] - a[1]);
            let (px, pz) = (x - a[0], z - a[1]);
            let t = ((px * bx + pz * bz) / (bx * bx + bz * bz).max(1e-6)).clamp(0.0, 1.0);
            best = best.min((px - bx * t).powi(2) + (pz - bz * t).powi(2));
        }
        best.sqrt() - half
    }

    /// Its box (x0, z0, x1, z1), border included.
    fn bounds(&self) -> [f32; 4] {
        let pts = if self.outline.len() >= 3 { &self.outline } else { &self.line };
        let r = if self.outline.len() >= 3 { 0.0 } else { self.width * 0.5 } + self.edge;
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for p in pts {
            b = [b[0].min(p[0] - r), b[1].min(p[1] - r), b[2].max(p[0] + r), b[3].max(p[1] + r)];
        }
        b
    }

    /// Its rise at (x, z) as a share of `rise`, broken into ridges and canyons.
    fn broken_share(&self, x: f32, z: f32) -> f32 {
        if self.rugged <= 0.0 {
            return 1.0;
        }
        // Its own noise (from its name), so two ranges don't share their ridges.
        let s = self.name.bytes().fold(0x5eed_u32, |a, b| crate::noise::pcg(a ^ b as u32));
        let r = crate::noise::ridged2(x * 0.0006, z * 0.0006, s, 4);
        let n = crate::noise::fbm2(x * 0.00025, z * 0.00025, s.wrapping_add(1), 3);
        let shape = (0.15 + 1.35 * r * r + 0.35 * n).max(0.05);
        1.0 - self.rugged + self.rugged * shape
    }

    fn usable(&self) -> bool {
        let pts = if self.outline.len() >= 3 { &self.outline } else { &self.line };
        !pts.is_empty() && pts.iter().all(|p| p[0].is_finite() && p[1].is_finite() && p[0].abs() < 1e5 && p[1].abs() < 1e5) && (self.outline.len() >= 3 || self.width > 0.0)
    }
}

/// Keep landforms the map can hold (finite, not too many, nor too big).
pub fn sanitize(forms: &mut Vec<Landform>) {
    forms.truncate(MAX_FORMS);
    for f in forms.iter_mut() {
        f.outline.truncate(MAX_POINTS);
        f.line.truncate(MAX_POINTS);
        f.width = if f.width.is_finite() { f.width.clamp(0.0, 40_000.0) } else { 0.0 };
        f.edge = if f.edge.is_finite() { f.edge.clamp(10.0, 20_000.0) } else { d_edge() };
        f.rise = if f.rise.is_finite() { f.rise.clamp(-500.0, 2500.0) } else { 0.0 };
        f.rugged = if f.rugged.is_finite() { f.rugged.clamp(0.0, 1.0) } else { 0.0 };
    }
    forms.retain(|f| f.usable());
}

/// The landforms baked: per sample, how much of each biome they make of the
/// land (as bytes; their sum is how much they hold the land at all) and the
/// rise (tenths of a metre over `RISE_FLOOR`), two words a sample. Samples
/// lie on a square grid from `origin`, `cell` metres apart, `n` a side.
#[derive(Clone, Debug, PartialEq)]
pub struct LandMap {
    pub origin: [f32; 2],
    pub cell: f32,
    pub n: u32,
    pub words: Vec<u32>,
}

impl LandMap {
    /// None when there is nothing to bake.
    pub fn bake(forms: &[Landform], biomes: &[Biome]) -> Option<LandMap> {
        let forms: Vec<(&Landform, usize, [f32; 4])> = forms
            .iter()
            .filter(|f| f.usable())
            .filter_map(|f| {
                let b = f.biome.trim().to_lowercase();
                let i = biomes.iter().position(|x| x.name.trim().to_lowercase() == b).or_else(|| b.parse::<usize>().ok().filter(|i| *i < biomes.len()))?;
                Some((f, i.min(MAX_BIOMES - 1), f.bounds()))
            })
            .collect();
        if forms.is_empty() {
            return None;
        }
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for (_, _, fb) in &forms {
            b = [b[0].min(fb[0]), b[1].min(fb[1]), b[2].max(fb[2]), b[3].max(fb[3])];
        }
        let side = (b[2] - b[0]).max(b[3] - b[1]);
        let cell = (side / (MAX_N - 1) as f32).max(MIN_CELL).ceil();
        let n = ((side / cell).ceil() as usize + 2).min(MAX_N);
        let origin = [(b[0] / cell).floor() * cell, (b[1] / cell).floor() * cell];
        let mut words = vec![0u32; n * n * 2];
        let threads = std::thread::available_parallelism().map(|t| t.get()).unwrap_or(4).min(16);
        let rows = n.div_ceil(threads);
        std::thread::scope(|s| {
            for (k, chunk) in words.chunks_mut(rows * n * 2).enumerate() {
                let forms = &forms;
                s.spawn(move || {
                    for (r, row) in chunk.chunks_mut(n * 2).enumerate() {
                        let z = origin[1] + (k * rows + r) as f32 * cell;
                        for i in 0..n {
                            let x = origin[0] + i as f32 * cell;
                            let (mut t, mut rise) = ([0.0f32; MAX_BIOMES], 0.0f32);
                            for (f, bi, fb) in forms {
                                if x < fb[0] || x > fb[2] || z < fb[1] || z > fb[3] {
                                    continue;
                                }
                                let p = smoothstep(f.edge, -f.edge, f.dist(x, z));
                                if p <= 0.0 {
                                    continue;
                                }
                                for (j, tj) in t.iter_mut().enumerate() {
                                    *tj = *tj * (1.0 - p) + if j == *bi { p } else { 0.0 };
                                }
                                rise = rise * (1.0 - p) + f.rise * f.broken_share(x, z) * p;
                            }
                            let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
                            let rq = (((rise - RISE_FLOOR) * 10.0).round().clamp(0.0, 65535.0)) as u32;
                            row[i * 2] = q(t[0]) | q(t[1]) << 8 | q(t[2]) << 16 | q(t[3]) << 24;
                            row[i * 2 + 1] = q(t[4]) | q(t[5]) << 8 | rq << 16;
                        }
                    }
                });
            }
        });
        Some(LandMap { origin, cell, n: n as u32, words })
    }

    fn sample(&self, i: usize) -> ([f32; MAX_BIOMES], f32) {
        let (a, b) = (self.words[i * 2], self.words[i * 2 + 1]);
        let u = |w: u32, s: u32| ((w >> s) & 255) as f32 / 255.0;
        ([u(a, 0), u(a, 8), u(a, 16), u(a, 24), u(b, 0), u(b, 8)], (b >> 16) as f32 * 0.1 + RISE_FLOOR)
    }

    /// What the landforms make of the land at (x, z): how much of each
    /// biome (summing to how much they hold it) and the rise. None outside.
    pub fn at(&self, x: f32, z: f32) -> Option<([f32; MAX_BIOMES], f32)> {
        let n = self.n as usize;
        let gx = (x - self.origin[0]) / self.cell;
        let gz = (z - self.origin[1]) / self.cell;
        if !(gx >= 0.0 && gz >= 0.0 && gx < (n - 1) as f32 && gz < (n - 1) as f32) {
            return None;
        }
        let (ix, iz) = (gx as usize, gz as usize);
        let (fx, fz) = (gx - ix as f32, gz - iz as f32);
        let i0 = iz * n + ix;
        let (a, b, c, d) = (self.sample(i0), self.sample(i0 + 1), self.sample(i0 + n), self.sample(i0 + n + 1));
        let lerp = |p: f32, q: f32, t: f32| p + (q - p) * t;
        let bl = |p: f32, q: f32, r: f32, s: f32| lerp(lerp(p, q, fx), lerp(r, s, fx), fz);
        let mut w = [0.0; MAX_BIOMES];
        for (j, wj) in w.iter_mut().enumerate() {
            *wj = bl(a.0[j], b.0[j], c.0[j], d.0[j]);
        }
        Some((w, bl(a.1, b.1, c.1, d.1)))
    }

    /// For the GPU: origin x, z, cell, samples a side.
    pub fn globals(&self) -> [f32; 4] {
        [self.origin[0], self.origin[1], self.cell, self.n as f32]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn biomes() -> Vec<Biome> {
        crate::terrain::default_biomes()
    }

    #[test]
    fn inside_is_its_biome_outside_is_untouched() {
        let b = biomes();
        let sea = Landform { name: "the bay".into(), biome: b[1].name.clone(), outline: vec![[-4000.0, -4000.0], [0.0, -4000.0], [0.0, 4000.0], [-4000.0, 4000.0]], edge: 200.0, ..Default::default() };
        let m = LandMap::bake(&[sea], &b).unwrap();
        let (w, rise) = m.at(-2000.0, 0.0).unwrap();
        assert!((w[1] - 1.0).abs() < 0.01 && w.iter().sum::<f32>() < 1.01 && rise.abs() < 0.1, "{w:?} {rise}");
        let (w, _) = m.at(0.0, 0.0).unwrap();
        assert!((w[1] - 0.5).abs() < 0.08, "half at the outline: {w:?}");
        assert!(m.at(300.0, 0.0).is_none_or(|(w, _)| w.iter().sum::<f32>() < 0.01));
    }

    #[test]
    fn a_shaped_land_begins_at_its_start() {
        // As genesis gives it: the land, then the sea west of the start.
        let mut look: crate::world::Look = serde_json::from_str(
            r#"{"landforms": [
                {"name": "the plain", "biome": "0", "outline": [[-6000, -6000], [6000, -6000], [6000, 6000], [-6000, 6000]], "edge": 800},
                {"name": "the sea", "biome": "1", "outline": [[-400, -6000], [-400, 6000], [-9000, 6000], [-9000, -6000]], "edge": 150},
                {"name": "nonsense", "biome": "0", "outline": [[1, 2]]}
            ]}"#,
        )
        .unwrap();
        look.sanitize();
        assert_eq!(look.landforms.len(), 2, "an outline of two points is dropped");
        let mut b = biomes();
        b[1].base = -10.0;
        b[0].base = 8.0;
        b[0].amp = 3.0;
        let t = crate::terrain::Terrain::new(3, b).with_lands(&look.landforms);
        let s = crate::model::find_first_spawn(&t);
        assert!(s.x.abs() < 120.0 && s.z.abs() < 120.0, "begins at the start: {s}");
        assert!(t.height(s.x, s.z) > t.water(s.x, s.z), "on dry land");
        assert!(t.height(-1500.0, 0.0) < t.water(-1500.0, 0.0) - 5.0, "the sea lies west");
        assert!(t.height(2000.0, 2500.0) > 0.0, "the plain east");
    }

    #[test]
    fn a_range_rises_toward_its_line() {
        let b = biomes();
        let range = Landform { biome: "0".into(), line: vec![[0.0, -3000.0], [0.0, 3000.0]], width: 4000.0, edge: 1500.0, rise: 400.0, ..Default::default() };
        let m = LandMap::bake(&[range], &b).unwrap();
        let r = |x: f32| m.at(x, 0.0).map(|a| a.1).unwrap_or(0.0);
        assert!(r(0.0) > 398.0 && r(400.0) > 398.0 && r(1500.0) > r(2500.0) && r(2500.0) > r(3300.0) && r(3600.0) < 1.0, "{} {} {} {}", r(0.0), r(1500.0), r(2500.0), r(3300.0));
    }
}
