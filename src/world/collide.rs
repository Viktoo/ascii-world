//! Collision against the CPU SDF of nearby solids, with sliding.

use super::Solid;
use crate::terrain::{Terrain, WATER_LEVEL};
use glam::{Vec2, Vec3};

pub const PLAYER_RADIUS: f32 = 0.35;
pub const NPC_RADIUS: f32 = 0.3;
/// Growth (bushes, flowers) gives way below this height above the feet.
pub const GROWTH_GIVES: f32 = 0.5;
/// Water deeper than this is impassable.
pub const MAX_WADE: f32 = 1.1;
/// The steepest ground anyone walks up (rise over run, about 54°): steeper
/// is a rock face. Down it they go (sliding, or falling off its edge).
pub const MAX_CLIMB: f32 = 1.4;
/// How far ahead the slope is judged (a single small step says little).
const CLIMB_PROBE: f32 = 0.3;

pub struct Obstacles<'a> {
    pub solids: &'a [Solid],
    /// Other bodies (feet position, radius) treated as vertical capsules.
    pub bodies: &'a [(Vec3, f32)],
}

/// A walking body: an upright capsule.
#[derive(Clone, Copy, Debug)]
pub struct Capsule {
    pub radius: f32,
    /// How tall it stands now (less when crouching).
    pub height: f32,
    /// The highest step it walks up without jumping.
    pub step: f32,
}

impl Capsule {
    /// A body of this height and radius, standing.
    pub fn of(height: f32, radius: f32) -> Capsule {
        Capsule { radius, height, step: step_for(height) }
    }
}

/// The step a body of `height` takes in its stride (stairs, kerbs, a root).
pub fn step_for(height: f32) -> f32 {
    (height * 0.25).clamp(0.08, 1.2)
}

impl Obstacles<'_> {
    /// Distance from a person-sized column standing on the ground at (x, z)
    /// to the nearest obstacle.
    pub fn dist(&self, terrain: &Terrain, x: f32, z: f32) -> f32 {
        let g = terrain.height(x, z);
        self.dist_body(x, z, g, Capsule::of(1.75, NPC_RADIUS))
    }

    /// Distance from a body's column at (x, z), feet at `feet`, to the nearest
    /// obstacle: solids from just above its step to the top of its head (the
    /// top sample a radius below it: a capsule's cap), and other bodies at
    /// about its height.
    pub fn dist_body(&self, x: f32, z: f32, feet: f32, b: Capsule) -> f32 {
        let lo0 = feet + b.step + 0.05;
        let hi = (feet + b.height - b.radius).max(lo0);
        let mut d = f32::MAX;
        for s in self.solids {
            let lo = if s.ty.is_growth() { lo0.max(feet + GROWTH_GIVES) } else { lo0 };
            if lo > hi {
                continue;
            }
            let n = (((hi - lo) / 0.7).ceil() as usize).clamp(1, 8);
            for i in 0..=n {
                let y = lo + (hi - lo) * i as f32 / n as f32;
                d = d.min(s.sdf(Vec3::new(x, y, z)));
            }
        }
        for (o, r) in self.bodies {
            if o.y > feet + b.height || o.y + 2.2 < feet {
                continue;
            }
            d = d.min(Vec2::new(o.x - x, o.z - z).length() - r);
        }
        d
    }

    /// Whether a body really stands clear at (x, z): its rim, at each height,
    /// is outside every solid. Shapes carved by subtraction (a doorway cut
    /// out of a wall) report distances that are too small near the cut; the
    /// sign is still right, so the rim tells the truth `dist_body` can't.
    pub fn clear(&self, x: f32, z: f32, feet: f32, b: Capsule) -> bool {
        for (o, r) in self.bodies {
            if o.y > feet + b.height || o.y + 2.2 < feet {
                continue;
            }
            if Vec2::new(o.x - x, o.z - z).length() - r < b.radius {
                return false;
            }
        }
        let lo0 = feet + b.step + 0.05;
        let hi = (feet + b.height - b.radius).max(lo0);
        for s in self.solids {
            let c = s.inst.center();
            if Vec2::new(c.x - x, c.z - z).length() > s.inst.radius() + b.radius + 0.1 {
                continue;
            }
            let lo = if s.ty.is_growth() { lo0.max(feet + GROWTH_GIVES) } else { lo0 };
            if lo > hi {
                continue;
            }
            let n = (((hi - lo) / 0.7).ceil() as usize).clamp(1, 8);
            for i in 0..=n {
                let y = lo + (hi - lo) * i as f32 / n as f32;
                let d0 = s.sdf(Vec3::new(x, y, z));
                if d0 < 0.0 {
                    return false;
                }
                if d0 >= b.radius {
                    continue;
                }
                // An uncarved shape's distance is true enough: it touches.
                if !s.ty.ct.carved {
                    return false;
                }
                // March out along each of eight directions to the rim: the
                // distance is a safe step, so even a thin door is met.
                for k in 0..8 {
                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                    let dir = Vec3::new(a.cos(), 0.0, a.sin());
                    let mut t = d0;
                    while t < b.radius {
                        let d = s.sdf(Vec3::new(x, y, z) + dir * t);
                        if d < 0.0 {
                            return false;
                        }
                        t += d.max(0.03);
                    }
                    if s.sdf(Vec3::new(x, y, z) + dir * b.radius) < 0.0 {
                        return false;
                    }
                }
            }
            // And up each line of its height, centre and rim: a beam lower
            // than its head is met however thin.
            if !s.ty.ct.carved {
                continue;
            }
            for k in 0..9 {
                let (ox, oz) = if k == 8 {
                    (0.0, 0.0)
                } else {
                    let a = k as f32 * std::f32::consts::FRAC_PI_4;
                    (a.cos() * b.radius, a.sin() * b.radius)
                };
                let mut y = lo;
                while y <= hi {
                    let d = s.sdf(Vec3::new(x + ox, y, z + oz));
                    if d < 0.0 {
                        return false;
                    }
                    y += d.max(0.03);
                }
            }
        }
        true
    }
}

