//! Procedural base layer: terrain height, biomes, ground colour, water.
//!
//! Mirrors `terrain_*` in `shaders/common.wgsl`. Nothing here is stored;
//! everything is a pure function of the universe seed and the biome table.

use crate::noise::{fbm2, h2, lerp, pcg, ridged2, u2f, vnoise2};
use serde::{Deserialize, Serialize};

pub const MAX_BIOMES: usize = 6;
pub const BIOME_SHARPNESS: f32 = 26.0;
/// The sea's level in most lands (a world may set its own, or have none).
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

/// The cells craters are drawn in, big and small (m): one crater at most
/// per cell, up to 0.45 of a cell across its rim (see `Terrain::craters`).
pub const CRATER_CELLS: [f32; 2] = [140.0, 30.0];

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
    /// How pocked with craters it is: 0 none … 1 bowls everywhere, big and
    /// small, rims overlapping (the Moon's highlands). See `Terrain::craters`.
    #[serde(default)]
    pub craters: f32,
    /// The ground's own fine texture, close up (see `Grain`); left out, it
    /// is chosen from the ground's colours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grain: Option<Grain>,
}

/// What the ground is like underfoot, drawn into its colour, so it costs no
/// geometry: fine blades combed one way (grass, reeds, heather), and flecks
/// of two colours scattered in clusters (clover and dry straw in a meadow,
/// fallen leaves in a wood, pebbles on a shore, lichen on tundra). Both fade
/// with distance, where a broad mottle carries on instead.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Grain {
    /// How strongly blades show, 0 (smooth: sand, mud, snow) … 1.
    #[serde(default)]
    pub blades: f32,
    /// How many flecks, 0 none … 1 thick.
    #[serde(default)]
    pub flecks: f32,
    /// The flecks' two colours.
    pub fleck: [u8; 3],
    pub fleck2: [u8; 3],
}

impl Biome {
    /// The grain it has, or one chosen from its colours: green ground gets
    /// grass blades, other ground a little grit; flecks of a darker and a
    /// drier shade of its own ground.
    pub fn grain(&self) -> Grain {
        if let Some(g) = &self.grain {
            return Grain { blades: g.blades.clamp(0.0, 1.0), flecks: g.flecks.clamp(0.0, 1.0), ..g.clone() };
        }
        let [r, g, b] = self.ground.map(|v| v as f32);
        let green = ((g - r.max(b)) / g.max(1.0)).clamp(0.0, 1.0);
        let shade = |c: [u8; 3], k: f32| c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8);
        Grain { blades: 0.15 + 0.5 * smoothstep(0.0, 0.25, green), flecks: 0.25, fleck: shade(self.ground, 0.72), fleck2: shade(self.ground2, 1.18) }
    }
}

