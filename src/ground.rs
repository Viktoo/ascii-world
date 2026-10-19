//! Ground edits: the land as people (and rivers) shape it.
//!
//! An edit is a shape (a line with a width, an outline, a disc), what it does
//! to the height inside (lower it to, raise it to, level it at, or leave it),
//! how its sides slope (`edge`), and optionally what covers it (a colour, a
//! rim of other stones just outside) and the water standing over it. Roads,
//! canals, quays, lagoons, islands, plazas and rivers are all edits; nothing
//! here knows those words, they are only `kind` labels for the story.
//!
//! Edits are not worked out per ray. They are stamped, in order, into tiles
//! of 16 m (0.5 m cells) beside the land's own height: how far the ground
//! moved (`delta`), the paint on it, and the water level. Reading the ground
//! is then one bilinear lookup however many edits there are, on the CPU here
//! and on the GPU (`ground_*` in `shaders/common.wgsl`, the twin: keep the
//! operation order identical). Only the tiles an edit touches are stored, and
//! changing an edit restamps only its own tiles.

use crate::terrain::smoothstep;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Cell size (m). Bilinear in between: a quay drops over one cell.
pub const CELL: f32 = 0.5;
/// Cells per tile side, and samples per tile row (one shared with the next tile).
pub const TILE: i32 = 32;
pub const STRIDE: usize = TILE as usize + 1;
pub const TILE_M: f32 = CELL * TILE as f32;
const SAMPLES: usize = STRIDE * STRIDE;
/// Words per tile on the GPU: delta, paint, water, pattern.
pub const TILE_WORDS: usize = SAMPLES * 4;
/// No water stands here (lower than any ground).
pub const NO_WATER: f32 = -1.0e4;
/// Tiles across the window the GPU is given round the camera (640 m), and
/// the step it moves in (64 m): the camera is always ≥ 256 m from its edge.
pub const WINDOW: i32 = 40;
const WINDOW_STEP: i32 = 4;

/// Where an edit acts, in world x, z.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    /// A line through points, `half` wide on each side: a road, a canal, a river.
    Line { pts: Vec<[f32; 2]>, half: f32 },
    /// Inside an outline: a lagoon, an island, a square.
    Area { pts: Vec<[f32; 2]> },
    Disc { c: [f32; 2], r: f32 },
}

/// What an edit does to the height inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Cut down to the height (a canal, a pit, a river bed); higher ground only.
    Lower,
    /// Fill up to it (an island, a causeway, a dyke); lower ground only.
    Raise,
    /// Make it that height either way (a road's bed, a square, a terrace).
    #[default]
    Level,
    /// Leave the height; only colour or water.
    Paint,
}

/// A height: one for the whole edit, or one per point of a line (between
/// points it runs straight), so a road can climb a hill and a river fall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Height {
    At(f32),
    Along(Vec<f32>),
}

impl Default for Height {
    fn default() -> Self {
        Height::At(0.0)
    }
}

impl Height {
    /// At `s`, a position along a line (point index plus fraction).
    pub fn at(&self, s: f32) -> f32 {
        match self {
            Height::At(h) => *h,
            Height::Along(v) if v.is_empty() => 0.0,
            Height::Along(v) => {
                let i = (s.max(0.0) as usize).min(v.len() - 1);
                let j = (i + 1).min(v.len() - 1);
                v[i] + (v[j] - v[i]) * (s - i as f32).clamp(0.0, 1.0)
            }
        }
    }
    fn finite(&self) -> bool {
        match self {
            Height::At(h) => h.is_finite(),
            Height::Along(v) => v.iter().all(|h| h.is_finite()),
        }
    }
}

/// One change to the ground.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edit {
    /// What it is, in words ("road", "canal", "lagoon", "river"): for the
    /// story and for what people make of it; the ground only reads the rest.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    pub shape: Shape,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub to: Height,
    /// How far its sides slope out past the shape (m): 0.3 a quay wall, 2 a
    /// road's shoulders, 10 a lagoon's shelving shore.
    #[serde(default = "d_edge")]
    pub edge: f32,
    /// What covers it (linear rgb), if anything.
    #[serde(default)]
    pub color: Option<[f32; 3]>,
    /// A band of something else just outside it, down its sides and along
    /// the top (quay stones, a kerb): colour and width (m).
    #[serde(default)]
    pub rim: Option<([f32; 3], f32)>,
    /// The water standing over it and its banks, if any (the sea stands
    /// everywhere below 0 anyway).
    #[serde(default)]
    pub water: Option<Height>,
    /// How what covers it is laid (cobbles, setts, flagstones, planks), if
    /// in a pattern: drawn over its colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Pattern>,
}

/// A laid surface's pattern: its kind and the size of one piece (m).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub kind: PatternKind,
    pub size: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternKind {
    /// Square setts in running rows (a street, a quay).
    Setts,
    /// Round cobbles every which way (an old lane, a square).
    Cobbles,
    /// Big flat slabs in rows of different lengths (a piazza, a church step).
    Flagstones,
    /// Boards side by side (a pier, a deck, a boardwalk).
    Planks,
    /// Bricks in a herringbone (a courtyard, a garden path).
    Bricks,
}

impl Pattern {
    /// From words ("cobbles", "flagstones"…) and a piece size, if one fits.
    pub fn parse(kind: &str, size: Option<f32>) -> Option<Pattern> {
        let k = kind.trim().to_lowercase();
        let (kind, d) = match () {
            _ if k.contains("cobble") => (PatternKind::Cobbles, 0.18),
            _ if k.contains("sett") || k.contains("pav") => (PatternKind::Setts, 0.22),
            _ if k.contains("flag") || k.contains("slab") => (PatternKind::Flagstones, 0.7),
            _ if k.contains("plank") || k.contains("board") || k.contains("deck") => (PatternKind::Planks, 0.22),
            _ if k.contains("brick") || k.contains("herring") => (PatternKind::Bricks, 0.22),
            _ => return None,
        };
        Some(Pattern { kind, size: size.filter(|s| s.is_finite()).unwrap_or(d).clamp(0.05, 12.0) })
    }

    /// Packed for the GPU: kind + 1 in the low byte, size in 1/20 m above.
    pub fn code(&self) -> u32 {
        let k = match self.kind {
            PatternKind::Setts => 1,
            PatternKind::Cobbles => 2,
            PatternKind::Flagstones => 3,
            PatternKind::Planks => 4,
            PatternKind::Bricks => 5,
        };
        k | (((self.size * 20.0).round() as u32).clamp(1, 255) << 8)
    }
}

/// How laid ground shades at (x, z) seen from `dist` metres: a factor on
/// its colour. Every laid surface has a faint grit (small stones, a soft
/// mottle), fully on plain ground (a gravel path, a dirt road) and half
/// over a pattern's own pieces. Twin of `pattern_shade` in common.wgsl.
pub fn pattern_shade(code: u32, x: f32, z: f32, dist: f32, seed: u32) -> f32 {
    let lay = laid_shade(code, x, z, dist, seed);
    lay * grit(x, z, dist, seed, if code & 255 == 0 { 1.0 } else { 0.5 })
}

