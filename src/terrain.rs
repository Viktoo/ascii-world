//! Procedural base layer: terrain height, biomes, ground colour, water.
//!
//! Mirrors `terrain_*` in `shaders/common.wgsl`. Nothing here is stored;
//! everything is a pure function of the universe seed and the biome table.

use crate::noise::{fbm2, lerp, ridged2, vnoise2};
use serde::{Deserialize, Serialize};

pub const MAX_BIOMES: usize = 6;
pub const BIOME_SHARPNESS: f32 = 26.0;
pub const WATER_LEVEL: f32 = 0.0;

/// Climate centres biomes are pinned to, in order. Spreading them over the
/// (temperature, moisture) square guarantees every biome actually occurs.
const CENTERS: [[f32; 2]; MAX_BIOMES] = [
    [0.5, 0.5],
    [0.15, 0.2],
    [0.85, 0.8],
    [0.2, 0.85],
    [0.8, 0.15],
    [0.5, 0.05],
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Biome {
    pub name: String,
    /// Mean ground height in metres (negative values make lakes and coasts).
    #[serde(default)]
    pub base: f32,
    /// Height amplitude in metres.
    #[serde(default = "d_amp")]
    pub amp: f32,
    /// 0 = rolling, 1 = craggy ridges.
    #[serde(default = "d_rough")]
    pub rough: f32,
    pub ground: [u8; 3],
    pub ground2: [u8; 3],
    /// Scatter density per tag, items per 100 m².
    #[serde(default)]
    pub scatter: std::collections::BTreeMap<String, f32>,
    /// How much of its relief stands as cliffs: 0 none … 1 wherever it has
    /// hills, the land steps up in ledges with bare rock faces between. Flat
    /// land stays flat: cliffs need hills (see `Terrain::cliff`).
    #[serde(default)]
    pub cliffs: f32,
    /// How strongly a tag grows in patches, 0 (spread evenly) … 1 (only in
    /// patches: groves and clearings, flower fields, boulder fields). Tags
    /// not named here use their kind's own (see `world::scatter::clump_of`).
    #[serde(default)]
    pub clump: std::collections::BTreeMap<String, f32>,
}

fn d_amp() -> f32 {
    14.0
}
fn d_rough() -> f32 {
    0.3
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Palette {
    pub sky_day: [u8; 3],
    pub horizon_day: [u8; 3],
    pub sky_dusk: [u8; 3],
    pub horizon_dusk: [u8; 3],
    pub sky_night: [u8; 3],
    pub horizon_night: [u8; 3],
    pub sun: [u8; 3],
    pub water: [u8; 3],
    pub rock: [u8; 3],
    pub snow: [u8; 3],
    pub sand: [u8; 3],
    /// Fog density multiplier (1 = clear, 3 = thick fog).
    #[serde(default = "d_fog")]
    pub fog: f32,
}

fn d_fog() -> f32 {
    1.0
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            sky_day: [70, 130, 210],
            horizon_day: [185, 210, 230],
            sky_dusk: [60, 60, 120],
            horizon_dusk: [240, 140, 90],
            sky_night: [6, 8, 22],
            horizon_night: [22, 28, 52],
            sun: [255, 244, 220],
            water: [38, 92, 120],
            rock: [118, 112, 104],
            snow: [236, 240, 245],
            sand: [200, 186, 140],
            fog: 1.0,
        }
    }
}

/// Small loose things on the ground (sticks, stones, mushrooms) that anyone
/// can pick up. Added to every biome that does not choose its own.
pub fn add_litter(biomes: &mut [Biome]) {
    for b in biomes {
        let wooded = ["tree", "pine", "bush"].iter().any(|t| b.scatter.get(*t).is_some_and(|d| *d > 0.0));
        b.scatter.entry("stick".into()).or_insert(if wooded { 0.12 } else { 0.03 });
        b.scatter.entry("stone".into()).or_insert(0.08);
        if wooded {
            b.scatter.entry("mushroom".into()).or_insert(0.05);
        }
    }
}