/// The grain's shading of ground colour `c` at (x, z), `dist` from the eye,
/// for biome weights `w` (the twin of `ground_grain` in common.wgsl).
pub fn ground_grain(biomes: &[Biome], w: &[f32], c: glam::Vec3, x: f32, z: f32, dist: f32, seed: u32) -> glam::Vec3 {
    use crate::noise::{h2, u2f, vnoise2};
    let (mut blades, mut flecks, mut f1, mut f2) = (0.0, 0.0, glam::Vec3::ZERO, glam::Vec3::ZERO);
    for (i, b) in biomes.iter().enumerate().take(MAX_BIOMES) {
        let g = b.grain();
        blades += w[i] * g.blades;
        flecks += w[i] * g.flecks;
        f1 += w[i] * rgbv(g.fleck);
        f2 += w[i] * rgbv(g.fleck2);
    }
    // A mottle of clumps and thinner patches, out to the middle distance.
    let mid = smoothstep(220.0, 40.0, dist);
    let mut c = c * (1.0 + (vnoise2(x * 0.7, z * 0.7, seed.wrapping_add(306)) - 0.5) * 0.22 * blades * mid);
    let near = smoothstep(45.0, 12.0, dist);
    if near > 0.0 {
        // Blades: thin streaks combed along a way that wanders over the land.
        let a = vnoise2(x * 0.04, z * 0.04, seed.wrapping_add(307)) * 6.2832;
        let (sa, ca) = a.sin_cos();
        let u = x * ca + z * sa;
        let v = z * ca - x * sa;
        let row = (v * 22.0).floor();
        let cell = (u * 3.0 + u2f(h2(row as i32, 0, seed.wrapping_add(308))) * 3.0).floor();
        let hb = u2f(h2(row as i32, cell as i32, seed.wrapping_add(309)));
        c *= 1.0 + (hb - 0.5) * 0.34 * blades * near;
    }
    let speck = smoothstep(70.0, 18.0, dist);
    if speck > 0.0 && flecks > 0.0 {
        // Flecks: one in some cells, thick where the patches are, each its
        // own size, stretched and turned (a leaf, a clover, a pebble), its
        // edge ragged and its shade its own.
        let (gx, gz) = ((x * 3.0).floor(), (z * 3.0).floor());
        let hc = h2(gx as i32, gz as i32, seed.wrapping_add(310));
        let clump = vnoise2(x * 0.3, z * 0.3, seed.wrapping_add(311));
        if u2f(hc) < flecks * clump * 0.8 {
            let r1 = |k: u32| u2f(crate::noise::pcg(hc ^ k));
            let (px, pz) = (gx + 0.2 + 0.6 * r1(0x5bd1), gz + 0.2 + 0.6 * r1(0x2c1b));
            let (sa, ca) = (r1(0x7f4a) * 3.1416).sin_cos();
            let (dx, dz) = (x * 3.0 - px, z * 3.0 - pz);
            let (du, dv) = (dx * ca + dz * sa, (dz * ca - dx * sa) / 0.55);
            let r = 0.12 + 0.18 * r1(0x1e35);
            let d = (du * du + dv * dv).sqrt() + (vnoise2(x * 14.0, z * 14.0, seed.wrapping_add(312)) - 0.5) * 0.12;
            if d < r {
                let col = if r1(0x68e3) < 0.5 { f1 } else { f2 };
                c = c.lerp(col * (0.8 + 0.4 * r1(0x3a9c)), smoothstep(r, r * 0.55, d) * 0.7 * speck);
            }
        }
    }
    c
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
            craters: 0.0,
            grain: None,
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
            craters: 0.0,
            grain: None,
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
            craters: 0.0,
            grain: None,
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
            craters: 0.0,
            grain: None,
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
            craters: 0.0,
            grain: None,
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
    /// Held while ground edits are stamped (one change at a time).
    stamping: std::sync::Arc<parking_lot::Mutex<()>>,
    /// Rivers from the seed (see `rivers`).
    pub rivers: std::sync::Arc<parking_lot::Mutex<crate::rivers::Rivers>>,
    /// The sea's level: water stands over all ground below it (`NO_WATER`
    /// in a land with no sea).
    pub sea: f32,
    /// How far under the land's own surface the ground water lies, if any:
    /// ground dug deeper than it fills up to it.
    pub groundwater: Option<f32>,
    /// The land's large shape where the world gives one (see `landforms`).
    pub lands: Option<std::sync::Arc<crate::landforms::LandMap>>,
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
    /// How far its sides slope out past `half` (m): its rim. Steep for a
    /// cellar or a spade's pit; wide for a crater anyone can walk out of.
    #[serde(default = "hollow_edge")]
    pub edge: f32,
}

/// The usual rim of a hollow (m): steep sides.
pub const HOLLOW_EDGE: f32 = 0.35;

fn hollow_edge() -> f32 {
    HOLLOW_EDGE
}

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
        if self.round { self.half[0] + self.edge } else { (self.half[0] * self.half[0] + self.half[1] * self.half[1]).sqrt() + self.edge }
    }
}

/// All the ground taken away.
#[derive(Clone, Debug, Default)]
pub struct Carve {
    /// Under placed shapes (from their `meta.hollow`), rebuilt with each snapshot.
    pub fixed: Vec<Hollow>,
    /// Dug by someone (live, saved with the live world).
    pub dug: Vec<Hollow>,
    /// The land as shaped: roads, canals, lagoons, rivers (see `ground`).
    pub ground: crate::ground::Ground,
    /// Changes whenever any list does.
    pub version: u32,
    /// Hollows by 16 m cell (indices into `fixed` then `dug`); see `reindex`.
    index: std::collections::HashMap<(i32, i32), Vec<u32>>,
}

const CARVE_CELL: f32 = 16.0;

/// A stretch of road: from a to b (x, z), half as wide as `half` × 2, in a
/// colour. How older worlds stored their roads; they become ground edits.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Road {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub half: f32,
    pub color: [f32; 3],
}