fn wadeable(terrain: &Terrain, x: f32, z: f32) -> bool {
    terrain.height(x, z) > WATER_LEVEL - MAX_WADE
}

/// Whether going from `from` towards `to` climbs a rock face: the natural
/// ground ahead rises steeper than `MAX_CLIMB` and above the feet. Dug
/// ground is left out (anyone climbs out of a pit or a cellar), and so is a
/// body standing above the ground on something (a roof, a bridge).
pub fn too_steep(terrain: &Terrain, from: Vec2, to: Vec2, feet: f32) -> bool {
    let d = to - from;
    let run = d.length();
    if run < 1e-5 {
        return false;
    }
    let reach = run.max(CLIMB_PROBE);
    let ahead = from + d / run * reach;
    let here = terrain.natural_height(from.x, from.y);
    let there = terrain.natural_height(ahead.x, ahead.y);
    there - here > MAX_CLIMB * reach && there > feet + 0.05 && feet < here + 0.5
}

/// Move a body by `delta` (XZ), sliding along surfaces. Returns the new feet
/// position (its height unchanged: footing settles it, see `sim::footing`).
pub fn move_body(terrain: &Terrain, obs: &Obstacles, from: Vec3, delta: Vec3, b: Capsule) -> Vec3 {
    let radius = b.radius;
    let feet = from.y;
    let dist = |x: f32, z: f32| obs.dist_body(x, z, feet, b);
    // Clear by the fast distance, or (near a carved opening) by the rim.
    let ok = |x: f32, z: f32, d: f32| d >= radius || obs.clear(x, z, feet, b);
    let start = Vec2::new(from.x, from.z);
    let passable = |q: Vec2| wadeable(terrain, q.x, q.y) && !too_steep(terrain, start, q, feet);
    let mut p = Vec2::new(from.x + delta.x, from.z + delta.z);
    if !passable(p) {
        // Slide along the shoreline or the foot of a cliff: try each axis on its own.
        let px = Vec2::new(from.x + delta.x, from.z);
        let pz = Vec2::new(from.x, from.z + delta.z);
        p = if delta.x.abs() > 1e-5 && passable(px) {
            px
        } else if delta.z.abs() > 1e-5 && passable(pz) {
            pz
        } else {
            start
        };
    }
    if !obs.solids.is_empty() || !obs.bodies.is_empty() {
        let d_start = dist(from.x, from.z);
        if d_start < radius * 0.5 && !obs.clear(from.x, from.z, feet, b) {
            // Already overlapping something (an old save, or a building that
            // appeared on top of us): move freely until clear, so we can walk out.
            return Vec3::new(p.x, feet, p.y);
        }
        for _ in 0..5 {
            let d = dist(p.x, p.y);
            if ok(p.x, p.y, d) {
                break;
            }
            let e = 0.04;
            let gx = dist(p.x + e, p.y) - dist(p.x - e, p.y);
            let gz = dist(p.x, p.y + e) - dist(p.x, p.y - e);
            let mut n = Vec2::new(gx, gz);
            if n.length_squared() < 1e-10 {
                n = -Vec2::new(delta.x, delta.z);
            }
            let n = n.normalize_or_zero();
            if n == Vec2::ZERO {
                break;
            }
            p += n * (radius - d + 0.005);
        }
        // Never end up deeper inside than we started (e.g. squeezed between two solids).
        // Never end up deeper inside than we started (e.g. squeezed between
        // two solids, or against a bush and a friend at once): no jostling.
        let d_end = dist(p.x, p.y);
        if d_end < radius && d_end < d_start - 1e-4 && !ok(p.x, p.y, d_end) {
            p = Vec2::new(from.x, from.z);
        }
    }
    Vec3::new(p.x, feet, p.y)
}