pub fn default_biomes() -> Vec<Biome> {
    let mut b = base_biomes();
    add_litter(&mut b);
    b
}

fn base_biomes() -> Vec<Biome> {
    let s = |pairs: &[(&str, f32)]| pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    vec![
        Biome {
            name: "meadow".into(),
            base: 4.0,
            amp: 10.0,
            rough: 0.15,
            ground: [96, 142, 64],
            ground2: [128, 160, 72],
            scatter: s(&[("tree", 0.25), ("bush", 0.5), ("rock", 0.15), ("grass", 3.0), ("flower", 0.9), ("tallgrass", 0.8)]),
            cliffs: 0.0,
            clump: Default::default(),
        },
        Biome {
            name: "pine forest".into(),
            base: 8.0,
            amp: 18.0,
            rough: 0.35,
            ground: [62, 96, 52],
            ground2: [84, 104, 58],
            scatter: s(&[("pine", 1.4), ("tree", 0.3), ("rock", 0.2), ("bush", 0.3), ("grass", 1.0)]),
            cliffs: 0.15,
            clump: Default::default(),
        },
        Biome {
            name: "highlands".into(),
            base: 18.0,
            amp: 34.0,
            rough: 0.75,
            ground: [110, 116, 84],
            ground2: [132, 124, 98],
            scatter: s(&[("rock", 0.6), ("pine", 0.2), ("grass", 0.8), ("tallgrass", 0.4)]),
            cliffs: 0.7,
            clump: Default::default(),
        },
        Biome {
            name: "lakeshore".into(),
            base: -5.0,
            amp: 9.0,
            rough: 0.1,
            ground: [104, 140, 82],
            ground2: [168, 160, 112],
            scatter: s(&[("bush", 0.4), ("rock", 0.25), ("grass", 2.0), ("tree", 0.1), ("tallgrass", 1.2)]),
            cliffs: 0.0,
            clump: Default::default(),
        },
        Biome {
            name: "heath".into(),
            base: 6.0,
            amp: 12.0,
            rough: 0.25,
            ground: [128, 112, 74],
            ground2: [112, 92, 96],
            scatter: s(&[("bush", 0.9), ("rock", 0.3), ("grass", 1.2), ("flower", 0.4)]),
            cliffs: 0.3,
            clump: Default::default(),
        },
    ]
}

/// Everything the terrain function needs. Uploaded verbatim to the GPU.
#[derive(Clone, Debug)]
pub struct Terrain {
    pub seed: u32,
    pub off: [f32; 6],
    pub biomes: Vec<Biome>,
    /// Ground taken away: cellars, burrows, houses set into hills, pits dug.
    /// Shared by every copy of this terrain (the world model's and each
    /// snapshot's), so a pit dug shows everywhere at once.
    pub carve: std::sync::Arc<parking_lot::RwLock<Carve>>,
}

/// A box (or round pit) of ground taken away, down to `floor`.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hollow {
    /// Centre (x, z) and the box's turn (cos, sin of its yaw).
    pub c: [f32; 2],
    pub rot: [f32; 2],
    /// Half-size (x, z) in its own frame; for a round pit, half[0] is the radius.
    pub half: [f32; 2],
    pub floor: f32,
    pub round: bool,
}

/// How far a hollow's sides slope out (m): its rim.
pub const HOLLOW_EDGE: f32 = 0.35;

impl Hollow {
    /// How far (x, z) lies outside the hollow (negative inside).
    pub fn outside(&self, x: f32, z: f32) -> f32 {
        let (dx, dz) = (x - self.c[0], z - self.c[1]);
        if self.round {
            return (dx * dx + dz * dz).sqrt() - self.half[0];
        }
        let (c, s) = (self.rot[0], self.rot[1]);
        let lx = c * dx - s * dz;
        let lz = s * dx + c * dz;
        (lx.abs() - self.half[0]).max(lz.abs() - self.half[1])
    }

