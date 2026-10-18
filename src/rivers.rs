//! Rivers from the seed. The land is cut into 512 m cells; some cells hold
//! a spring on high ground. From each spring a river runs downhill (the
//! land's own slope, smoothed, with some momentum and a wander), its water
//! never running uphill: where the land rises in its way it cuts a gorge,
//! up to a point, and where it can't go on it ends in a pond. It widens and
//! deepens as it goes and runs out into the sea. A river is a pure function
//! of the seed and the land, so any part of it can be worked out without
//! the rest of the world: only the stretches near the traveler are laid as
//! ground edits (the land layer), and they come and go as they walk.
//!
//! Caves come the same way: some 384 m cells have one opening in a
//! hillside, a passage running into the hill and down to a chamber.

use crate::ground::{Edit, Height, Mode, Shape};
use crate::noise::{hseqf, vnoise2};
use crate::terrain::Terrain;
use glam::Vec2;
use std::collections::HashMap;

/// One spring per cell at most (m).
pub const CELL: f32 = 512.0;
/// Rivers run at most this far (m).
pub const MAX_LEN: f32 = 4500.0;
const STEP: f32 = 12.0;
/// Stretches are laid in pieces of this many steps.
const PIECE: usize = 24;
/// Laid within this far of the traveler (m); worked out again past `MOVE`.
pub const NEAR: f32 = 750.0;
const MOVE: f32 = 250.0;
/// The deepest a river cuts through land in its way before it gives up (m).
const MAX_CUT: f32 = 7.0;

/// A river's course: its points, the water's surface and the bed at each,
/// and how wide it is there (half, m).
#[derive(Clone, Debug, PartialEq)]
pub struct River {
    pub pts: Vec<Vec2>,
    pub surface: Vec<f32>,
    pub bed: Vec<f32>,
    pub half: Vec<f32>,
    /// It ends in a pond (no way on).
    pub pond: bool,
}

/// Rivers worked out so far, and where the land layer was last laid.
#[derive(Default, Debug)]
pub struct Rivers {
    /// How many springs the land has, 0 (none) … 1 (a wet land).
    pub density: f32,
    /// What their banks are (the land's sand, linear rgb).
    pub sand: [f32; 3],
    traced: HashMap<(i32, i32), Option<River>>,
    pub laid_at: Option<Vec2>,
    /// Being laid on another thread now.
    pub busy: bool,
    /// How many caves the hills have, 0 … 1.
    pub caves: f32,
    caves_found: HashMap<(i32, i32), Option<crate::ground::Cave>>,
}

/// One cave mouth per cell at most (m).
pub const CAVE_CELL: f32 = 384.0;

/// The cave in a cell, if it has one: a mouth on a hillside (not too
/// steep, well above any water), going into the hill.
pub fn natural_cave(t: &Terrain, density: f32, c: (i32, i32)) -> Option<crate::ground::Cave> {
    let seed = t.seed.wrapping_add(9931);
    if hseqf(seed, c.0, c.1, 0) > density * 0.35 {
        return None;
    }
    for k in 0..8 {
        let p = Vec2::new((c.0 as f32 + 0.1 + 0.8 * hseqf(seed, c.0, c.1, 1 + k * 2)) * CAVE_CELL, (c.1 as f32 + 0.1 + 0.8 * hseqf(seed, c.0, c.1, 2 + k * 2)) * CAVE_CELL);
        let h = t.raw_height(p.x, p.y);
        let e = 3.0;
        let g = Vec2::new(t.raw_height(p.x + e, p.y) - t.raw_height(p.x - e, p.y), t.raw_height(p.x, p.y + e) - t.raw_height(p.x, p.y - e)) / (2.0 * e);
        let slope = g.length();
        if h < t.sea + 4.0 || !(0.25..0.9).contains(&slope) {
            continue;
        }
        // Into the hill: up its slope, the passage running level-ish while
        // the hill rises over it.
        let dir = g / slope;
        let r = |i: u32| hseqf(seed, c.0, c.1, 40 + i);
        let len = 8.0 + 14.0 * r(0);
        let ahead = p + dir * (len + 6.0);
        // There must be hill enough over the chamber.
        if t.raw_height(ahead.x, ahead.y) < h + 4.0 {
            continue;
        }
        return Some(crate::ground::Cave::dug("cave", "", glam::Vec3::new(p.x, h, p.y), dir, len, 1.5 + 3.0 * r(1), 3.5 + 3.5 * r(2)));
    }
    None
}