/// The top of the support under a column at (x, z): the ground, or the
/// highest surface of a solid between the ground and `top` (a floor, a stair,
/// a roof, a rock). A solid that fills the column at `top` is a wall there,
/// not a floor.
pub fn support(terrain: &Terrain, solids: &[Solid], x: f32, z: f32, top: f32) -> f32 {
    let g = terrain.height(x, z);
    let mut best = g;
    for s in solids {
        let c = s.inst.center();
        let r = s.inst.radius();
        if (c.x - x) * (c.x - x) + (c.z - z) * (c.z - z) > r * r || c.y - r > top || c.y + r < best {
            continue;
        }
        if let Some(y) = column_top(s, x, z, top, best) {
            best = best.max(y);
        }
    }
    best
}

/// Marching down a solid's column from `top`, its first surface above `floor`.
fn column_top(s: &Solid, x: f32, z: f32, top: f32, floor: f32) -> Option<f32> {
    let mut y = top;
    if s.sdf(Vec3::new(x, y, z)) <= 0.0 {
        return None;
    }
    for _ in 0..40 {
        let d = s.sdf(Vec3::new(x, y, z));
        if d < 0.01 {
            return Some(y - d.max(0.0));
        }
        y -= d.max(0.01);
        if y < floor {
            return None;
        }
    }
    None
}

/// The support under a body's footprint: the ground under its centre, and
/// solids under its centre or four points round it (it can stand on the edge
/// of a stair).
pub fn support_under(terrain: &Terrain, solids: &[Solid], p: Vec3, radius: f32, top: f32) -> f32 {
    let mut best = support(terrain, solids, p.x, p.z, top);
    if solids.is_empty() {
        return best;
    }
    let r = radius * 0.6;
    for (dx, dz) in [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)] {
        let (x, z) = (p.x + dx, p.z + dz);
        for s in solids {
            let c = s.inst.center();
            let rr = s.inst.radius();
            if (c.x - x) * (c.x - x) + (c.z - z) * (c.z - z) > rr * rr || c.y - rr > top || c.y + rr < best {
                continue;
            }
            if let Some(y) = column_top(s, x, z, top, best) {
                best = best.max(y);
            }
        }
    }
    best
}