    /// The bounding radius, rim included.
    pub fn reach(&self) -> f32 {
        if self.round { self.half[0] + HOLLOW_EDGE } else { (self.half[0] * self.half[0] + self.half[1] * self.half[1]).sqrt() + HOLLOW_EDGE }
    }
}

/// All the ground taken away.
#[derive(Clone, Debug, Default)]
pub struct Carve {
    /// Under placed shapes (from their `meta.hollow`), rebuilt with each snapshot.
    pub fixed: Vec<Hollow>,
    /// Dug by someone (live, saved with the live world).
    pub dug: Vec<Hollow>,
    /// Roads and paths worn into the ground between buildings.
    pub roads: Vec<Road>,
    /// Changes whenever any list does.
    pub version: u32,
    /// Hollows by 16 m cell (indices into `fixed` then `dug`); see `reindex`.
    index: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

const CARVE_CELL: f32 = 16.0;

/// A stretch of road: from a to b (x, z), half as wide as `half` × 2, in a colour.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Road {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub half: f32,
    pub color: [f32; 3],
}

impl Road {
    /// Distance from (x, z) to the road's middle line.
    pub fn dist(&self, x: f32, z: f32) -> f32 {
        let (ax, az) = (self.a[0], self.a[1]);
        let (bx, bz) = (self.b[0] - ax, self.b[1] - az);
        let (px, pz) = (x - ax, z - az);
        let l2 = (bx * bx + bz * bz).max(1e-6);
        let t = ((px * bx + pz * bz) / l2).clamp(0.0, 1.0);
        ((px - bx * t).powi(2) + (pz - bz * t).powi(2)).sqrt()
    }

    /// How much of the road shows at (x, z): 1 on it, fading at its edges.
    pub fn cover(&self, x: f32, z: f32) -> f32 {
        smoothstep(self.half + 0.4, self.half - 0.2, self.dist(x, z))
    }
}

impl Carve {
    pub fn all(&self) -> impl Iterator<Item = &Hollow> {
        self.fixed.iter().chain(self.dug.iter())
    }

    /// Rebuild the cell index after the lists change.
    pub fn reindex(&mut self) {
        self.index.clear();
        let n = self.fixed.len();
        for (i, h) in self.fixed.iter().chain(self.dug.iter()).enumerate() {
            let r = h.reach();
            let lo = (((h.c[0] - r) / CARVE_CELL).floor() as i32, ((h.c[1] - r) / CARVE_CELL).floor() as i32);
            let hi = (((h.c[0] + r) / CARVE_CELL).floor() as i32, ((h.c[1] + r) / CARVE_CELL).floor() as i32);
            for cz in lo.1..=hi.1 {
                for cx in lo.0..=hi.0 {
                    self.index.entry((cx, cz)).or_default().push(i as u32);
                }
            }
        }
        let _ = n;
        self.version = self.version.wrapping_add(1);
    }

    fn get(&self, i: u32) -> Option<&Hollow> {
        let i = i as usize;
        if i < self.fixed.len() { self.fixed.get(i) } else { self.dug.get(i - self.fixed.len()) }
    }

    /// The ground height at (x, z) with the hollows taken out of `h`, and how
    /// much of a hollow the point is in (0 … 1, for the earth's colour).
    pub fn apply(&self, x: f32, z: f32, h: f32) -> (f32, f32) {
        let mut out = h;
        let mut w_max = 0.0f32;
        let Some(ids) = self.index.get(&((x / CARVE_CELL).floor() as i32, (z / CARVE_CELL).floor() as i32)) else { return (h, 0.0) };
        for hl in ids.iter().filter_map(|i| self.get(*i)) {
            let r = hl.reach();
            let (dx, dz) = (x - hl.c[0], z - hl.c[1]);
            if dx * dx + dz * dz > r * r {
                continue;
            }
            let e = hl.outside(x, z);
            if e >= HOLLOW_EDGE {
                continue;
            }
            let w = smoothstep(HOLLOW_EDGE, 0.0, e);
            if h > hl.floor {
                out = out.min(h - w * (h - hl.floor));
            }
            w_max = w_max.max(w);
        }
        (out, w_max)
    }
}