/// The grit of laid ground: a mottle out to 120 m, and close up small
/// stones, each a little lighter or darker than the ground round it.
fn grit(x: f32, z: f32, dist: f32, seed: u32, k: f32) -> f32 {
    use crate::noise::{h2, pcg, u2f, vnoise2};
    let mid = smoothstep(120.0, 25.0, dist);
    if mid <= 0.0 {
        return 1.0;
    }
    let mut f = 1.0 + (vnoise2(x * 1.3, z * 1.3, seed.wrapping_add(320)) - 0.5) * 0.12 * k * mid;
    let near = smoothstep(30.0, 8.0, dist);
    if near > 0.0 {
        let (gx, gz) = ((x * 6.0).floor(), (z * 6.0).floor());
        let h = h2(gx as i32, gz as i32, seed.wrapping_add(321));
        let (px, pz) = (gx + 0.25 + 0.5 * u2f(pcg(h)), gz + 0.25 + 0.5 * u2f(pcg(h ^ 0x51ed)));
        let d = ((x * 6.0 - px).powi(2) + (z * 6.0 - pz).powi(2)).sqrt();
        let r = 0.18 + 0.2 * u2f(pcg(h ^ 0x2b3c));
        if d < r {
            f *= 1.0 + (u2f(h) - 0.5) * 0.22 * k * near * smoothstep(r, r * 0.6, d);
        }
    }
    f
}

/// A laid pattern's own shading: the joints between pieces darker and each
/// piece a little lighter or darker than the next; fading out where the
/// joints grow thinner than a pixel.
fn laid_shade(code: u32, x: f32, z: f32, dist: f32, seed: u32) -> f32 {
    use crate::noise::{h2, u2f};
    let kind = code & 255;
    if kind == 0 {
        return 1.0;
    }
    let s = ((code >> 8) & 255) as f32 / 20.0;
    let fade = smoothstep(0.35, 0.12, dist * 0.004 / s);
    if fade <= 0.0 {
        return 1.0;
    }
    let edge = |f: f32| f.min(1.0 - f);
    let (e, cx, cz) = match kind {
        // Cobbles: the nearest of jittered points; the joint where two meet.
        2 => {
            let (gx, gz) = ((x / s).floor() as i32, (z / s).floor() as i32);
            let (mut f1, mut f2, mut best) = (9.0f32, 9.0f32, (0, 0));
            for j in -1..=1 {
                for i in -1..=1 {
                    let (ix, iz) = (gx + i, gz + j);
                    let px = ix as f32 + 0.15 + 0.7 * u2f(h2(ix, iz, seed));
                    let pz = iz as f32 + 0.15 + 0.7 * u2f(h2(iz, ix, seed ^ 0x9e37));
                    let d = ((x / s - px).powi(2) + (z / s - pz).powi(2)).sqrt();
                    if d < f1 {
                        f2 = f1;
                        f1 = d;
                        best = (ix, iz);
                    } else if d < f2 {
                        f2 = d;
                    }
                }
            }
            ((f2 - f1) * 0.5 * s, best.0, best.1)
        }
        // Planks: long boards across z, their ends staggered.
        4 => {
            let v = z / s;
            let row = v.floor() as i32;
            let u = x / (s * 7.0) + u2f(h2(row, 7, seed));
            (edge(v.fract().abs()).min(edge(u - u.floor()) * 7.0) * s, u.floor() as i32, row)
        }
        // Bricks: two by one, rows turned each way in turn (a basket weave).
        5 => {
            let (bx, bz) = ((x / (2.0 * s)).floor() as i32, (z / (2.0 * s)).floor() as i32);
            let turn = (bx + bz) & 1 == 1;
            let (a, b) = if turn { (z, x) } else { (x, z) };
            let (u, v) = (a / (2.0 * s), b / s);
            (edge(u - u.floor()).min(edge(v - v.floor()) * 0.5) * 2.0 * s, bx * 2 + (v.floor() as i32 & 1), bz)
        }
        // Setts and flagstones: rows, each piece offset from the row before
        // (flagstones by a different amount each row, and longer).
        _ => {
            let long = if kind == 3 { 1.5 } else { 1.0 };
            let v = z / s;
            let row = v.floor() as i32;
            let shift = if kind == 3 { u2f(h2(row, 3, seed)) } else { 0.5 * (row & 1) as f32 };
            let u = x / (s * long) + shift;
            (edge(v - v.floor()).min(edge(u - u.floor()) * long) * s, u.floor() as i32, row)
        }
    };
    let w = s * if kind == 2 { 0.11 } else { 0.06 };
    let joint = smoothstep(w * 1.6, w, e);
    let piece = 0.86 + 0.26 * u2f(h2(cx, cz, seed ^ 0x51));
    let shade = piece + (0.5 - piece) * joint;
    1.0 + (shade - 1.0) * fade
}

fn d_edge() -> f32 {
    1.0
}

impl Edit {
    /// Signed distance from (x, z) to the shape (negative inside) and the
    /// position along a line (point index plus fraction; 0 for areas).
    pub fn dist(&self, x: f32, z: f32) -> (f32, f32) {
        match &self.shape {
            Shape::Disc { c, r } => (((x - c[0]).powi(2) + (z - c[1]).powi(2)).sqrt() - r, 0.0),
            Shape::Line { pts, half } => {
                if pts.len() == 1 {
                    return (((x - pts[0][0]).powi(2) + (z - pts[0][1]).powi(2)).sqrt() - half, 0.0);
                }
                let mut best = (f32::MAX, 0.0);
                for i in 0..pts.len().saturating_sub(1) {
                    let (a, b) = (pts[i], pts[i + 1]);
                    let (bx, bz) = (b[0] - a[0], b[1] - a[1]);
                    let (px, pz) = (x - a[0], z - a[1]);
                    let l2 = (bx * bx + bz * bz).max(1e-6);
                    let t = ((px * bx + pz * bz) / l2).clamp(0.0, 1.0);
                    let d2 = (px - bx * t).powi(2) + (pz - bz * t).powi(2);
                    if d2 < best.0 {
                        best = (d2, i as f32 + t);
                    }
                }
                (best.0.sqrt() - half, best.1)
            }
            Shape::Area { pts } => (poly_sdf(pts, x, z), 0.0),
        }
    }

    /// How far past the shape it reaches (sides, rim, paint's soft edge).
    pub fn reach(&self) -> f32 {
        let rim = self.rim.map(|r| r.1 + 0.4).unwrap_or(0.0);
        let side = if self.mode == Mode::Paint && self.water.is_none() { 0.0 } else { self.edge };
        side.max(rim).max(0.4) + CELL
    }

    /// The shape's bounding box (x0, z0, x1, z1), reach included.
    pub fn bounds(&self) -> [f32; 4] {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        let mut add = |p: [f32; 2], r: f32| {
            lo = [lo[0].min(p[0] - r), lo[1].min(p[1] - r)];
            hi = [hi[0].max(p[0] + r), hi[1].max(p[1] + r)];
        };
        match &self.shape {
            Shape::Disc { c, r } => add(*c, *r),
            Shape::Line { pts, half } => pts.iter().for_each(|p| add(*p, *half)),
            Shape::Area { pts } => pts.iter().for_each(|p| add(*p, 0.0)),
        }
        let r = self.reach();
        [lo[0] - r, lo[1] - r, hi[0] + r, hi[1] + r]
    }