fn cell_of(p: Vec2) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

/// The land smoothed over about `r` metres (rivers follow valleys, not every bump).
fn smooth(t: &Terrain, p: Vec2, r: f32) -> f32 {
    let mut s = t.raw_height(p.x, p.y) * 2.0;
    for k in 0..6 {
        let a = k as f32 * std::f32::consts::TAU / 6.0;
        s += t.raw_height(p.x + a.cos() * r, p.y + a.sin() * r);
    }
    s / 8.0
}

/// The spring in a cell, if it has one: on high ground, more often in a
/// wetter land.
pub fn spring(t: &Terrain, density: f32, c: (i32, i32)) -> Option<Vec2> {
    let seed = t.seed.wrapping_add(7717);
    if hseqf(seed, c.0, c.1, 0) > density * 0.45 {
        return None;
    }
    // The highest of a few spots in the cell.
    let mut best: Option<(f32, Vec2)> = None;
    for k in 0..5 {
        let p = Vec2::new((c.0 as f32 + 0.1 + 0.8 * hseqf(seed, c.0, c.1, 1 + k * 2)) * CELL, (c.1 as f32 + 0.1 + 0.8 * hseqf(seed, c.0, c.1, 2 + k * 2)) * CELL);
        let h = t.raw_height(p.x, p.y);
        if best.is_none_or(|b| h > b.0) {
            best = Some((h, p));
        }
    }
    best.filter(|b| b.0 > 9.0).map(|b| b.1)
}

/// Follow the land down from a spring.
pub fn trace(t: &Terrain, from: Vec2) -> Option<River> {
    let seed = t.seed.wrapping_add(7718);
    let mut pts = vec![from];
    let mut dir = Vec2::ZERO;
    let mut surface = vec![t.raw_height(from.x, from.y) - 0.6];
    let mut pond = false;
    let mut flat_for = 0;
    let mut len = 0.0;
    while len < MAX_LEN {
        let p = *pts.last().expect("a point");
        let e = 14.0;
        let g = Vec2::new(smooth(t, p + Vec2::X * e, 20.0) - smooth(t, p - Vec2::X * e, 20.0), smooth(t, p + Vec2::Y * e, 20.0) - smooth(t, p - Vec2::Y * e, 20.0)) / (2.0 * e);
        let down = -g.normalize_or_zero();
        if g.length() < 0.004 {
            flat_for += 1;
        } else {
            flat_for = 0;
        }
        // Downhill, with momentum, wandering.
        let wander = (vnoise2(p.x * 0.004, p.y * 0.004, seed) - 0.5) * 1.1;
        let want = if down == Vec2::ZERO { dir } else { (down + dir * 1.6).normalize_or(down) };
        let want = Vec2::from_angle(wander * 0.5).rotate(want).normalize_or(Vec2::X);
        dir = want;
        let q = p + dir * STEP;
        let h = t.raw_height(q.x, q.y);
        let s_prev = *surface.last().expect("a surface");
        let s = s_prev.min(h - 0.6);
        // The water can't climb: land in the way is cut through, up to a point.
        if h - s > MAX_CUT + 0.6 || flat_for > 12 {
            pond = h > t.sea;
            break;
        }
        // Not back on itself.
        if pts.iter().rev().skip(3).take(60).any(|o| (*o - q).length() < STEP * 0.9) {
            pond = true;
            break;
        }
        pts.push(q);
        surface.push(s);
        len += STEP;
        // Out into the sea a little way, then it is the sea.
        if s < t.sea - 0.5 {
            break;
        }
    }
    if pts.len() < 12 {
        return None;
    }
    let n = pts.len();
    let half: Vec<f32> = (0..n).map(|i| 1.6 + (i as f32 * STEP).sqrt() * 0.13).collect();
    let bed: Vec<f32> = (0..n).map(|i| surface[i] - (0.7 + half[i] * 0.18)).collect();
    // Into the sea the surface is the sea's.
    let surface = surface.into_iter().map(|s| s.max(t.sea - 0.05)).collect();
    Some(River { pts, surface, bed, half, pond })
}