pub struct Sample {
    pub height: f32,
}

/// Where a point lies on the land's cliffs (see `Terrain::cliff`).
#[derive(Clone, Copy, Debug)]
pub struct Cliff {
    /// The height with the cliffs in.
    pub h: f32,
    /// How much cliff there is here (0 … 1).
    pub k: f32,
    /// How far up its step the point is (0 … 1; the face is the top `w`).
    pub t: f32,
    /// How much of a step its face takes here (small: sheer; 1: a gully).
    pub w: f32,
}

/// The share of a step a sheer face rises in (the rest is ledge).
pub const CLIFF_FACE: f32 = 0.12;
/// How much of the land's own slope a ledge keeps.
pub const CLIFF_TILT: f32 = 0.2;

impl Terrain {
    pub fn new(seed: u32, biomes: Vec<Biome>) -> Self {
        let mut biomes = biomes;
        biomes.truncate(MAX_BIOMES);
        if biomes.is_empty() {
            biomes = default_biomes();
        }
        for b in &mut biomes {
            b.base = b.base.clamp(-12.0, 40.0);
            b.amp = b.amp.clamp(1.0, 60.0);
            b.rough = b.rough.clamp(0.0, 1.0);
            b.cliffs = if b.cliffs.is_finite() { b.cliffs.clamp(0.0, 1.0) } else { 0.0 };
            for c in b.clump.values_mut() {
                *c = if c.is_finite() { c.clamp(0.0, 1.0) } else { 0.5 };
            }
        }
        // Offsets stay zero: per-field seeds decorrelate the noise, and any offset
        // would cost precision (and CPU/GPU parity) far from the origin.
        let o = |_i: u32| 0.0f32;
        Terrain { seed, off: [o(1), o(2), o(3), o(4), o(5), o(6)], biomes, carve: Default::default() }
    }

    pub fn center(i: usize) -> [f32; 2] {
        CENTERS[i]
    }

    pub fn weights(&self, x: f32, z: f32) -> [f32; MAX_BIOMES] {
        let t = vnoise2(x * 0.0011 + self.off[0], z * 0.0011 + self.off[1], self.seed);
        let m = vnoise2(x * 0.0011 + self.off[2], z * 0.0011 + self.off[3], self.seed.wrapping_add(17));
        let t = ((t - 0.5) * 2.4 + 0.5).clamp(0.0, 1.0);
        let m = ((m - 0.5) * 2.4 + 0.5).clamp(0.0, 1.0);
        let mut w = [0.0f32; MAX_BIOMES];
        let mut sum = 1e-6f32;
        for (i, wi) in w.iter_mut().enumerate().take(self.biomes.len()) {
            let dt = t - CENTERS[i][0];
            let dm = m - CENTERS[i][1];
            *wi = (-(dt * dt + dm * dm) * BIOME_SHARPNESS).exp();
            sum += *wi;
        }
        for wi in w.iter_mut() {
            *wi /= sum;
        }
        w
    }

    pub fn sample(&self, x: f32, z: f32) -> Sample {
        let w = self.weights(x, z);
        let mut base = 0.0;
        let mut amp = 0.0;
        let mut rough = 0.0;
        let mut cliffs = 0.0;
        for (i, b) in self.biomes.iter().enumerate() {
            base += w[i] * b.base;
            amp += w[i] * b.amp;
            rough += w[i] * b.rough;
            cliffs += w[i] * b.cliffs;
        }
        let n = fbm2(x * 0.0045 + self.off[4], z * 0.0045 + self.off[5], self.seed.wrapping_add(101), 5);
        let r = ridged2(x * 0.0032 + self.off[5], z * 0.0032 + self.off[4], self.seed.wrapping_add(202), 3);
        let height = base + amp * lerp(n, r * 2.0 - 0.6, rough);
        Sample { height: self.cliff(x, z, height, cliffs, amp).h }
    }