    /// Whether the edit can be stamped (finite, not absurdly big or many-pointed).
    pub fn sane(&self) -> bool {
        let ok = |p: &[f32; 2]| p[0].is_finite() && p[1].is_finite();
        let shape = match &self.shape {
            Shape::Disc { c, r } => ok(c) && r.is_finite() && *r > 0.0 && *r < 2000.0,
            Shape::Line { pts, half } => !pts.is_empty() && pts.len() <= 4096 && pts.iter().all(ok) && half.is_finite() && *half > 0.0 && *half < 500.0,
            Shape::Area { pts } => pts.len() >= 3 && pts.len() <= 4096 && pts.iter().all(ok),
        };
        let b = self.bounds();
        shape && self.edge.is_finite() && self.edge >= 0.0 && self.to.finite() && self.water.as_ref().is_none_or(|w| w.finite()) && b[2] - b[0] < 6000.0 && b[3] - b[1] < 6000.0
    }

    /// Apply to one point: the height `h` there, its paint (rgb, cover) and
    /// water level, in place.
    pub fn apply_at(&self, x: f32, z: f32, h: &mut f32, paint: &mut (Vec3, f32), water: &mut f32, pattern: &mut u32) {
        let (d, s) = self.dist(x, z);
        if d > self.reach() {
            return;
        }
        if self.mode != Mode::Paint && d < self.edge.max(0.05) {
            let w = if d <= 0.0 { 1.0 } else { smoothstep(self.edge.max(0.05), 0.0, d) };
            let to = self.to.at(s);
            match self.mode {
                Mode::Lower if *h > to => *h -= w * (*h - to),
                Mode::Raise if *h < to => *h += w * (to - *h),
                Mode::Level => *h += w * (to - *h),
                _ => {}
            }
        }
        if let Some((c, rw)) = self.rim {
            // Down its sides (inside the edge) and along the top beyond.
            let k = smoothstep(rw + 0.3, rw, d - self.edge.min(0.6)) * smoothstep(-0.35, -0.05, d);
            if k > 0.0 {
                over(paint, c, k);
            }
            if k > 0.5 {
                *pattern = 0;
            }
        }
        if let Some(c) = self.color {
            let k = if self.rim.is_some() { smoothstep(-0.05, -0.35, d) } else { smoothstep(0.3, -0.2, d) };
            if k > 0.0 {
                over(paint, c, k);
            }
            // Its pattern where it mostly covers; plain colour laid over a
            // pattern hides it.
            if k > 0.5 {
                *pattern = self.pattern.map(|p| p.code()).unwrap_or(0);
            }
        }
        if let Some(wl) = &self.water {
            if d <= self.edge.max(0.5) {
                *water = water.max(wl.at(s));
            }
        }
    }
}

/// Lay colour `c` at cover `k` over the paint so far (rgb premultiplied by
/// its cover, and the cover).
fn over(paint: &mut (Vec3, f32), c: [f32; 3], k: f32) {
    paint.0 = paint.0 * (1.0 - k) + Vec3::from_array(c) * k;
    paint.1 += k * (1.0 - paint.1);
}