impl River {
    /// The river as ground edits, in pieces (each of nearly one width),
    /// those within `r` of `near` only; and a pond at its end.
    pub fn edits(&self, near: Vec2, r: f32, sand: [f32; 3]) -> Vec<Edit> {
        let mut out = Vec::new();
        let n = self.pts.len();
        let mut i = 0;
        while i + 1 < n {
            let j = (i + PIECE).min(n - 1);
            let close = self.pts[i..=j].iter().any(|p| (*p - near).length() < r);
            if close {
                let half = self.half[i..=j].iter().sum::<f32>() / (j - i + 1) as f32;
                let line: Vec<[f32; 2]> = self.pts[i..=j].iter().map(|p| p.to_array()).collect();
                // Its banks: where the land beside it lies lower than its
                // water, a low bank holds it in (a levee).
                out.push(Edit { pattern: None,
                    kind: "riverbank".into(),
                    name: String::new(),
                    shape: Shape::Line { pts: line.clone(), half: half + 2.0 + half * 0.7 },
                    mode: Mode::Raise,
                    to: Height::Along(self.surface[i..=j].iter().map(|s| s + 0.35).collect()),
                    edge: 4.0,
                    color: None,
                    rim: None,
                    water: None,
                });
                out.push(Edit { pattern: None,
                    kind: "river".into(),
                    name: String::new(),
                    shape: Shape::Line { pts: line, half },
                    mode: Mode::Lower,
                    to: Height::Along(self.bed[i..=j].to_vec()),
                    edge: 2.0 + half * 0.7,
                    color: None,
                    // Sand down its sides and a little up the bank.
                    rim: Some((sand, 0.8)),
                    water: Some(Height::Along(self.surface[i..=j].to_vec())),
                });
            }
            i = j;
        }
        if self.pond {
            let (end, s) = (self.pts[n - 1], self.surface[n - 1]);
            if (end - near).length() < r + 40.0 {
                let rr = 10.0 + self.half[n - 1] * 2.0;
                out.push(Edit { pattern: None,
                    kind: "pond".into(),
                    name: String::new(),
                    shape: Shape::Disc { c: end.to_array(), r: rr },
                    mode: Mode::Lower,
                    to: Height::At(s - 1.2),
                    edge: 6.0,
                    color: None,
                    rim: None,
                    water: Some(Height::At(s)),
                });
            }
        }
        out
    }
}

impl Rivers {
    pub fn new(density: f32, sand: [f32; 3], caves: f32) -> Rivers {
        Rivers { density: density.clamp(0.0, 1.0), sand, caves: caves.clamp(0.0, 1.0), ..Default::default() }
    }

    /// The caves within `r` of `p`.
    pub fn caves_near(&mut self, t: &Terrain, p: Vec2, r: f32) -> Vec<crate::ground::Cave> {
        if self.caves <= 0.0 {
            return Vec::new();
        }
        let n = (r / CAVE_CELL).ceil() as i32 + 1;
        let c0 = ((p.x / CAVE_CELL).floor() as i32, (p.y / CAVE_CELL).floor() as i32);
        let mut out = Vec::new();
        for dz in -n..=n {
            for dx in -n..=n {
                let c = (c0.0 + dx, c0.1 + dz);
                let density = self.caves;
                let cave = self.caves_found.entry(c).or_insert_with(|| natural_cave(t, density, c));
                if let Some(cv) = cave.as_ref().filter(|cv| (Vec2::new(cv.mouth[0], cv.mouth[2]) - p).length() < r) {
                    out.push(cv.clone());
                }
            }
        }
        out
    }