    /// Steps the rolling height `h` into ledges and rock faces where the land
    /// has cliffs (`cliffs`, the biomes' blend). Twin of `cliff_h` in
    /// `shaders/common.wgsl`; keep the operation order identical.
    ///
    /// The height is cut into steps 5–12 m tall (varying across the land);
    /// within a step the ground keeps a fifth of its slope as a ledge, then
    /// rises the rest of the way in a face. Where the land is flat a step is
    /// no taller than the land already rose, so faces only stand where there
    /// were hills, and low land (`amp` under 12 m) has fewer cliffs, none
    /// below 3 m. Cliffs come in stretches (`k`), not everywhere a biome
    /// allows, and in places a face slumps into a steep, walkable gully
    /// (`w`), so the ledges above can be reached. Where the steps are cut
    /// wanders a little (`j`) so rims aren't contour lines, but only in hilly
    /// land (by `amp`): on a plain that wander alone would raise a face.
    /// Continuous everywhere: at a step's top `g - t` is 0 on both sides.
    pub fn cliff(&self, x: f32, z: f32, h: f32, cliffs: f32, amp: f32) -> Cliff {
        let none = Cliff { h, k: 0.0, t: 0.0, w: 1.0 };
        if cliffs < 0.01 {
            return none;
        }
        let s = self.seed;
        let k = smoothstep(1.0 - cliffs, 1.3 - cliffs, vnoise2(x * 0.004, z * 0.004, s.wrapping_add(404))) * smoothstep(3.0, 12.0, amp);
        if k <= 0.0 {
            return none;
        }
        let st = 5.0 + 7.0 * vnoise2(x * 0.0023, z * 0.0023, s.wrapping_add(405));
        let j = (0.3 * vnoise2(x * 0.02, z * 0.02, s.wrapping_add(406)) + 0.08 * vnoise2(x * 0.12, z * 0.12, s.wrapping_add(408))) * smoothstep(4.0, 20.0, amp);
        let w = lerp(CLIFF_FACE, 1.0, smoothstep(0.6, 0.75, vnoise2(x * 0.03, z * 0.03, s.wrapping_add(407))));
        // Buttresses and clefts: the face's foot wanders in and out along it.
        let wf = (w * (0.55 + 0.9 * vnoise2(x * 0.2, z * 0.2, s.wrapping_add(409)))).min(1.0);
        let q = h / st + j;
        let t = q - q.floor();
        let g = CLIFF_TILT * t + (1.0 - CLIFF_TILT) * smoothstep(1.0 - wf, 1.0, t);
        Cliff { h: h + k * (g - t) * st, k, t, w: wf }
    }