/// A way's line as laid on the land: cut into pieces of at most `step`
/// metres, each point at the land's height there (`raw`), smoothed over
/// about `smooth` metres so it climbs evenly instead of following every bump.
pub fn lay(pts: &[[f32; 2]], step: f32, smooth: f32, raw: &dyn Fn(f32, f32) -> f32) -> (Vec<[f32; 2]>, Vec<f32>) {
    let mut out: Vec<[f32; 2]> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        let n = (len / step.max(0.5)).ceil().max(1.0) as usize;
        for i in 0..n {
            let t = i as f32 / n as f32;
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    if let Some(l) = pts.last() {
        out.push(*l);
    }
    let h: Vec<f32> = out.iter().map(|p| raw(p[0], p[1])).collect();
    let k = ((smooth / step.max(0.5)) * 0.5).round() as isize;
    let hs = (0..h.len() as isize)
        .map(|i| {
            let (lo, hi) = ((i - k).max(0) as usize, ((i + k) as usize).min(h.len() - 1));
            h[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32
        })
        .collect();
    (out, hs)
}

/// A hollow inside the ground: a cave, a tunnel, a mine, a cellar dug
/// under a hill. Its shape is a few capsules (a passage from `a` to `b` of
/// radius `r`; a round chamber is a short fat one); where one breaks the
/// ground's surface is a way in. The ground is solid below its surface
/// and outside every hollow. `mouth` is where it is entered from outside
/// (the surface end of its way in), for finding the way.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cave {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
    pub parts: Vec<Part>,
    pub mouth: [f32; 3],
}

/// One part of a cave: a capsule from `a` to `b`, `r` round; or, with
/// `half`, a box (a room set into the ground) centred on `a`, `half` its
/// half-size in its own frame, turned `yaw` about the vertical.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub r: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub half: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub yaw: f32,
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl Part {
    /// A capsule from `a` to `b`, `r` round.
    pub fn capsule(a: glam::Vec3, b: glam::Vec3, r: f32) -> Part {
        Part { a: a.to_array(), b: b.to_array(), r, half: None, yaw: 0.0 }
    }

    /// A box centred on `c`, `half` its half-size, turned `yaw` (as a
    /// placed shape's `rot_y`).
    pub fn boxed(c: glam::Vec3, half: glam::Vec3, yaw: f32) -> Part {
        Part { a: c.to_array(), b: c.to_array(), r: 0.0, half: Some(half.to_array()), yaw }
    }

    /// `p` in the box's own frame (from its centre).
    fn local(&self, p: glam::Vec3) -> glam::Vec3 {
        let (dx, dy, dz) = (p.x - self.a[0], p.y - self.a[1], p.z - self.a[2]);
        let (c, s) = (self.yaw.cos(), self.yaw.sin());
        glam::Vec3::new(c * dx - s * dz, dy, s * dx + c * dz)
    }

    pub fn dist(&self, p: glam::Vec3) -> f32 {
        if let Some(h) = self.half {
            let q = self.local(p).abs() - glam::Vec3::from(h);
            return q.max(glam::Vec3::ZERO).length() + q.max_element().min(0.0);
        }
        let (a, b) = (glam::Vec3::from(self.a), glam::Vec3::from(self.b));
        let (pa, ba) = (p - a, b - a);
        let h = (pa.dot(ba) / ba.length_squared().max(1e-6)).clamp(0.0, 1.0);
        (pa - ba * h).length() - self.r
    }

    /// How far it reaches from its axis in x, z.
    fn reach(&self) -> f32 {
        match self.half {
            Some(h) => (h[0] * h[0] + h[2] * h[2]).sqrt(),
            None => self.r,
        }
    }

    /// Distance from (x, z) to it seen from above (for sorting).
    fn flat_dist(&self, x: f32, z: f32) -> f32 {
        let flat = Part { a: [self.a[0], 0.0, self.a[2]], b: [self.b[0], 0.0, self.b[2]], r: self.reach(), half: None, yaw: 0.0 };
        flat.dist(glam::Vec3::new(x, 0.0, z))
    }

    fn sane(&self) -> bool {
        let ok = |v: [f32; 3]| v.iter().all(|x| x.is_finite() && x.abs() < 1e6);
        let size = match self.half {
            Some(h) => h.iter().all(|v| v.is_finite() && *v > 0.05 && *v <= 40.0) && self.yaw.is_finite(),
            None => self.r.is_finite() && self.r > 0.3 && self.r < 40.0,
        };
        ok(self.a) && ok(self.b) && size
    }
}

impl Cave {
    /// Distance to its hollow (negative inside).
    pub fn dist(&self, p: glam::Vec3) -> f32 {
        self.parts.iter().map(|q| q.dist(p)).fold(f32::MAX, f32::min)
    }
    /// Its box in x, z (x0, z0, x1, z1).
    pub fn bounds(&self) -> [f32; 4] {
        let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for q in &self.parts {
            let r = q.reach();
            for e in [q.a, q.b] {
                b = [b[0].min(e[0] - r), b[1].min(e[2] - r), b[2].max(e[0] + r), b[3].max(e[2] + r)];
            }
        }
        b
    }
    pub fn sane(&self) -> bool {
        let ok = |v: [f32; 3]| v.iter().all(|x| x.is_finite() && x.abs() < 1e6);
        !self.parts.is_empty() && self.parts.len() <= 32 && self.parts.iter().all(|q| q.sane()) && ok(self.mouth)
    }
    /// A way into the land from `mouth` (on the surface, at its height)
    /// heading `dir` (x, z) down at `slope`, `len` metres to a chamber
    /// `room` round, its floor `drop` below the mouth: a cave, a mine, a
    /// dwelling dug into a hill.
    pub fn dug(kind: &str, name: &str, mouth: glam::Vec3, dir: glam::Vec2, len: f32, drop: f32, room: f32) -> Cave {
        let d = dir.normalize_or(glam::Vec2::X);
        let floor = mouth.y - drop;
        // Tall enough to walk in; under a roof of earth where it is deep enough.
        let w = ((drop - 0.8) * 0.5).clamp(1.15, 1.6);
        // The passage's floor starts at the ground (a ramp down from the
        // surface: the mouth opens it), and runs down to the chamber's.
        let a = glam::Vec3::new(mouth.x - d.x * 0.5, mouth.y + w - 0.05, mouth.z - d.y * 0.5);
        let end = glam::Vec3::new(mouth.x + d.x * len, floor + w * 0.92, mouth.z + d.y * len);
        // The chamber: as wide as asked, as tall as the depth allows.
        let rr = ((drop - 1.0) * 0.5).clamp(1.2, room * 0.62);
        let c = glam::Vec3::new(mouth.x + d.x * (len + room * 0.7), floor + rr * 0.92, mouth.z + d.y * (len + room * 0.7));
        let side = glam::Vec3::new(-d.y, 0.0, d.x) * (room * 0.5 - rr).max(0.0);
        let deep = d * (room * 0.5 - rr).max(0.0);
        let deep = glam::Vec3::new(deep.x, 0.0, deep.y);
        Cave {
            kind: kind.into(),
            name: name.into(),
            parts: vec![
                Part::capsule(a, end, w),
                Part::capsule(c - side, c + side, rr),
                Part::capsule(c - deep, c + deep, rr),
            ],
            mouth: mouth.to_array(),
        }
    }
}

/// Signed distance to a polygon (negative inside).
pub fn poly_sdf(pts: &[[f32; 2]], x: f32, z: f32) -> f32 {
    let n = pts.len();
    if n < 3 {
        return f32::MAX;
    }
    let mut d = (x - pts[0][0]).powi(2) + (z - pts[0][1]).powi(2);
    let mut s = 1.0f32;
    let mut j = n - 1;
    for i in 0..n {
        let (vi, vj) = (pts[i], pts[j]);
        let e = [vj[0] - vi[0], vj[1] - vi[1]];
        let w = [x - vi[0], z - vi[1]];
        let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-9)).clamp(0.0, 1.0);
        let b = [w[0] - e[0] * t, w[1] - e[1] * t];
        d = d.min(b[0] * b[0] + b[1] * b[1]);
        let c = [z >= vi[1], z < vj[1], e[0] * w[1] > e[1] * w[0]];
        if (c[0] && c[1] && c[2]) || (!c[0] && !c[1] && !c[2]) {
            s = -s;
        }
        j = i;
    }
    s * d.sqrt()
}

/// One 16 m tile: per sample, how far the ground moved, its paint (rgb and
/// cover, 8 bits each) and the water level.
#[derive(Clone, Debug)]
pub struct Tile {
    pub delta: Box<[f32]>,
    pub paint: Box<[u32]>,
    pub water: Box<[f32]>,
    /// The laid pattern's code (`Pattern::code`, 0: none), nearest sample.
    pub pattern: Box<[u32]>,
    pub wet: bool,
}

/// The ground at a point, as a tile says.
#[derive(Clone, Copy, Debug)]
pub struct At {
    pub delta: f32,
    pub color: Vec3,
    pub cover: f32,
    pub water: f32,
    pub pattern: u32,
}

/// Which list an edit belongs to; they are applied in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The land's own: rivers (from the seed).
    Land = 0,
    /// Settlements' (from their plans).
    Places = 1,
    /// Made in play (saved with the live world).
    Made = 2,
    /// Hollows under shapes set into the ground (caves only; rebuilt with
    /// each world version from what is placed).
    Shapes = 3,
}

/// All the ground edits, and their tiles.
#[derive(Default, Debug)]
pub struct Ground {
    layers: [Vec<Edit>; 3],
    /// Hollows in the ground, by layer like the edits.
    caves: [Vec<Cave>; 4],
    /// Cave ids (into `all_caves`) by tile.
    cave_index: HashMap<(i32, i32), Vec<u32>>,
    all_caves: Vec<Cave>,
    /// Edit ids (into `all`) by tile.
    index: HashMap<(i32, i32), Vec<u32>>,
    all: Vec<Edit>,
    tiles: HashMap<(i32, i32), Arc<Tile>>,
    pub version: u32,
    pack: parking_lot::Mutex<Option<Arc<Pack>>>,
}

impl Clone for Ground {
    fn clone(&self) -> Self {
        Ground { layers: self.layers.clone(), caves: self.caves.clone(), cave_index: self.cave_index.clone(), all_caves: self.all_caves.clone(), index: self.index.clone(), all: self.all.clone(), tiles: self.tiles.clone(), version: self.version, pack: Default::default() }
    }
}

/// The tiles an edit's box covers.
fn tiles_of(b: [f32; 4]) -> impl Iterator<Item = (i32, i32)> {
    let (x0, z0) = ((b[0] / TILE_M).floor() as i32, (b[1] / TILE_M).floor() as i32);
    let (x1, z1) = ((b[2] / TILE_M).floor() as i32, (b[3] / TILE_M).floor() as i32);
    (z0..=z1).flat_map(move |z| (x0..=x1).map(move |x| (x, z)))
}