    /// The edits of every river passing within `r` of `p`.
    pub fn edits_near(&mut self, t: &Terrain, p: Vec2, r: f32) -> Vec<Edit> {
        if self.density <= 0.0 {
            return Vec::new();
        }
        let reach = ((MAX_LEN + r) / CELL).ceil() as i32;
        let c0 = cell_of(p);
        // A spring further than a river runs can't reach here.
        let cells: Vec<(i32, i32)> = (-reach..=reach)
            .flat_map(|dz| (-reach..=reach).map(move |dx| (c0.0 + dx, c0.1 + dz)))
            .filter(|c| (Vec2::new((c.0 as f32 + 0.5) * CELL, (c.1 as f32 + 0.5) * CELL) - p).length() <= MAX_LEN + r + CELL)
            .collect();
        // Those not yet followed, in parallel.
        let density = self.density;
        let todo: Vec<(i32, i32)> = cells.iter().copied().filter(|c| !self.traced.contains_key(c)).collect();
        if !todo.is_empty() {
            let threads = crate::ground::workers();
            let chunk = todo.len().div_ceil(threads);
            let done: Vec<((i32, i32), Option<River>)> = std::thread::scope(|s| {
                let hs: Vec<_> = todo.chunks(chunk).map(|cs| s.spawn(move || cs.iter().map(|c| (*c, spring(t, density, *c).and_then(|sp| trace(t, sp)))).collect::<Vec<_>>())).collect();
                hs.into_iter().flat_map(|h| h.join().expect("river thread")).collect()
            });
            self.traced.extend(done);
        }
        let mut out = Vec::new();
        for c in cells {
            if let Some(Some(rv)) = self.traced.get(&c) {
                out.extend(rv.edits(p, r, self.sand));
            }
        }
        out
    }

    /// Whether the land layer should be laid again for someone at `p`.
    pub fn due(&self, p: Vec2) -> bool {
        (self.density > 0.0 || self.caves > 0.0) && !self.busy && self.laid_at.is_none_or(|a| (a - p).length() > MOVE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn land() -> Terrain {
        let t = Terrain::new(42, crate::terrain::default_biomes());
        *t.rivers.lock() = Rivers::new(1.0, [0.8, 0.7, 0.5], 0.0);
        t
    }

    #[test]
    fn rivers_run_down_to_the_sea_or_a_pond() {
        let t = land();
        let mut found = 0;
        for cz in -4..4 {
            for cx in -4..4 {
                let Some(s) = spring(&t, 1.0, (cx, cz)) else { continue };
                let Some(r) = trace(&t, s) else { continue };
                found += 1;
                // The water never runs uphill.
                assert!(r.surface.windows(2).all(|w| w[1] <= w[0] + 1e-4), "monotone surface");
                // It widens as it goes, its bed under its water.
                assert!(r.half.last() > r.half.first());
                assert!(r.bed.iter().zip(&r.surface).all(|(b, s)| b < s));
                // It ends at the sea, in a pond, or far away.
                let end = *r.surface.last().unwrap();
                let len = (r.pts.len() - 1) as f32 * STEP;
                assert!(end < crate::terrain::WATER_LEVEL + 0.1 || r.pond || len > MAX_LEN - STEP * 2.0, "ends somewhere: {end} after {len} m");
            }
        }
        assert!(found >= 3, "a land of springs has rivers ({found})");
    }

    #[test]
    fn rivers_are_the_same_from_anywhere() {
        // Laid from two places, the stretches both see are the same edits.
        let a = land();
        let b = land();
        let near_a = a.rivers.lock().edits_near(&a, Vec2::new(0.0, 0.0), 900.0);
        let near_b = b.rivers.lock().edits_near(&b, Vec2::new(400.0, 300.0), 900.0);
        let shared: Vec<&Edit> = near_a.iter().filter(|e| near_b.contains(e)).collect();
        assert!(!near_a.is_empty(), "rivers near the middle");
        assert!(!shared.is_empty(), "overlapping stretches agree");
    }

    #[test]
    fn a_dry_land_has_none() {
        let t = Terrain::new(42, crate::terrain::default_biomes());
        assert!(t.rivers.lock().edits_near(&t, Vec2::ZERO, 2000.0).is_empty());
    }
}