    /// The ground's height at (x, z) (as `height`) and how much it lies at
    /// the foot of a rock face (0 … 1): where what falls off cliffs gathers
    /// (boulders, stones).
    pub fn height_and_foot(&self, x: f32, z: f32) -> (f32, f32) {
        let w = self.weights(x, z);
        let (mut base, mut amp, mut rough, mut cliffs) = (0.0, 0.0, 0.0, 0.0);
        for (i, b) in self.biomes.iter().enumerate() {
            base += w[i] * b.base;
            amp += w[i] * b.amp;
            rough += w[i] * b.rough;
            cliffs += w[i] * b.cliffs;
        }
        let n = fbm2(x * 0.0045 + self.off[4], z * 0.0045 + self.off[5], self.seed.wrapping_add(101), 5);
        let r = ridged2(x * 0.0032 + self.off[5], z * 0.0032 + self.off[4], self.seed.wrapping_add(202), 3);
        let c = self.cliff(x, z, base + amp * lerp(n, r * 2.0 - 0.6, rough), cliffs, amp);
        let h = {
            let carve = self.carve.read();
            if carve.index.is_empty() { c.h } else { carve.apply(x, z, c.h).0 }
        };
        // On the ledge just below a sheer face (a gully sheds nothing).
        let below = (1.0 - c.w) - c.t;
        let near = if below >= -0.02 { 1.0 - smoothstep(0.0, 0.08, below) } else { 0.0 };
        (h, c.k * near * (1.0 - smoothstep(0.3, 0.6, c.w)))
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        let h = self.sample(x, z).height;
        let c = self.carve.read();
        if c.index.is_empty() {
            return h;
        }
        c.apply(x, z, h).0
    }

    /// The ground as the land made it, before anything was dug out of it.
    pub fn natural_height(&self, x: f32, z: f32) -> f32 {
        self.sample(x, z).height
    }

    /// The roads within `r` of (x, z), nearest first (at most `n`).
    pub fn roads_near(&self, x: f32, z: f32, r: f32, n: usize) -> Vec<Road> {
        let c = self.carve.read();
        let mut v: Vec<(f32, Road)> = c.roads.iter().map(|rd| (rd.dist(x, z) - rd.half, *rd)).filter(|(d, _)| *d < r).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.into_iter().take(n).map(|x| x.1).collect()
    }

    /// Whether (x, z) lies on a road (scatter keeps off it).
    pub fn on_road(&self, x: f32, z: f32, margin: f32) -> bool {
        self.carve.read().roads.iter().any(|r| r.dist(x, z) < r.half + margin)
    }

    /// The hollows nearest `p` (at most `n`), for the renderer.
    pub fn hollows_near(&self, x: f32, z: f32, n: usize) -> (Vec<Hollow>, u32) {
        let c = self.carve.read();
        let mut v: Vec<(f32, Hollow)> = c.all().map(|h| (((h.c[0] - x).powi(2) + (h.c[1] - z).powi(2)).sqrt() - h.reach(), *h)).filter(|(d, _)| *d < 400.0).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        (v.into_iter().take(n).map(|x| x.1).collect(), c.version)
    }

    pub fn normal(&self, x: f32, z: f32) -> glam::Vec3 {
        let e = 0.25;
        let hx = self.height(x + e, z) - self.height(x - e, z);
        let hz = self.height(x, z + e) - self.height(x, z - e);
        glam::Vec3::new(-hx, 2.0 * e, -hz).normalize()
    }

    /// Dominant biome index at a point.
    pub fn biome_at(&self, x: f32, z: f32) -> usize {
        let w = self.weights(x, z);
        let mut best = 0;
        for i in 0..self.biomes.len() {
            if w[i] > w[best] {
                best = i;
            }
        }
        best
    }