/// Floor division of a cell index into its tile and the cell within it.
#[inline]
fn split(i: i32) -> (i32, usize) {
    let t = i >> 5;
    (t, (i - (t << 5)) as usize)
}

impl Ground {
    pub fn edits(&self) -> &[Edit] {
        &self.all
    }
    pub fn layer(&self, l: Layer) -> &[Edit] {
        &self.layers[l as usize]
    }
    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// The ground at (x, z), if any edit reaches it. Twin of `ground_at` in
    /// common.wgsl.
    pub fn at(&self, x: f32, z: f32) -> Option<At> {
        if self.tiles.is_empty() {
            return None;
        }
        let fx = x * (1.0 / CELL);
        let fz = z * (1.0 / CELL);
        let cx = fx.floor();
        let cz = fz.floor();
        let (tx, lx) = split(cx as i32);
        let (tz, lz) = split(cz as i32);
        let t = self.tiles.get(&(tx, tz))?;
        let (ux, uz) = (fx - cx, fz - cz);
        let i0 = lz * STRIDE + lx;
        let bl = |v: &[f32]| lerp(lerp(v[i0], v[i0 + 1], ux), lerp(v[i0 + STRIDE], v[i0 + STRIDE + 1], ux), uz);
        let p = [t.paint[i0], t.paint[i0 + 1], t.paint[i0 + STRIDE], t.paint[i0 + STRIDE + 1]].map(unpack);
        let pc = p[0].lerp(p[1], ux).lerp(p[2].lerp(p[3], ux), uz);
        // Stored premultiplied by its cover, so a soft edge fades to the
        // ground and not through black.
        let color = pc.truncate() / pc.w.max(1.0 / 255.0);
        let near = i0 + if uz >= 0.5 { STRIDE } else { 0 } + if ux >= 0.5 { 1 } else { 0 };
        Some(At { delta: bl(&t.delta), color, cover: pc.w, water: if t.wet { bl(&t.water) } else { NO_WATER }, pattern: t.pattern[near] })
    }

    /// How far the ground at (x, z) was moved (0 where no edit reaches).
    pub fn delta(&self, x: f32, z: f32) -> f32 {
        self.at(x, z).map(|a| a.delta).unwrap_or(0.0)
    }

    /// The edits whose shape (not sides) holds (x, z), with how far inside.
    pub fn edits_at(&self, x: f32, z: f32, margin: f32) -> Vec<(&Edit, f32)> {
        let k = ((x / TILE_M).floor() as i32, (z / TILE_M).floor() as i32);
        let Some(ids) = self.index.get(&k) else { return Vec::new() };
        ids.iter().filter_map(|i| self.all.get(*i as usize)).filter_map(|e| Some((e, e.dist(x, z).0)).filter(|(_, d)| *d <= margin)).collect()
    }

    /// The edits whose boxes reach within `r` of (x, z).
    pub fn edits_near(&self, x: f32, z: f32, r: f32) -> Vec<&Edit> {
        self.all.iter().filter(|e| e.dist(x, z).0 < r).collect()
    }

    /// Replace one layer's edits. Returns the tiles to restamp (those any
    /// changed edit touches), or None when nothing changed.
    pub fn plan(&self, layer: Layer, edits: &[Edit]) -> Option<(Vec<Edit>, Vec<(i32, i32)>)> {
        let old = &self.layers[layer as usize];
        if old.as_slice() == edits {
            return None;
        }
        let mut touched = std::collections::HashSet::new();
        for e in old.iter().filter(|e| !edits.contains(e)).chain(edits.iter().filter(|e| !old.contains(e))) {
            touched.extend(tiles_of(e.bounds()));
        }
        // Edits after a changed one in the same tile see a different ground
        // under them, so the whole tile is restamped: `touched` covers that.
        let mut all = Vec::new();
        for (i, l) in self.layers.iter().enumerate() {
            all.extend(if i == layer as usize { edits.iter() } else { l.iter() }.cloned());
        }
        Some((all, touched.into_iter().collect()))
    }

    /// Swap in a planned layer and its restamped tiles (None: now empty).
    pub fn commit(&mut self, layer: Layer, edits: Vec<Edit>, all: Vec<Edit>, tiles: Vec<((i32, i32), Option<Arc<Tile>>)>) {
        self.layers[layer as usize] = edits;
        self.index = index_of(&all);
        self.all = all;
        for (k, t) in tiles {
            match t {
                Some(t) => self.tiles.insert(k, t),
                None => self.tiles.remove(&k),
            };
        }
        self.version = self.version.wrapping_add(1);
        *self.pack.lock() = None;
    }

    /// Replace one layer's caves. True if anything changed.
    pub fn set_caves(&mut self, layer: Layer, caves: Vec<Cave>) -> bool {
        let caves: Vec<Cave> = caves.into_iter().filter(|c| c.sane()).collect();
        if self.caves[layer as usize] == caves {
            return false;
        }
        self.caves[layer as usize] = caves;
        self.all_caves = self.caves.iter().flatten().cloned().collect();
        self.cave_index.clear();
        for (i, c) in self.all_caves.iter().enumerate() {
            for k in tiles_of(c.bounds()) {
                self.cave_index.entry(k).or_default().push(i as u32);
            }
        }
        self.version = self.version.wrapping_add(1);
        *self.pack.lock() = None;
        true
    }

    pub fn caves(&self) -> &[Cave] {
        &self.all_caves
    }

    pub fn cave_layer(&self, l: Layer) -> &[Cave] {
        &self.caves[l as usize]
    }

    /// The caves reaching into the tile holding (x, z).
    pub fn caves_at(&self, x: f32, z: f32) -> impl Iterator<Item = &Cave> {
        let k = ((x / TILE_M).floor() as i32, (z / TILE_M).floor() as i32);
        self.cave_index.get(&k).into_iter().flatten().filter_map(|i| self.all_caves.get(*i as usize))
    }

    /// Distance to the nearest hollow at `p` (negative inside one; large
    /// where there is none).
    pub fn void(&self, p: glam::Vec3) -> f32 {
        self.caves_at(p.x, p.z).map(|c| c.dist(p)).fold(1e4, f32::min)
    }

    /// The capsules nearest (x, z) for the renderer, at most `n`.
    pub fn parts_near(&self, x: f32, z: f32, n: usize) -> Vec<Part> {
        let mut v: Vec<(f32, Part)> = self
            .all_caves
            .iter()
            .flat_map(|c| c.parts.iter())
            .map(|q| (q.flat_dist(x, z), *q))
            .filter(|(d, _)| *d < 250.0)
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.into_iter().take(n).map(|x| x.1).collect()
    }

    /// A fingerprint of the caves reaching into the box (x0, z0)–(x1, z1).
    pub fn cave_sig(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut ids: Vec<u32> = tiles_of([x0, z0, x1, z1]).flat_map(|k| self.cave_index.get(&k).into_iter().flatten().copied()).collect();
        ids.sort_unstable();
        ids.dedup();
        for c in ids.into_iter().filter_map(|i| self.all_caves.get(i as usize)) {
            for v in c.mouth.iter().chain(c.parts.iter().flat_map(|q| q.a.iter())) {
                v.to_bits().hash(&mut h);
            }
        }
        h.finish()
    }

