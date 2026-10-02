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

pub fn default_biomes() -> Vec<Biome> {
    let s = |pairs: &[(&str, f32)]| pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    vec![
        Biome {
            name: "meadow".into(),
            base: 4.0,
            amp: 10.0,
            rough: 0.15,
            ground: [96, 142, 64],
            ground2: [128, 160, 72],
            scatter: s(&[("tree", 0.25), ("bush", 0.5), ("rock", 0.15), ("grass", 3.0)]),
        },
        Biome {
            name: "pine forest".into(),
            base: 8.0,
            amp: 18.0,
            rough: 0.35,
            ground: [62, 96, 52],
            ground2: [84, 104, 58],
            scatter: s(&[("pine", 1.4), ("tree", 0.3), ("rock", 0.2), ("bush", 0.3), ("grass", 1.0)]),
        },
        Biome {
            name: "highlands".into(),
            base: 18.0,
            amp: 34.0,
            rough: 0.75,
            ground: [110, 116, 84],
            ground2: [132, 124, 98],
            scatter: s(&[("rock", 0.6), ("pine", 0.2), ("grass", 0.8)]),
        },
        Biome {
            name: "lakeshore".into(),
            base: -5.0,
            amp: 9.0,
            rough: 0.1,
            ground: [104, 140, 82],
            ground2: [168, 160, 112],
            scatter: s(&[("bush", 0.4), ("rock", 0.25), ("grass", 2.0), ("tree", 0.1)]),
        },
        Biome {
            name: "heath".into(),
            base: 6.0,
            amp: 12.0,
            rough: 0.25,
            ground: [128, 112, 74],
            ground2: [112, 92, 96],
            scatter: s(&[("bush", 0.9), ("rock", 0.3), ("grass", 1.2)]),
        },
    ]
}

/// Everything the terrain function needs. Uploaded verbatim to the GPU.
#[derive(Clone, Debug)]
pub struct Terrain {
    pub seed: u32,
    pub off: [f32; 6],
    pub biomes: Vec<Biome>,
}

pub struct Sample {
    pub height: f32,
}

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
        }
        // Offsets stay zero: per-field seeds decorrelate the noise, and any offset
        // would cost precision (and CPU/GPU parity) far from the origin.
        let o = |_i: u32| 0.0f32;
        Terrain { seed, off: [o(1), o(2), o(3), o(4), o(5), o(6)], biomes }
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
        for (i, b) in self.biomes.iter().enumerate() {
            base += w[i] * b.base;
            amp += w[i] * b.amp;
            rough += w[i] * b.rough;
        }
        let n = fbm2(x * 0.0045 + self.off[4], z * 0.0045 + self.off[5], self.seed.wrapping_add(101), 5);
        let r = ridged2(x * 0.0032 + self.off[5], z * 0.0032 + self.off[4], self.seed.wrapping_add(202), 3);
        let height = base + amp * lerp(n, r * 2.0 - 0.6, rough);
        Sample { height }
    }

    pub fn height(&self, x: f32, z: f32) -> f32 {
        self.sample(x, z).height
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
        c = c.lerp(rock * (0.85 + 0.3 * v), steep);
        let beach = smoothstep(WATER_LEVEL + 1.4, WATER_LEVEL + 0.3, h);
        c = c.lerp(sand, beach);
        let snowy = smoothstep(46.0, 56.0, h + v * 6.0) * smoothstep(0.6, 0.8, ny);
        c.lerp(snow, snowy)
    }
}

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
}