    /// Ground colour (linear 0..1), mirroring `ground_color` in the shader.
    pub fn ground_color(&self, pal: &Palette, x: f32, z: f32, h: f32, ny: f32) -> glam::Vec3 {
        let w = self.weights(x, z);
        let v = vnoise2(x * 0.11, z * 0.11, self.seed.wrapping_add(303));
        let v2 = vnoise2(x * 0.023, z * 0.023, self.seed.wrapping_add(304));
        let mut c = glam::Vec3::ZERO;
        for (i, b) in self.biomes.iter().enumerate() {
            let g1 = rgbv(b.ground);
            let g2 = rgbv(b.ground2);
            c += w[i] * g1.lerp(g2, (v * 0.5 + v2 * 0.8).clamp(0.0, 1.0));
        }
        let rock = rgbv(pal.rock);
        let sand = rgbv(pal.sand);
        let snow = rgbv(pal.snow);
        let steep = smoothstep(0.82, 0.68, ny);
        // Bare rock shows its beds: bands that run level along a face.
        let strata = smoothstep(0.3, 0.7, vnoise2(h * 0.9, (x + z) * 0.015, self.seed.wrapping_add(305)));
        let bed = rock * (0.55 + 0.25 * v + 0.45 * strata);
        c = c.lerp(bed * glam::Vec3::new(1.0 + 0.1 * strata, 1.0, 1.0 - 0.1 * strata), steep);
        let beach = smoothstep(WATER_LEVEL + 1.4, WATER_LEVEL + 0.3, h);
        c = c.lerp(sand, beach);
        let snowy = smoothstep(46.0, 56.0, h + v * 6.0) * smoothstep(0.6, 0.8, ny);
        let mut c = c.lerp(snow, snowy);
        let carve = self.carve.read();
        for r in &carve.roads {
            let k = r.cover(x, z);
            if k > 0.0 {
                c = c.lerp(glam::Vec3::from_array(r.color) * (0.9 + 0.2 * v), k);
            }
        }
        let (_, dug) = carve.apply(x, z, h);
        c.lerp(EARTH * (0.8 + 0.3 * v), dug * 0.85)
    }
}

/// Bare earth, where ground was dug away.
pub const EARTH: glam::Vec3 = glam::Vec3::new(0.29, 0.22, 0.16);