    /// A fingerprint of the tiles in the box (x0, z0)–(x1, z1): changes
    /// when the ground there is reshaped.
    pub fn sig(&self, x0: f32, z0: f32, x1: f32, z1: f32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for k in tiles_of([x0, z0, x1, z1]) {
            if let Some(t) = self.tiles.get(&k) {
                k.hash(&mut h);
                (Arc::as_ptr(t) as usize).hash(&mut h);
            }
        }
        h.finish()
    }

    /// The GPU's copy of the tiles round (cx, cz) (see `Pack`), cached.
    pub fn pack(&self, cx: f32, cz: f32) -> Arc<Pack> {
        let origin = window_origin(cx, cz);
        let mut c = self.pack.lock();
        if let Some(p) = c.as_ref().filter(|p| p.origin == origin && p.version == self.version) {
            return p.clone();
        }
        let n = WINDOW as usize;
        let mut words = vec![0u32; n * n];
        let mut wet = false;
        for dz in 0..WINDOW {
            for dx in 0..WINDOW {
                let Some(t) = self.tiles.get(&(origin[0] + dx, origin[1] + dz)) else { continue };
                let slot = (words.len() - n * n) / TILE_WORDS;
                words[(dz * WINDOW + dx) as usize] = slot as u32 + 1;
                words.extend(t.delta.iter().map(|v| v.to_bits()));
                words.extend(t.paint.iter());
                words.extend(t.water.iter().map(|v| v.to_bits()));
                words.extend(t.pattern.iter());
                wet |= t.wet;
            }
        }
        let p = Arc::new(Pack { origin, version: self.version, words, wet });
        *c = Some(p.clone());
        p
    }
}

/// The tiles round the camera as the GPU reads them: `WINDOW`² directory
/// words (slot + 1, 0 for none), then each tile's delta, paint and water.
#[derive(Debug)]
pub struct Pack {
    pub origin: [i32; 2],
    pub version: u32,
    pub words: Vec<u32>,
    pub wet: bool,
}

impl Pack {
    pub fn tiles(&self) -> usize {
        (self.words.len() - (WINDOW * WINDOW) as usize) / TILE_WORDS
    }
    /// For `Globals::ground`: origin tile x, z, window size (0: none), water.
    pub fn globals(&self) -> [f32; 4] {
        if self.tiles() == 0 {
            return [0.0; 4];
        }
        [self.origin[0] as f32, self.origin[1] as f32, WINDOW as f32, if self.wet { 1.0 } else { 0.0 }]
    }
}

/// The window's first tile for a camera at (cx, cz), moving in steps.
pub fn window_origin(cx: f32, cz: f32) -> [i32; 2] {
    let o = |c: f32| (((c / TILE_M).floor() as i32 - WINDOW / 2).div_euclid(WINDOW_STEP)) * WINDOW_STEP;
    [o(cx), o(cz)]
}

fn index_of(all: &[Edit]) -> HashMap<(i32, i32), Vec<u32>> {
    let mut index: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    for (i, e) in all.iter().enumerate() {
        for k in tiles_of(e.bounds()) {
            index.entry(k).or_default().push(i as u32);
        }
    }
    index
}

/// Threads for a big job (stamping, tracing rivers): most cores, but two
/// under test, where many run at once beside timing tests.
pub fn workers() -> usize {
    if cfg!(test) { 2 } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 12) }
}

/// Stamp the tiles `keys` from the full edit list `all`, over the land's own
/// height `raw`. In parallel; empty tiles come back as None.
pub fn stamp(all: &[Edit], keys: &[(i32, i32)], raw: &(dyn Fn(f32, f32) -> f32 + Sync)) -> Vec<((i32, i32), Option<Arc<Tile>>)> {
    let index = index_of(all);
    let threads = workers();
    let chunk = keys.len().div_ceil(threads).max(1);
    std::thread::scope(|s| {
        let hs: Vec<_> = keys.chunks(chunk).map(|ks| s.spawn(|| ks.iter().map(|k| (*k, stamp_tile(all, index.get(k).map(|v| v.as_slice()).unwrap_or(&[]), *k, raw))).collect::<Vec<_>>())).collect();
        hs.into_iter().flat_map(|h| h.join().expect("stamp thread")).collect()
    })
}

fn stamp_tile(all: &[Edit], ids: &[u32], k: (i32, i32), raw: &(dyn Fn(f32, f32) -> f32 + Sync)) -> Option<Arc<Tile>> {
    if ids.is_empty() {
        return None;
    }
    let mut t = Tile { delta: vec![0.0; SAMPLES].into(), paint: vec![0; SAMPLES].into(), water: vec![NO_WATER; SAMPLES].into(), pattern: vec![0; SAMPLES].into(), wet: false };
    let mut any = false;
    for j in 0..STRIDE {
        for i in 0..STRIDE {
            let x = (k.0 * TILE + i as i32) as f32 * CELL;
            let z = (k.1 * TILE + j as i32) as f32 * CELL;
            let h0 = raw(x, z);
            let mut h = h0;
            let mut paint = (Vec3::ZERO, 0.0f32);
            let mut water = NO_WATER;
            let mut pattern = 0;
            for e in ids.iter().filter_map(|i| all.get(*i as usize)) {
                e.apply_at(x, z, &mut h, &mut paint, &mut water, &mut pattern);
            }
            let n = j * STRIDE + i;
            t.delta[n] = h - h0;
            t.paint[n] = pack(paint.0, paint.1);
            t.pattern[n] = pattern;
            t.water[n] = water;
            t.wet |= water > NO_WATER;
            any |= h != h0 || paint.1 > 0.0 || water > NO_WATER;
        }
    }
    any.then(|| Arc::new(t))
}

fn pack(c: Vec3, cover: f32) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    q(c.x) | q(c.y) << 8 | q(c.z) << 16 | q(cover) << 24
}