/// The nearest spot to `p` (spiralling out up to `max` metres) where a body of
/// `radius` fits on dry land. Returns `p` if it already fits.
pub fn free_spot(terrain: &Terrain, obs: &Obstacles, p: Vec3, radius: f32, max: f32) -> Vec3 {
    let fits = |x: f32, z: f32| wadeable(terrain, x, z) && obs.dist(terrain, x, z) >= radius + 0.15;
    if fits(p.x, p.z) {
        return p;
    }
    let mut k = 1;
    loop {
        let r = (k as f32).sqrt() * 0.6;
        if r > max {
            return p;
        }
        let a = k as f32 * 2.399_963;
        let (x, z) = (p.x + a.cos() * r, p.z + a.sin() * r);
        if fits(x, z) {
            return Vec3::new(x, terrain.height(x, z), z);
        }
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::compile;
    use crate::render::GpuInst;
    use crate::world::TypeEntry;
    use std::sync::Arc;

    #[test]
    fn can_walk_out_of_a_solid_and_find_free_spot() {
        let ct = compile(
            "export const meta = { name: \"hut\", bounds: [3, 2, 3], tags: [] };\n\
             export function sdf(x, y, z, k) { return box(x, y, z, 3, 2, 3); }\n\
             export function color(x, y, z, k) { return rgb(200, 200, 200); }",
        )
        .unwrap();
        let ty = Arc::new(TypeEntry { id: 1, ct: Arc::new(ct), builtin: false, solid: true, bottom: -2.0, top: 2.0, sphere_cy: 0.0, sphere_r: 4.7 });
        let terrain = Terrain::new(1, crate::terrain::default_biomes());
        let c = Vec3::new(500.0, 0.0, 500.0);
        let inst = GpuInst { pos_scale: [c.x, terrain.height(c.x, c.z) + 1.0, c.z, 1.0], rot: [1.0, 0.0, ty.radius(), 1.0], ..Default::default() };
        let solids = vec![Solid { inst, ty }];
        let obs = Obstacles { solids: &solids, bodies: &[] };
        // Standing inside: walking forward must work until we are out.
        let mut p = Vec3::new(c.x + 0.5, terrain.height(c.x, c.z), c.z);
        for _ in 0..80 {
            p = move_body(&terrain, &obs, p, Vec3::new(0.0, 0.0, 0.1), Capsule::of(1.75, PLAYER_RADIUS));
            p.y = terrain.height(p.x, p.z);
        }
        assert!(p.z - c.z > 3.0 + PLAYER_RADIUS, "walked out of the hut: dz = {}", p.z - c.z);
        // And back towards it is blocked again.
        let before = p;
        for _ in 0..40 {
            p = move_body(&terrain, &obs, p, Vec3::new(0.0, 0.0, -0.1), Capsule::of(1.75, PLAYER_RADIUS));
            p.y = terrain.height(p.x, p.z);
        }
        assert!(p.z - c.z > 3.0, "blocked at the wall: {}", p.z - c.z);
        assert!(before.z > p.z, "walked back until the wall");
        let f = free_spot(&terrain, &obs, Vec3::new(c.x, 0.0, c.z), PLAYER_RADIUS, 20.0);
        assert!(obs.dist(&terrain, f.x, f.z) >= PLAYER_RADIUS);
    }

    #[test]
    fn cannot_walk_through_a_box_and_slides() {
        let ct = compile(
            "export const meta = { name: \"wall\", bounds: [3, 2, 0.5], tags: [] };\n\
             export function sdf(x, y, z, k) { return box(x, y, z, 3, 2, 0.5); }\n\
             export function color(x, y, z, k) { return rgb(200, 200, 200); }",
        )
        .unwrap();
        let ty = Arc::new(TypeEntry { id: 1, ct: Arc::new(ct), builtin: false, solid: true, bottom: -2.0, top: 2.0, sphere_cy: 0.0, sphere_r: 3.7 });
        let terrain = Terrain::new(1, crate::terrain::default_biomes());
        let base = Vec3::new(500.0, 0.0, 500.0);
        let g = terrain.height(base.x, base.z + 5.0);
        let inst = GpuInst {
            pos_scale: [base.x, g + 1.0, base.z + 5.0, 1.0],
            rot: [1.0, 0.0, ty.radius(), 1.0],
            ..Default::default()
        };
        let solids = vec![Solid { inst, ty }];
        let obs = Obstacles { solids: &solids, bodies: &[] };
        let mut p = Vec3::new(base.x, terrain.height(base.x, base.z), base.z);
        // Walk straight at the wall (+z), slightly diagonally.
        for _ in 0..120 {
            p = move_body(&terrain, &obs, p, Vec3::new(0.02, 0.0, 0.1), Capsule::of(1.75, PLAYER_RADIUS));
            p.y = terrain.height(p.x, p.z);
        }
        assert!(p.z - base.z < 4.5 - 0.3, "walked into the wall: z = {}", p.z - base.z);
        assert!(p.x - base.x > 2.0, "should have slid sideways along the wall: x = {}", p.x - base.x);
    }

    /// Walk `steps` steps of `dir`, standing on the ground after each.
    fn walk(terrain: &Terrain, from: Vec3, dir: Vec2, steps: usize) -> Vec3 {
        let obs = Obstacles { solids: &[], bodies: &[] };
        let mut p = from;
        for _ in 0..steps {
            p = move_body(terrain, &obs, p, Vec3::new(dir.x, 0.0, dir.y) * 0.05, Capsule::of(1.75, PLAYER_RADIUS));
            p.y = terrain.height(p.x, p.z);
        }
        p
    }

    #[test]
    fn rock_faces_stop_a_climber_but_not_a_descent() {
        let mut b = crate::terrain::default_biomes()[2].clone();
        (b.base, b.amp, b.rough, b.cliffs) = (10.0, 30.0, 0.5, 1.0);
        let terrain = Terrain::new(9, vec![b]);
        // Sheer faces: steeper than 70°.
        let mut faces = Vec::new();
        'scan: for j in 0..300 {
            for i in 0..300 {
                let (x, z) = (i as f32 * 3.0 - 450.0, j as f32 * 3.0 - 450.0);
                let n = terrain.normal(x, z);
                if n.y < 0.33 && terrain.height(x, z) > WATER_LEVEL + 4.0 {
                    faces.push((Vec2::new(x, z), Vec2::new(n.x, n.z).normalize()));
                    if faces.len() == 20 {
                        break 'scan;
                    }
                }
            }
        }
        assert!(faces.len() >= 10, "found faces: {}", faces.len());
        let (mut held, mut went_down) = (0, 0);
        for (at, away) in &faces {
            // From 4 m out, straight at it.
            let s = *at + *away * 4.0;
            let start = Vec3::new(s.x, terrain.height(s.x, s.y), s.y);
            let end = walk(&terrain, start, -*away, 200);
            let top = terrain.height(at.x - away.x * 3.0, at.y - away.y * 3.0);
            held += (end.y < start.y + (top - start.y) * 0.5) as usize;
            // From above it, off the edge and down.
            let a = *at - *away * 2.5;
            let high = Vec3::new(a.x, terrain.height(a.x, a.y), a.y);
            let low = walk(&terrain, high, *away, 200);
            went_down += (low.y < high.y - 2.0) as usize;
        }
        assert!(held as f32 >= faces.len() as f32 * 0.9, "faces hold climbers back: {held} of {}", faces.len());
        assert_eq!(went_down, faces.len(), "and anyone goes down them");
    }

    #[test]
    fn anyone_climbs_out_of_a_dug_pit() {
        let terrain = Terrain::new(1, crate::terrain::default_biomes());
        let c = Vec2::new(500.0, 500.0);
        let floor = terrain.natural_height(c.x, c.y) - 2.0;
        {
            let mut cv = terrain.carve.write();
            cv.dug.push(crate::terrain::Hollow { c: [c.x, c.y], rot: [1.0, 0.0], half: [1.0, 1.0], floor, round: true });
            cv.reindex();
        }
        let start = Vec3::new(c.x, terrain.height(c.x, c.y), c.y);
        let end = walk(&terrain, start, Vec2::X, 80);
        assert!(end.x - c.x > 2.5, "walked out of the pit: {}", end.x - c.x);
    }
}