pub fn rgbv(c: [u8; 3]) -> glam::Vec3 {
    glam::Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32) / 255.0
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_are_sane() {
        let t = Terrain::new(42, default_biomes());
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for i in 0..400 {
            for j in 0..40 {
                let h = t.height(i as f32 * 25.0 - 5000.0, j as f32 * 250.0 - 5000.0);
                assert!(h.is_finite());
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        assert!(lo < 0.0, "expected some water, lowest {lo}");
        assert!(hi > 20.0, "expected some hills, highest {hi}");
        let w = t.weights(123.0, 456.0);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-3);
    }

    /// One biome everywhere, hilly enough for cliffs to show.
    fn crags(cliffs: f32, amp: f32) -> Terrain {
        let mut b = base_biomes()[2].clone();
        b.base = 10.0;
        b.amp = amp;
        b.rough = 0.5;
        b.cliffs = cliffs;
        Terrain::new(9, vec![b])
    }

    /// Share of a 2 km square (sampled every 4 m) whose ground is steeper
    /// than `max_ny` (a normal's y), and flatter than `min_ny`.
    fn steep_and_flat(t: &Terrain, max_ny: f32, min_ny: f32) -> (f32, f32) {
        let (mut steep, mut flat, mut n) = (0, 0, 0);
        for j in 0..500 {
            for i in 0..500 {
                let ny = t.normal(i as f32 * 4.0 - 1000.0, j as f32 * 4.0 - 1000.0).y;
                steep += (ny < max_ny) as usize;
                flat += (ny > min_ny) as usize;
                n += 1;
            }
        }
        (steep as f32 / n as f32, flat as f32 / n as f32)
    }

    #[test]
    fn worlds_without_cliffs_keep_their_ground() {
        // A world saved before cliffs existed reads with none…
        let old = r#"{ "name": "meadow", "base": 4, "amp": 10, "rough": 0.15, "ground": [1,2,3], "ground2": [4,5,6], "scatter": { "tree": 0.3 } }"#;
        let b: Biome = serde_json::from_str(old).unwrap();
        assert_eq!(b.cliffs, 0.0);
        assert!(b.clump.is_empty());
        // …and its ground is exactly what it was.
        let t = Terrain::new(42, vec![b.clone(), base_biomes()[1].clone().tap_cliffs(0.0)]);
        for i in 0..2000 {
            let (x, z) = (i as f32 * 7.3 - 6000.0, (i * 37 % 2000) as f32 * 5.1 - 5000.0);
            let w = t.weights(x, z);
            let (mut base, mut amp, mut rough) = (0.0, 0.0, 0.0);
            for (k, b) in t.biomes.iter().enumerate() {
                base += w[k] * b.base;
                amp += w[k] * b.amp;
                rough += w[k] * b.rough;
            }
            let n = fbm2(x * 0.0045, z * 0.0045, t.seed.wrapping_add(101), 5);
            let r = ridged2(x * 0.0032, z * 0.0032, t.seed.wrapping_add(202), 3);
            assert_eq!(t.height(x, z), base + amp * lerp(n, r * 2.0 - 0.6, rough), "at ({x}, {z})");
        }
    }

    #[test]
    fn cliffs_make_rock_faces_and_ledges() {
        let rolling = steep_and_flat(&crags(0.0, 30.0), 0.5, 0.999);
        let cliffy = steep_and_flat(&crags(1.0, 30.0), 0.5, 0.999);
        eprintln!("steeper than 60°, flatter than 2.5°: rolling {rolling:?}, cliffy {cliffy:?}");
        assert!(rolling.0 < 0.002, "rolling hills have no faces: {}", rolling.0);
        assert!(cliffy.0 > 0.02, "cliffs stand as faces: {}", cliffy.0);
        assert!(cliffy.1 > rolling.1 * 1.5, "and step into ledges: {} vs {}", cliffy.1, rolling.1);
        // Some faces are sheer, steeper than anyone climbs.
        let sheer = steep_and_flat(&crags(1.0, 30.0), 0.35, 1.0).0;
        assert!(sheer > 0.003, "sheer faces: {sheer}");
    }

    #[test]
    fn flat_land_stays_flat_however_cliffy() {
        let t = crags(1.0, 3.0);
        let (steep, _) = steep_and_flat(&t, 0.8, 1.0);
        assert!(steep < 0.001, "a plain grows no faces: {steep}");
    }

    #[test]
    fn cliffs_are_continuous() {
        // Walk lines in fine steps: a face rises steeply but never jumps.
        let t = crags(1.0, 40.0);
        let mut worst = 0.0f32;
        for line in 0..40 {
            let z = line as f32 * 53.0 - 1000.0;
            let mut last = t.height(-1000.0, z);
            for i in 1..40_000 {
                let h = t.height(-1000.0 + i as f32 * 0.05, z);
                worst = worst.max((h - last).abs());
                last = h;
            }
        }
        eprintln!("largest rise in 5 cm: {worst} m");
        assert!(worst < 0.6, "no jumps: {worst} m in 5 cm");
    }

    #[test]
    fn boulders_gather_at_the_foot_of_faces() {
        let t = crags(1.0, 30.0);
        let (mut feet, mut near_face) = (0, 0);
        for j in 0..400 {
            for i in 0..400 {
                let (x, z) = (i as f32 * 3.0 - 600.0, j as f32 * 3.0 - 600.0);
                if t.height_and_foot(x, z).1 < 0.7 {
                    continue;
                }
                feet += 1;
                // Uphill from the foot a face rises within a few metres.
                let n = t.normal(x, z);
                let up = -glam::Vec2::new(n.x, n.z).normalize_or_zero();
                let h0 = t.height(x, z);
                let rise = (1..=20).map(|k| t.height(x + up.x * k as f32 * 0.5, z + up.y * k as f32 * 0.5) - h0).fold(0.0f32, f32::max);
                near_face += (rise > 2.0) as usize;
            }
        }
        eprintln!("{near_face} of {feet} foot points under a face");
        assert!(feet > 200, "cliffs have feet: {feet}");
        assert!(near_face as f32 > feet as f32 * 0.6, "{near_face} of {feet} under a face");
    }

    trait TapCliffs {
        fn tap_cliffs(self, c: f32) -> Self;
    }
    impl TapCliffs for Biome {
        fn tap_cliffs(mut self, c: f32) -> Self {
            self.cliffs = c;
            self
        }
    }
}