fn unpack(p: u32) -> glam::Vec4 {
    glam::Vec4::new((p & 255) as f32, (p >> 8 & 255) as f32, (p >> 16 & 255) as f32, (p >> 24) as f32) * (1.0 / 255.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The water a hollow holds when it is filled at `at`: it rises from the
/// hollow's bottom over the ground joined to it until it would spill over
/// the lowest point of its rim, or reaches `up_to` (the ground water's
/// level, say), whichever is lower. Looked for within `reach` of `at` (a
/// hollow open past that spills there). An edit laying that water over the
/// ground it covers, or None if none would stand there.
pub fn basin(height: &dyn Fn(f32, f32) -> f32, at: [f32; 2], up_to: f32, reach: f32) -> Option<Edit> {
    use std::cmp::Reverse;
    let reach = if reach.is_finite() { reach.clamp(2.0, 300.0) } else { 2.0 };
    let step = (reach / 80.0).max(CELL);
    let n = (2.0 * reach / step).ceil() as i32 + 1;
    let o = [at[0] - reach, at[1] - reach];
    let pos = |i: i32, j: i32| [o[0] + i as f32 * step, o[1] + j as f32 * step];
    let id = |i: i32, j: i32| (j * n + i) as usize;
    let h: Vec<f32> = (0..n * n).map(|k| pos(k % n, k / n)).map(|p| height(p[0], p[1])).collect();
    let rim = |i: i32, j: i32| i <= 0 || j <= 0 || i >= n - 1 || j >= n - 1;
    // Down to the hollow's bottom first (the deed may point at its side).
    let (mut i, mut j) = (n / 2, n / 2);
    loop {
        let lower = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)].iter().map(|d| (i + d.0, j + d.1)).filter(|q| !rim(q.0, q.1)).min_by(|a, b| h[id(a.0, a.1)].total_cmp(&h[id(b.0, b.1)]));
        match lower {
            Some(q) if h[id(q.0, q.1)] < h[id(i, j)] => (i, j) = q,
            _ => break,
        }
    }
    let start = id(i, j);
    if !(h[start] < up_to) {
        return None;
    }
    // Then the water rises over the lowest ground next to it, one cell at
    // a time, until the next lies lower than it already stands (past the
    // lip: it would run out), the hollow opens past `reach`, or it would
    // rise past `up_to`.
    let key = |v: f32| Reverse((v * 1000.0) as i64);
    let mut seen = vec![false; h.len()];
    let mut heap = std::collections::BinaryHeap::new();
    heap.push((key(h[start]), start));
    seen[start] = true;
    let mut level = h[start];
    while let Some((_, c)) = heap.pop() {
        let (ci, cj) = (c as i32 % n, c as i32 / n);
        if h[c] >= up_to {
            level = up_to;
            break;
        }
        if h[c] < level - 0.01 || rim(ci, cj) {
            // Full to its brim, a little under where it would run out.
            level -= 0.1;
            break;
        }
        level = level.max(h[c]);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let q = id(ci + di, cj + dj);
            if !seen[q] {
                seen[q] = true;
                heap.push((key(h[q]), q));
            }
        }
    }
    // The ground under it joined to the bottom.
    let mut wet = vec![false; h.len()];
    let mut todo = vec![start];
    wet[start] = h[start] < level;
    while let Some(c) = todo.pop() {
        let (ci, cj) = (c as i32 % n, c as i32 / n);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (qi, qj) = (ci + di, cj + dj);
            if rim(qi, qj) {
                continue;
            }
            let q = id(qi, qj);
            if !wet[q] && h[q] < level {
                wet[q] = true;
                todo.push(q);
            }
        }
    }
    if !wet[start] {
        return None;
    }
    // Its outline, laid a half cell out so it meets the shore.
    let ring = outline(&wet, n);
    let shape = if ring.len() < 3 {
        Shape::Disc { c: pos(i, j), r: step }
    } else {
        let mut pts: Vec<[f32; 2]> = ring.iter().map(|&(i, j)| pos(i, j)).collect();
        pts.push(pts[0]);
        let mut pts = simplify(&pts, step * 0.6);
        pts.pop();
        if pts.len() < 3 { Shape::Disc { c: pos(i, j), r: step } } else { Shape::Area { pts } }
    };
    Some(Edit { pattern: None, kind: "water".into(), name: String::new(), shape, mode: Mode::Paint, to: Height::At(0.0), edge: step, color: None, rim: None, water: Some(Height::At(level)) })
}

