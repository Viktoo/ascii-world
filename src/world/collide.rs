//! Collision against the CPU SDF of nearby solids, with sliding.

use super::Solid;
use crate::terrain::{Terrain, WATER_LEVEL};
use glam::{Vec2, Vec3};

pub const PLAYER_RADIUS: f32 = 0.35;
pub const NPC_RADIUS: f32 = 0.3;
/// Water deeper than this is impassable.
pub const MAX_WADE: f32 = 1.1;

pub struct Obstacles<'a> {
    pub solids: &'a [Solid],
    /// Other bodies (feet position) treated as vertical capsules.
    pub bodies: &'a [Vec3],
}

impl Obstacles<'_> {
    /// Distance from a body column at (x, z) to the nearest obstacle.
    pub fn dist(&self, terrain: &Terrain, x: f32, z: f32) -> f32 {
        let g = terrain.height(x, z);
        let mut d = f32::MAX;
        for s in self.solids {
            d = d.min(s.sdf(Vec3::new(x, g + 0.5, z))).min(s.sdf(Vec3::new(x, g + 1.4, z)));
        }
        for b in self.bodies {
            d = d.min(Vec2::new(b.x - x, b.z - z).length() - NPC_RADIUS);
        }
        d
    }
}

fn wadeable(terrain: &Terrain, x: f32, z: f32) -> bool {
    terrain.height(x, z) > WATER_LEVEL - MAX_WADE
}

/// Move a body by `delta` (XZ), sliding along surfaces. Returns the new feet position.
pub fn move_body(terrain: &Terrain, obs: &Obstacles, from: Vec3, delta: Vec3, radius: f32) -> Vec3 {
    let mut p = Vec2::new(from.x + delta.x, from.z + delta.z);
    if !wadeable(terrain, p.x, p.y) {
        // Slide along the shoreline: try each axis on its own.
        let px = Vec2::new(from.x + delta.x, from.z);
        let pz = Vec2::new(from.x, from.z + delta.z);
        p = if delta.x.abs() > 1e-5 && wadeable(terrain, px.x, px.y) {
            px
        } else if delta.z.abs() > 1e-5 && wadeable(terrain, pz.x, pz.y) {
            pz
        } else {
            Vec2::new(from.x, from.z)
        };
    }
    if !obs.solids.is_empty() || !obs.bodies.is_empty() {
        let d_start = obs.dist(terrain, from.x, from.z);
        if d_start < radius * 0.5 {
            // Already overlapping something (an old save, or a building that
            // appeared on top of us): move freely until clear, so we can walk out.
            return Vec3::new(p.x, terrain.height(p.x, p.y), p.y);
        }
        for _ in 0..5 {
            let d = obs.dist(terrain, p.x, p.y);
            if d >= radius {
                break;
            }
            let e = 0.04;
            let gx = obs.dist(terrain, p.x + e, p.y) - obs.dist(terrain, p.x - e, p.y);
            let gz = obs.dist(terrain, p.x, p.y + e) - obs.dist(terrain, p.x, p.y - e);
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
        let d_end = obs.dist(terrain, p.x, p.y);
        if d_end < radius * 0.5 && d_end < d_start {
            p = Vec2::new(from.x, from.z);
        }
    }
    Vec3::new(p.x, terrain.height(p.x, p.y), p.y)
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
        let ty = Arc::new(TypeEntry { id: 1, ct: Arc::new(ct), builtin: false, solid: true, bottom: -2.0, sphere_cy: 0.0, sphere_r: 4.7 });
        let terrain = Terrain::new(1, crate::terrain::default_biomes());
        let c = Vec3::new(500.0, 0.0, 500.0);
        let inst = GpuInst { pos_scale: [c.x, terrain.height(c.x, c.z) + 1.0, c.z, 1.0], rot: [1.0, 0.0, ty.radius(), 1.0], ..Default::default() };
        let solids = vec![Solid { inst, ty }];
        let obs = Obstacles { solids: &solids, bodies: &[] };
        // Standing inside: walking forward must work until we are out.
        let mut p = Vec3::new(c.x + 0.5, terrain.height(c.x, c.z), c.z);
        for _ in 0..80 {
            p = move_body(&terrain, &obs, p, Vec3::new(0.0, 0.0, 0.1), PLAYER_RADIUS);
        }
        assert!(p.z - c.z > 3.0 + PLAYER_RADIUS, "walked out of the hut: dz = {}", p.z - c.z);
        // And back towards it is blocked again.
        let before = p;
        for _ in 0..40 {
            p = move_body(&terrain, &obs, p, Vec3::new(0.0, 0.0, -0.1), PLAYER_RADIUS);
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
        let ty = Arc::new(TypeEntry { id: 1, ct: Arc::new(ct), builtin: false, solid: true, bottom: -2.0, sphere_cy: 0.0, sphere_r: 3.7 });
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
            p = move_body(&terrain, &obs, p, Vec3::new(0.02, 0.0, 0.1), PLAYER_RADIUS);
        }
        assert!(p.z - base.z < 4.5 - 0.3, "walked into the wall: z = {}", p.z - base.z);
        assert!(p.x - base.x > 2.0, "should have slid sideways along the wall: x = {}", p.x - base.x);
    }
}