impl Road {
    /// The road as a ground edit: its bed levelled along the land, with
    /// shoulders, in its colour.
    pub fn edit(&self, raw: &dyn Fn(f32, f32) -> f32) -> crate::ground::Edit {
        let (pts, hs) = crate::ground::lay(&[self.a, self.b], 3.0, 12.0, raw);
        crate::ground::Edit { pattern: None,
            kind: "road".into(),
            name: String::new(),
            shape: crate::ground::Shape::Line { pts, half: self.half },
            mode: crate::ground::Mode::Level,
            to: crate::ground::Height::Along(hs),
            edge: 1.5,
            color: Some(self.color),
            rim: None,
            water: None,
        }
    }
}

/// The most hollows kept dug; past it the oldest small ones fill back in.
pub const MAX_DUG: usize = 200;

impl Carve {
    /// Keep `dug` within `MAX_DUG`, filling in the oldest spade pits first
    /// (a conjured crater outlasts them).
    pub fn trim_dug(dug: &mut Vec<Hollow>) {
        while dug.len() > MAX_DUG {
            let i = dug.iter().position(|h| h.half[0].max(h.half[1]) <= 1.1).unwrap_or(0);
            dug.remove(i);
        }
    }

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
            if e >= hl.edge {
                continue;
            }
            let w = smoothstep(hl.edge, 0.0, e);
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
            b.craters = if b.craters.is_finite() { b.craters.clamp(0.0, 1.0) } else { 0.0 };
            for c in b.clump.values_mut() {
                *c = if c.is_finite() { c.clamp(0.0, 1.0) } else { 0.5 };
            }
        }
        // Offsets stay zero: per-field seeds decorrelate the noise, and any offset
        // would cost precision (and CPU/GPU parity) far from the origin.
        let o = |_i: u32| 0.0f32;
        Terrain { seed, off: [o(1), o(2), o(3), o(4), o(5), o(6)], biomes, carve: Default::default(), stamping: Default::default(), rivers: Default::default(), sea: WATER_LEVEL, groundwater: None, lands: None }
    }

    /// With the world's sea (None: no sea) and ground water.
    pub fn with_water(mut self, sea: Option<f32>, groundwater: Option<f32>) -> Self {
        self.sea = sea.unwrap_or(crate::ground::NO_WATER);
        self.groundwater = groundwater;
        self
    }

    /// With the world's landforms (none: the seed's land everywhere).
    pub fn with_lands(mut self, forms: &[crate::landforms::Landform]) -> Self {
        self.lands = crate::landforms::LandMap::bake(forms, &self.biomes).map(std::sync::Arc::new);
        self
    }

    /// The level ground water stands at under (x, z), if the land has any.
    pub fn groundwater_at(&self, x: f32, z: f32) -> Option<f32> {
        self.groundwater.map(|d| self.raw_height(x, z) - d)
    }

    pub fn center(i: usize) -> [f32; 2] {
        CENTERS[i]
    }

    pub fn weights(&self, x: f32, z: f32) -> [f32; MAX_BIOMES] {
        self.weights_rise(x, z).0
    }

    /// The biomes' weights at (x, z) and how far landforms raise the land
    /// there. Twin of `terrain_weights` in common.wgsl.
    pub fn weights_rise(&self, x: f32, z: f32) -> ([f32; MAX_BIOMES], f32) {
        let mut w = self.climate_weights(x, z);
        let mut rise = 0.0;
        if let Some((t, r)) = self.lands.as_ref().and_then(|m| m.at(x, z)) {
            let s: f32 = t.iter().sum();
            for (wi, ti) in w.iter_mut().zip(t) {
                *wi = *wi * (1.0 - s) + ti;
            }
            rise = r;
        }
        (w, rise)
    }

    fn climate_weights(&self, x: f32, z: f32) -> [f32; MAX_BIOMES] {
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
        let (w, rise) = self.weights_rise(x, z);
        let mut base = rise;
        let mut amp = 0.0;
        let mut rough = 0.0;
        let mut cliffs = 0.0;
        let mut craters = 0.0;
        for (i, b) in self.biomes.iter().enumerate() {
            base += w[i] * b.base;
            amp += w[i] * b.amp;
            rough += w[i] * b.rough;
            cliffs += w[i] * b.cliffs;
            craters += w[i] * b.craters;
        }
        let n = fbm2(x * 0.0045 + self.off[4], z * 0.0045 + self.off[5], self.seed.wrapping_add(101), 5);
        let r = ridged2(x * 0.0032 + self.off[5], z * 0.0032 + self.off[4], self.seed.wrapping_add(202), 3);
        let height = base + amp * lerp(n, r * 2.0 - 0.6, rough);
        Sample { height: self.craters(x, z, self.cliff(x, z, height, cliffs, amp).h, craters) }
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

    /// Pocks the height `h` with craters where the land has them (`cr`, the
    /// biomes' blend). Twin of `craters_h` in `shaders/common.wgsl`; keep
    /// the operation order identical.
    ///
    /// At two sizes the land is cut into cells, and a cell holds one crater
    /// more often the higher `cr` (growing in as `cr` passes its draw, so
    /// none starts at a step): a round bowl a fifth as deep as it is wide
    /// across, with a raised rim that fades out on the land around it. Each
    /// reaches less than a cell past its own, so the 3 × 3 cells round a
    /// point hold every crater that touches it. Overlapping ones add up.
    pub fn craters(&self, x: f32, z: f32, h: f32, cr: f32) -> f32 {
        if cr < 0.01 {
            return h;
        }
        let mut d = 0.0;
        for k in 0..2u32 {
            let cell = CRATER_CELLS[k as usize];
            let (gx, gz) = ((x / cell).floor() as i32, (z / cell).floor() as i32);
            for oz in -1..=1 {
                for ox in -1..=1 {
                    let (ix, iz) = (gx + ox, gz + oz);
                    let hs = h2(ix, iz, self.seed.wrapping_add(510 + k));
                    let a = ((cr - u2f(hs)) * 8.0).clamp(0.0, 1.0);
                    if a <= 0.0 {
                        continue;
                    }
                    let cx = (ix as f32 + 0.15 + 0.7 * u2f(pcg(hs.wrapping_add(1)))) * cell;
                    let cz = (iz as f32 + 0.15 + 0.7 * u2f(pcg(hs.wrapping_add(2)))) * cell;
                    let r = cell * (0.18 + 0.27 * u2f(pcg(hs.wrapping_add(3))));
                    let (dx, dz) = (x - cx, z - cz);
                    let q = (dx * dx + dz * dz).sqrt() / r;
                    if q >= 1.6 {
                        continue;
                    }
                    let depth = r * 0.22 * a;
                    if q < 1.0 {
                        d -= depth * (1.0 - q * q);
                    }
                    let u = (q - 1.05) / 0.55;
                    if u.abs() < 1.0 {
                        let b = 1.0 - u * u;
                        d += depth * 0.3 * b * b;
                    }
                }
            }
        }
        h + d
    }

    /// The ground's height at (x, z) (as `height`) and how much it lies at
    /// the foot of a rock face (0 … 1): where what falls off cliffs gathers
    /// (boulders, stones).
    pub fn height_and_foot(&self, x: f32, z: f32) -> (f32, f32) {
        let (w, rise) = self.weights_rise(x, z);
        let (mut base, mut amp, mut rough, mut cliffs, mut craters) = (rise, 0.0, 0.0, 0.0, 0.0);
        for (i, b) in self.biomes.iter().enumerate() {
            base += w[i] * b.base;
            amp += w[i] * b.amp;
            rough += w[i] * b.rough;
            cliffs += w[i] * b.cliffs;
            craters += w[i] * b.craters;
        }
        let n = fbm2(x * 0.0045 + self.off[4], z * 0.0045 + self.off[5], self.seed.wrapping_add(101), 5);
        let r = ridged2(x * 0.0032 + self.off[5], z * 0.0032 + self.off[4], self.seed.wrapping_add(202), 3);
        let c = self.cliff(x, z, base + amp * lerp(n, r * 2.0 - 0.6, rough), cliffs, amp);
        let h = {
            let carve = self.carve.read();
            let h = self.craters(x, z, c.h, craters) + carve.ground.delta(x, z);
            if carve.index.is_empty() { h } else { carve.apply(x, z, h).0 }
        };
        // On the ledge just below a sheer face (a gully sheds nothing).
        let below = (1.0 - c.w) - c.t;
        let near = if below >= -0.02 { 1.0 - smoothstep(0.0, 0.08, below) } else { 0.0 };
        (h, c.k * near * (1.0 - smoothstep(0.3, 0.6, c.w)))
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        let h = self.sample(x, z).height;
        let c = self.carve.read();
        let h = h + c.ground.delta(x, z);
        if c.index.is_empty() {
            return h;
        }
        c.apply(x, z, h).0
    }

    /// The ground as the land made it and people shaped it (roads, canals,
    /// rivers), before anything was dug out of it.
    pub fn natural_height(&self, x: f32, z: f32) -> f32 {
        self.sample(x, z).height + self.carve.read().ground.delta(x, z)
    }

    /// The land's own height, from the seed alone (no edits, no pits).
    pub fn raw_height(&self, x: f32, z: f32) -> f32 {
        self.sample(x, z).height
    }

    /// The water's surface at (x, z): the sea's level, or a river's, a
    /// pond's or a canal's where one stands higher. Water shows only where
    /// it is above the ground.
    pub fn water(&self, x: f32, z: f32) -> f32 {
        let c = self.carve.read();
        c.ground.at(x, z).map(|a| a.water.max(self.sea)).unwrap_or(self.sea)
    }

    /// How deep the water is at (x, z) (0 on dry land).
    pub fn water_depth(&self, x: f32, z: f32) -> f32 {
        (self.water(x, z) - self.height(x, z)).max(0.0)
    }

    /// Replace one layer of ground edits (rivers, settlements', or those
    /// made in play), restamping only the tiles that change.
    pub fn set_ground(&self, layer: crate::ground::Layer, edits: Vec<crate::ground::Edit>) {
        let edits: Vec<crate::ground::Edit> = edits.into_iter().filter(|e| e.sane()).collect();
        let _one = self.stamping.lock();
        let Some((all, keys)) = self.carve.read().ground.plan(layer, &edits) else { return };
        let tiles = crate::ground::stamp(&all, &keys, &|x, z| self.raw_height(x, z));
        let mut c = self.carve.write();
        c.ground.commit(layer, edits, all, tiles);
        c.version = c.version.wrapping_add(1);
    }

    /// Lay the rivers near `p` (if it moved far enough since last time):
    /// stretches that come into reach are cut, those left behind fill in.
    pub fn lay_rivers(&self, x: f32, z: f32) {
        let p = glam::Vec2::new(x, z);
        let (edits, caves) = {
            let mut r = self.rivers.lock();
            if !r.due(p) {
                return;
            }
            r.laid_at = Some(p);
            (r.edits_near(self, p, crate::rivers::NEAR), r.caves_near(self, p, crate::rivers::NEAR))
        };
        self.set_ground(crate::ground::Layer::Land, edits);
        self.set_caves(crate::ground::Layer::Land, caves);
    }

    /// As `lay_rivers`, on another thread (for someone walking: the
    /// stretches ahead are cut a little after they come into reach, and
    /// they are far off then).
    pub fn lay_rivers_soon(&self, x: f32, z: f32) {
        {
            // Never waits: while they are being worked out, that is busy.
            let Some(mut r) = self.rivers.try_lock() else { return };
            if !r.due(glam::Vec2::new(x, z)) {
                return;
            }
            r.busy = true;
        }
        let t = self.clone();
        let spawned = std::thread::Builder::new().name("rivers".into()).spawn(move || {
            let p = glam::Vec2::new(x, z);
            let (edits, caves) = {
                let mut r = t.rivers.lock();
                r.laid_at = Some(p);
                (r.edits_near(&t, p, crate::rivers::NEAR), r.caves_near(&t, p, crate::rivers::NEAR))
            };
            t.set_ground(crate::ground::Layer::Land, edits);
            t.set_caves(crate::ground::Layer::Land, caves);
            t.rivers.lock().busy = false;
        });
        if spawned.is_err() {
            self.rivers.lock().busy = false;
        }
    }

    /// How many springs the land has (0 none … 1 a wet land); rivers are
    /// worked out again from scratch.
    pub fn set_rivers(&self, density: f32, sand: [u8; 3], caves: f32) {
        let mut r = self.rivers.lock();
        let sand = rgbv(sand).to_array();
        if r.density != density || r.sand != sand || r.caves != caves {
            *r = crate::rivers::Rivers::new(density, sand, caves);
        }
    }

    /// Replace one layer's caves (hollows inside the ground).
    pub fn set_caves(&self, layer: crate::ground::Layer, caves: Vec<crate::ground::Cave>) {
        let mut c = self.carve.write();
        if c.ground.set_caves(layer, caves) {
            c.version = c.version.wrapping_add(1);
        }
    }

    /// Distance from `p` to the nearest hollow in the ground (negative inside one).
    pub fn void(&self, p: glam::Vec3) -> f32 {
        self.carve.read().ground.void(p)
    }

    /// Whether `p` is inside the solid ground: under its surface and in no hollow.
    pub fn solid(&self, p: glam::Vec3) -> bool {
        p.y < self.height(p.x, p.z) && self.void(p) > 0.0
    }

    /// Whether a cave reaches near (x, z).
    pub fn caves_near(&self, x: f32, z: f32) -> bool {
        self.carve.read().ground.caves_at(x, z).next().is_some()
    }

    /// The floor under a point: the highest ground at or below `top` in the
    /// column at (x, z). The surface, or inside a cave its floor (and over a
    /// cave's mouth, the way down). Where `top` is in solid rock (a cave's
    /// wall), `top` itself.
    pub fn floor(&self, x: f32, top: f32, z: f32) -> f32 {
        let h = self.height(x, z);
        let c = self.carve.read();
        if c.ground.caves_at(x, z).next().is_none() {
            return h;
        }
        let air = |y: f32| (y - h).max(-c.ground.void(glam::Vec3::new(x, y, z)));
        let mut y = top.min(h + 50.0);
        if air(y) <= 0.0 {
            return y;
        }
        for _ in 0..80 {
            let d = air(y);
            if d < 0.004 {
                break;
            }
            y -= d.max(0.01);
        }
        y
    }

    /// The floor of the hollow under (x, z), if a cave lies under it (the
    /// highest one, within 40 m down).
    pub fn cave_floor_under(&self, x: f32, z: f32) -> Option<f32> {
        if !self.caves_near(x, z) {
            return None;
        }
        let h = self.height(x, z);
        let mut y = h - 0.3;
        while y > h - 40.0 {
            let d = self.void(glam::Vec3::new(x, y, z));
            if d < 0.0 {
                return Some(self.floor(x, y, z));
            }
            y -= d.clamp(0.1, 2.0);
        }
        None
    }

    /// The floor of the cave chamber nearest (x, z) within `r`: the land's
    /// own caves (from the seed, laid yet or not) and those dug, but not the
    /// rooms of shapes set into the ground.
    pub fn cave_floor_near(&self, x: f32, z: f32, r: f32) -> Option<glam::Vec3> {
        let p = glam::Vec2::new(x, z);
        let mut caves: Vec<crate::ground::Cave> = self.carve.read().ground.caves().iter().filter(|c| c.kind != crate::inground::KIND).cloned().collect();
        caves.extend(self.rivers.lock().caves_near(self, p, r));
        caves
            .iter()
            .filter_map(|c| {
                let q = c.parts.last().filter(|q| q.half.is_none())?;
                let m = (glam::Vec3::from(q.a) + glam::Vec3::from(q.b)) * 0.5;
                Some(glam::Vec3::new(m.x, m.y - q.r + 0.05, m.z))
            })
            .filter(|m| (glam::Vec2::new(m.x, m.z) - p).length() < r)
            .min_by(|a, b| (glam::Vec2::new(a.x, a.z) - p).length().total_cmp(&(glam::Vec2::new(b.x, b.z) - p).length()))
    }

    /// The ground edits holding (x, z) (within `margin` of their shapes).
    pub fn edits_at(&self, x: f32, z: f32, margin: f32) -> Vec<crate::ground::Edit> {
        self.carve.read().ground.edits_at(x, z, margin).into_iter().map(|(e, _)| e.clone()).collect()
    }

    /// Whether (x, z) lies on ground laid with something (a road, a path, a
    /// quay, a paved square or terrace): scatter keeps off it.
    pub fn on_road(&self, x: f32, z: f32, margin: f32) -> bool {
        self.carve.read().ground.edits_at(x, z, margin).iter().any(|(e, _)| e.color.is_some() && e.mode != crate::ground::Mode::Lower)
    }

    /// A fingerprint of the hollows reaching into the box (x0, z0)–(x1, z1):
    /// changes when ground there is dug, so what stands on it can be placed
    /// again (see `world::scatter::ScatterCache`).
    pub fn carve_sig(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> u64 {
        use std::hash::{Hash, Hasher};
        let c = self.carve.read();
        if c.index.is_empty() && c.ground.is_empty() && c.ground.caves().is_empty() {
            return 0;
        }
        let mut ids: Vec<u32> = Vec::new();
        for cz in (z0 / CARVE_CELL).floor() as i32..=(z1 / CARVE_CELL).floor() as i32 {
            for cx in (x0 / CARVE_CELL).floor() as i32..=(x1 / CARVE_CELL).floor() as i32 {
                ids.extend(c.index.get(&(cx, cz)).into_iter().flatten());
            }
        }
        ids.sort_unstable();
        ids.dedup();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        c.ground.sig(x0, z0, x1, z1).hash(&mut h);
        c.ground.cave_sig(x0, z0, x1, z1).hash(&mut h);
        for hl in ids.into_iter().filter_map(|i| c.get(i)) {
            for v in [hl.c[0], hl.c[1], hl.half[0], hl.half[1], hl.floor, hl.edge] {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish()
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
    pub fn ground_color(&self, pal: &Palette, x: f32, z: f32, h: f32, ny: f32, dist: f32) -> glam::Vec3 {
        let w = self.weights(x, z);
        let v = vnoise2(x * 0.11, z * 0.11, self.seed.wrapping_add(303));
        let v2 = vnoise2(x * 0.023, z * 0.023, self.seed.wrapping_add(304));
        let mut c = glam::Vec3::ZERO;
        for (i, b) in self.biomes.iter().enumerate() {
            let g1 = rgbv(b.ground);
            let g2 = rgbv(b.ground2);
            c += w[i] * g1.lerp(g2, (v * 0.5 + v2 * 0.8).clamp(0.0, 1.0));
        }
        c = ground_grain(&self.biomes, &w, c, x, z, dist, self.seed);
        let rock = rgbv(pal.rock);
        let sand = rgbv(pal.sand);
        let snow = rgbv(pal.snow);
        let steep = smoothstep(0.82, 0.68, ny);
        // Bare rock shows its beds: bands that run level along a face.
        let strata = smoothstep(0.3, 0.7, vnoise2(h * 0.9, (x + z) * 0.015, self.seed.wrapping_add(305)));
        let bed = rock * (0.55 + 0.25 * v + 0.45 * strata);
        c = c.lerp(bed * glam::Vec3::new(1.0 + 0.1 * strata, 1.0, 1.0 - 0.1 * strata), steep);
        let beach = smoothstep(self.sea + 1.4, self.sea + 0.3, h);
        c = c.lerp(sand, beach);
        let snowy = smoothstep(46.0, 56.0, h + v * 6.0) * smoothstep(0.6, 0.8, ny);
        let mut c = c.lerp(snow, snowy);
        let carve = self.carve.read();
        if let Some(g) = carve.ground.at(x, z) {
            let lay = crate::ground::pattern_shade(g.pattern, x, z, dist, self.seed);
            c = c.lerp(g.color * (0.9 + 0.2 * v) * lay, g.cover);
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
    fn grain_follows_the_ground_and_fades() {
        let b = default_biomes();
        let meadow = b.iter().find(|b| b.name == "meadow").unwrap().grain();
        assert!(meadow.blades > 0.5, "green ground gets grass blades: {meadow:?}");
        let sand = Biome { ground: [210, 190, 150], ground2: [190, 170, 130], ..b[0].clone() };
        assert!(sand.grain().blades < 0.3, "sand gets only grit");
        let set = Biome { grain: Some(Grain { blades: 3.0, flecks: 0.2, fleck: [1, 2, 3], fleck2: [4, 5, 6] }), ..b[0].clone() };
        assert_eq!(set.grain().blades, 1.0, "a world's own grain is kept, clamped");
        // Far off, the ground is its plain colour; near, the grain varies it.
        let w = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let c = glam::Vec3::new(0.3, 0.5, 0.2);
        assert_eq!(ground_grain(&b, &w, c, 10.0, 20.0, 500.0, 7), c);
        let near: Vec<glam::Vec3> = (0..64).map(|i| ground_grain(&b, &w, c, i as f32 * 0.37, i as f32 * 0.11, 3.0, 7)).collect();
        assert!(near.iter().any(|v| (*v - c).length() > 0.01), "grain shows up close");
    }

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