/// The cells round the edge of a joined patch of `mask` (n × n), in order.
fn outline(mask: &[bool], n: i32) -> Vec<(i32, i32)> {
    const D: [(i32, i32); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
    let inside = |i: i32, j: i32| i >= 0 && j >= 0 && i < n && j < n && mask[(j * n + i) as usize];
    let Some(s) = mask.iter().position(|w| *w) else { return Vec::new() };
    let s = (s as i32 % n, s as i32 / n);
    let mut out = vec![s];
    // Nothing lies before the first cell in its row, nor in the rows above.
    let (mut p, mut back) = (s, 4);
    let mut first = None;
    for _ in 0..4 * n * n {
        let Some(d) = (1..=8).map(|k| (back + k) % 8).find(|&d| inside(p.0 + D[d].0, p.1 + D[d].1)) else { break };
        let q = (p.0 + D[d].0, p.1 + D[d].1);
        if p == s && first == Some(q) {
            break;
        }
        first.get_or_insert(q);
        if q != s {
            out.push(q);
        }
        p = q;
        back = (d + 4) % 8;
    }
    out
}

/// A line with the points that add little to it (within `tol`) left out.
fn simplify(pts: &[[f32; 2]], tol: f32) -> Vec<[f32; 2]> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut todo = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = todo.pop() {
        let (pa, pb) = (pts[a], pts[b]);
        let e = [pb[0] - pa[0], pb[1] - pa[1]];
        let l2 = (e[0] * e[0] + e[1] * e[1]).max(1e-9);
        let far = (a + 1..b)
            .map(|k| {
                let w = [pts[k][0] - pa[0], pts[k][1] - pa[1]];
                let t = ((w[0] * e[0] + w[1] * e[1]) / l2).clamp(0.0, 1.0);
                (k, (w[0] - e[0] * t).powi(2) + (w[1] - e[1] * t).powi(2))
            })
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((k, _)) = far.filter(|f| f.1 > tol * tol) {
            keep[k] = true;
            todo.push((a, k));
            todo.push((k, b));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bowl dug into a slope fills to the lowest point of its rim (on the
    /// downhill side), not over the slope below it; capped lower, it stops
    /// there; flat ground holds none.
    #[test]
    fn a_basin_fills_to_its_lip() {
        let slope = |x: f32, z: f32| {
            let r = (x * x + z * z).sqrt();
            100.0 + 0.1 * x - 3.0 * (1.0 - r / 8.0).max(0.0)
        };
        let e = basin(&slope, [2.0, 1.0], f32::MAX, 30.0).expect("water");
        let level = match &e.water { Some(Height::At(l)) => *l, _ => panic!() };
        // The lip is on the downhill side, where the bowl's edge (r = 8) meets the slope at x = -8.
        assert!((level - (100.0 - 0.8 - 0.1)).abs() < 0.3, "to its lip: {level}");
        assert!(e.dist(0.0, 0.0).0 < 0.0, "over its bottom");
        assert!(e.dist(-14.0, 0.0).0 > e.edge.max(0.5), "not down the slope below");
        let low = basin(&slope, [0.0, 0.0], 98.0, 30.0).unwrap();
        assert_eq!(low.water, Some(Height::At(98.0)), "capped");
        assert!(basin(&|x, _| x * 0.01, [0.0, 0.0], f32::MAX, 20.0).is_none(), "no hollow");
    }


    fn flat(_: f32, _: f32) -> f32 {
        5.0
    }

    fn ground_of(edits: Vec<Edit>) -> Ground {
        let mut g = Ground::default();
        let (all, keys) = g.plan(Layer::Places, &edits).unwrap();
        let tiles = stamp(&all, &keys, &flat);
        g.commit(Layer::Places, edits, all, tiles);
        g
    }

    fn canal() -> Edit {
        Edit { pattern: None, kind: "canal".into(), name: String::new(), shape: Shape::Line { pts: vec![[-40.0, 3.0], [40.0, 3.0]], half: 3.0 }, mode: Mode::Lower, to: Height::At(-1.5), edge: 0.3, color: Some([0.2, 0.2, 0.15]), rim: Some(([0.6, 0.6, 0.6], 0.8)), water: None }
    }

    #[test]
    fn a_canal_cuts_a_channel_with_quays() {
        let g = ground_of(vec![canal()]);
        let h = |x: f32, z: f32| 5.0 + g.delta(x, z);
        assert!((h(0.0, 3.0) - -1.5).abs() < 1e-4, "its bed: {}", h(0.0, 3.0));
        assert!((h(0.0, 1.0) - -1.5).abs() < 1e-4, "flat across: {}", h(0.0, 1.0));
        assert!((h(0.0, 7.0) - 5.0).abs() < 1e-4, "the bank beyond its sides: {}", h(0.0, 7.0));
        assert!((h(0.0, -1.0) - 5.0).abs() < 1e-4, "both banks: {}", h(0.0, -1.0));
        // The quay's stones along the top, the bed's colour inside.
        let q = g.at(0.1, 6.6).unwrap();
        assert!(q.cover > 0.9 && q.color.x > 0.5, "quay stones: {q:?}");
        let b = g.at(0.1, 3.0).unwrap();
        assert!(b.cover > 0.9 && b.color.x < 0.3, "the bed: {b:?}");
        assert!(g.at(0.0, 30.0).is_none(), "nothing far off");
    }

    #[test]
    fn order_matters_and_lines_carry_heights() {
        // A road climbing from 2 m to 8 m, then the canal cutting across it.
        let road = Edit { pattern: None, kind: "road".into(), name: String::new(), shape: Shape::Line { pts: vec![[0.0, -20.0], [0.0, 20.0]], half: 2.0 }, mode: Mode::Level, to: Height::Along(vec![2.0, 8.0]), edge: 2.0, color: Some([0.5, 0.4, 0.3]), rim: None, water: None };
        let g = ground_of(vec![road, canal()]);
        let h = |x: f32, z: f32| 5.0 + g.delta(x, z);
        assert!((h(0.0, -20.0) - 2.0).abs() < 1e-3);
        assert!((h(0.0, -10.0) - 3.5).abs() < 1e-3, "a quarter of the way up: {}", h(0.0, -10.0));
        assert!((h(1.5, -10.0) - 3.5).abs() < 1e-3, "level across");
        assert!((h(0.0, 3.0) - -1.5).abs() < 1e-3, "the canal cuts through the road");
        assert!((h(30.0, -10.0) - 5.0).abs() < 1e-4, "the land beside");
    }

    #[test]
    fn areas_raise_islands_and_water_stands() {
        let lagoon = Edit { pattern: None, kind: "lagoon".into(), name: String::new(), shape: Shape::Disc { c: [0.0, 0.0], r: 60.0 }, mode: Mode::Lower, to: Height::At(-2.0), edge: 8.0, color: None, rim: None, water: None };
        let island = Edit { pattern: None, kind: "island".into(), name: String::new(), shape: Shape::Area { pts: vec![[-20.0, -15.0], [20.0, -15.0], [20.0, 15.0], [-20.0, 15.0]] }, mode: Mode::Raise, to: Height::At(0.8), edge: 0.4, color: None, rim: None, water: None };
        let pond = Edit { pattern: None, kind: "pond".into(), name: String::new(), shape: Shape::Disc { c: [100.0, 0.0], r: 6.0 }, mode: Mode::Lower, to: Height::At(3.0), edge: 2.0, color: None, rim: None, water: Some(Height::At(4.2)) };
        let g = ground_of(vec![lagoon, island, pond]);
        let h = |x: f32, z: f32| 5.0 + g.delta(x, z);
        assert!((h(40.0, 40.0) - -2.0).abs() < 1e-3, "the lagoon: {}", h(40.0, 40.0));
        assert!((h(0.0, 0.0) - 0.8).abs() < 1e-3, "the island: {}", h(0.0, 0.0));
        assert!((h(80.0, -60.0) - 5.0).abs() < 1e-3, "the land round it");
        assert!(h(65.0, 0.0) > -2.0 && h(65.0, 0.0) < 5.0, "its shore shelves");
        assert!((g.at(100.0, 0.0).unwrap().water - 4.2).abs() < 1e-4, "a pond's water above the sea");
        assert_eq!(g.at(0.0, 0.0).unwrap().water, NO_WATER);
    }

    #[test]
    fn only_changed_tiles_are_restamped() {
        let mut g = ground_of(vec![canal()]);
        let n = g.tile_count();
        assert!(n >= 6, "{n} tiles");
        let far = Edit { pattern: None, kind: "square".into(), name: String::new(), shape: Shape::Disc { c: [500.0, 500.0], r: 4.0 }, mode: Mode::Level, to: Height::At(6.0), edge: 1.0, color: None, rim: None, water: None };
        let (all, keys) = g.plan(Layer::Made, std::slice::from_ref(&far)).unwrap();
        assert!(keys.len() <= 4, "only the square's tiles: {}", keys.len());
        let tiles = stamp(&all, &keys, &flat);
        g.commit(Layer::Made, vec![far.clone()], all, tiles);
        assert!(g.tile_count() > n && g.tile_count() <= n + keys.len(), "the square's tiles added");
        assert!(g.plan(Layer::Made, &[far]).is_none(), "nothing changed");
        // Taken away again, its tiles go.
        let (all, keys) = g.plan(Layer::Made, &[]).unwrap();
        let tiles = stamp(&all, &keys, &flat);
        g.commit(Layer::Made, vec![], all, tiles);
        assert_eq!(g.tile_count(), n);
    }

    #[test]
    fn the_window_moves_in_steps_and_packs_what_it_holds() {
        let g = ground_of(vec![canal()]);
        let a = g.pack(0.0, 0.0);
        assert_eq!(a.tiles(), g.tile_count());
        assert!(Arc::ptr_eq(&a, &g.pack(10.0, 10.0)), "cached while the window stays");
        let o = window_origin(0.0, 0.0);
        for (x, z) in [(0.0f32, 0.0f32), (63.0, -63.0), (-100.0, 200.0), (5000.0, -7000.0)] {
            let o = window_origin(x, z);
            let (tx, tz) = ((x / TILE_M).floor() as i32, (z / TILE_M).floor() as i32);
            for (t, o) in [(tx, o[0]), (tz, o[1])] {
                assert!(t - o >= 16 && o + WINDOW - 1 - t >= 15, "camera tile {t} well inside [{o}, {})", o + WINDOW);
            }
        }
        assert_eq!(o, window_origin(30.0, 30.0));
    }

    #[test]
    fn polygons_know_inside() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        assert!((poly_sdf(&sq, 5.0, 5.0) + 5.0).abs() < 1e-4);
        assert!((poly_sdf(&sq, 13.0, 5.0) - 3.0).abs() < 1e-4);
        let rev: Vec<[f32; 2]> = sq.iter().rev().copied().collect();
        assert!((poly_sdf(&rev, 5.0, 5.0) + 5.0).abs() < 1e-4, "either winding");
    }
}
