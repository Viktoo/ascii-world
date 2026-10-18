//! Shapes set into the ground (`meta.in_ground`): a cliff dwelling, an earth
//! house, a bunker, a well. The earth is taken away exactly where the shape's
//! solid box is and stays round and over it (a box-shaped hollow in the
//! ground, like a cave's), so its walls meet the earth with no gap and the
//! hill stays over its roof. A doorway under the ground gets a way dug down
//! to it from the surface; a hatch sits level with the ground. Nothing here
//! knows what the shape is.

use crate::ground::{Cave, Part};
use crate::lang::ir::Anchor;
use crate::terrain::Terrain;
use crate::world::{Placed, TypeEntry};
use glam::{Vec2, Vec3};

/// What makes a cave a shape's.
pub const KIND: &str = "in_ground";

/// The box (local, unscaled) a shape's solid fills.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolidBox {
    pub lo: Vec3,
    pub hi: Vec3,
}

/// How far the earth reaches into a shape's walls (m), so no crack of air
/// shows between them.
const INSET: f32 = 0.03;

/// The box a shape's solid fills: traced in from each side of its bounds
/// (which may have a margin), its bottom and top as probed.
pub fn solid_box(e: &TypeEntry) -> SolidBox {
    let b = e.ct.meta.bounds;
    let k = crate::lang::probe::default_k(0.0);
    let sdf = |p: [f32; 3]| e.ct.sdf(p, &k);
    let (y0, y1) = if e.top > e.bottom { (e.bottom, e.top) } else { (-b[1], b[1]) };
    let mut lo = Vec3::new(-b[0], y0, -b[2]);
    let mut hi = Vec3::new(b[0], y1, b[2]);
    const N: usize = 9;
    for axis in [0usize, 2] {
        let other = 2 - axis;
        for sign in [-1.0f32, 1.0] {
            let mut ext = f32::MIN;
            for i in 0..N {
                for j in 0..N {
                    let u = ((i as f32 + 0.5) / N as f32 * 2.0 - 1.0) * b[other];
                    let y = y0 + (j as f32 + 0.5) / N as f32 * (y1 - y0);
                    let len = 2.0 * b[axis] + 0.2;
                    let mut t = 0.0;
                    while t < len {
                        let s = sign * (b[axis] + 0.1 - t);
                        let mut p = [0.0; 3];
                        p[axis] = s;
                        p[other] = u;
                        p[1] = y;
                        let d = sdf(p);
                        if d < 0.003 {
                            ext = ext.max(sign * s);
                            break;
                        }
                        // Short steps: a shape's distance may run long.
                        t += d.clamp(0.004, 0.2);
                    }
                }
            }
            if ext > f32::MIN {
                if sign > 0.0 {
                    hi[axis] = ext.min(b[axis]);
                } else {
                    lo[axis] = (-ext).max(-b[axis]);
                }
            }
        }
    }
    if hi.x - lo.x < 0.1 || hi.z - lo.z < 0.1 {
        return SolidBox { lo: Vec3::new(-b[0], y0, -b[2]), hi: Vec3::new(b[0], y1, b[2]) };
    }
    SolidBox { lo, hi }
}

/// Local (lx, lz) → world (x, z) for a shape at (x, z) turned `rot`.
fn world(x: f32, z: f32, rot: f32, l: Vec2) -> Vec2 {
    let (c, s) = (rot.cos(), rot.sin());
    Vec2::new(x + c * l.x + s * l.y, z - s * l.x + c * l.y)
}

fn door_of(e: &TypeEntry) -> Option<&Anchor> {
    e.ct.meta.anchors.iter().find(|a| a.kind == "door")
}

fn hatch_of(e: &TypeEntry) -> Option<&Anchor> {
    e.ct.meta.anchors.iter().find(|a| a.kind == "hatch")
}

/// Where a shape set into the ground stands: its origin's height and its
/// turn. Its floor lies `in_ground` metres under the ground before its
/// doorway (y = 0 is the ground's level where it has no doorway); a hatch
/// sits level with the ground. A doorway meant to open onto the surface is
/// turned to the open side of a hill when the plan's turn would face it
/// into the earth.
pub fn seat(t: &Terrain, e: &TypeEntry, sb: &SolidBox, x: f32, z: f32, rot: f32, scale: f32) -> (f32, f32) {
    let depth = e.ct.meta.in_ground.unwrap_or(0.0);
    if let Some(h) = hatch_of(e) {
        let p = world(x, z, rot, Vec2::new(h.at[0], h.at[2]) * scale);
        return (t.height(p.x, p.y) - h.at[1] * scale, rot);
    }
    let Some(door) = door_of(e) else { return (t.height(x, z) - depth, rot) };
    let front = |r: f32| {
        let out = door.out();
        let l = Vec2::new(door.at[0] + out[0] * 0.3, door.at[2] + out[2] * 0.3) * scale + Vec2::new(out[0], out[2]) * 0.9;
        world(x, z, r, l)
    };
    let ground_at = |p: Vec2| t.height(p.x, p.y);
    let mut rot = rot;
    if depth < 1.0 {
        // How buried its back is against its front: high in a hillside.
        let back = |r: f32| {
            let out = door.out();
            let reach = Vec2::new((sb.hi.x - sb.lo.x) * 0.35, (sb.hi.z - sb.lo.z) * 0.35);
            world(x, z, r, Vec2::new(-out[0] * reach.x, -out[2] * reach.y) * scale)
        };
        let score = |r: f32| ground_at(back(r)) - ground_at(front(r));
        let here = score(rot);
        let best = (1..8).map(|k| rot + k as f32 * std::f32::consts::FRAC_PI_4).map(|r| (score(r), r)).fold((here, rot), |a, b| if b.0 > a.0 { b } else { a });
        if best.0 > 1.0 && best.0 > here + 0.75 {
            rot = best.1;
        }
    }
    (ground_at(front(rot)) - depth - door.at[1] * scale, rot)
}

/// The hollow a placed shape set into the ground takes: its box, each
/// doorway's opening through the earth, and a way in from the surface (a
/// ramp down to a doorway under the ground, a shaft up from a hatch).
pub fn cave(t: &Terrain, p: &Placed, e: &TypeEntry, sb: &SolidBox) -> Cave {
    let g = p.gpu(e, 1.0);
    let s = p.scale.max(0.1);
    let centre = g.from_local((sb.lo + sb.hi) * 0.5);
    let half = ((sb.hi - sb.lo) * 0.5 * s - Vec3::splat(INSET)).max(Vec3::splat(0.06));
    let room = Part::boxed(centre, half, p.rot_y);
    let mut way: Vec<Part> = Vec::new();
    let mut openings: Vec<Part> = Vec::new();
    let mut mouth = None;
    for a in e.ct.meta.anchors.iter() {
        let [w, h] = a.size.unwrap_or([1.0, 2.0]).map(|v| v * s);
        match a.kind.as_str() {
            "door" => {
                let yaw = p.rot_y + a.face.to_radians();
                let at = g.from_local(Vec3::from_array(a.at));
                let out = g.from_local(Vec3::from_array(a.at) + Vec3::from_array(a.out())) - at;
                let dir = Vec2::new(out.x, out.z).normalize_or(Vec2::Y);
                openings.push(Part::boxed(at + Vec3::Y * (h * 0.5), Vec3::new(w * 0.5 + 0.05, h * 0.5, 0.7), yaw));
                if mouth.is_some() {
                    continue;
                }
                // Under the ground before it: a ramp up to the surface.
                let front = at + Vec3::new(dir.x, 0.0, dir.y) * 1.0;
                if t.height(front.x, front.z) > at.y + 0.3 {
                    const SLOPE: f32 = 0.5;
                    let r = (h * 0.62).clamp(1.15, 2.5);
                    let mut d = 0.5;
                    while d < 60.0 && t.height(at.x + dir.x * d, at.z + dir.y * d) > at.y + d * SLOPE + 0.02 {
                        d += 0.25;
                    }
                    let m = Vec3::new(at.x + dir.x * d, at.y + d * SLOPE, at.z + dir.y * d);
                    let a0 = at - Vec3::new(dir.x, 0.0, dir.y) * 0.2 + Vec3::Y * r;
                    let b0 = m + Vec3::new(dir.x, 0.0, dir.y) * 0.5 + Vec3::Y * (r - 0.05);
                    way.push(Part::capsule(b0, a0, r));
                    mouth = Some(m);
                } else {
                    mouth = Some(Vec3::new(front.x, t.height(front.x, front.z), front.z));
                }
            }
            "hatch" => {
                // A shaft from the hatch up through any earth over it.
                let at = g.from_local(Vec3::from_array(a.at));
                let d = a.size.map(|v| v[1] * s).unwrap_or(w);
                let top = t.height(at.x, at.z).max(at.y) + 0.6;
                let mid = (at.y - 0.3 + top) * 0.5;
                openings.push(Part::boxed(Vec3::new(at.x, mid, at.z), Vec3::new(w * 0.5 + 0.05, (top - at.y + 0.3) * 0.5, d * 0.5 + 0.05), p.rot_y + a.face.to_radians()));
                if mouth.is_none() {
                    mouth = Some(Vec3::new(at.x, t.height(at.x, at.z).max(at.y), at.z));
                }
            }
            _ => {}
        }
    }
    // Its way in first (people find their way by it), then its room.
    let mut parts = way;
    let first_opening = openings.first().copied();
    if parts.is_empty() {
        parts.extend(first_opening);
        parts.push(room);
        parts.extend(openings.into_iter().skip(1));
    } else {
        parts.push(room);
        parts.extend(openings);
    }
    let mouth = mouth.unwrap_or_else(|| Vec3::new(centre.x, t.height(centre.x, centre.z), centre.z));
    Cave { kind: KIND.into(), name: e.name().to_string(), parts, mouth: mouth.to_array() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(src: &str) -> TypeEntry {
        let ct = crate::lang::compile(src).expect("compiles");
        let rep = crate::lang::probe::probe(&ct).expect("probes");
        TypeEntry { id: 1, ct: std::sync::Arc::new(ct), builtin: false, solid: true, bottom: rep.bottom, top: rep.top, sphere_cy: 0.0, sphere_r: 6.0 }
    }

    const HUT: &str = r#"
export const meta = { name: "hut", bounds: [3.5, 3, 3.5], tags: ["building"], in_ground: 0,
  anchors: [{ kind: "door", at: [0, 0, 2.5], face: 0, size: [1, 2] }] };
export function sdf(x, y, z, k) {
  const shell = box(x, y - 1.3, z, 2.5, 1.3, 2.5);
  const room = box(x, y - 1.3, z, 2.2, 1.1, 2.2);
  const door = box(x, y - 1, z - 2.4, 0.5, 1, 0.3);
  return max(shell, -min(room, door));
}
export function color(x, y, z, k) { return rgb(150, 120, 90); }
"#;

    /// Land laid at 20 m over the sea, rising `slope` per metre northward (+z).
    fn land(slope: f32) -> Terrain {
        let t = Terrain::new(1, crate::terrain::default_biomes());
        let e = crate::ground::Edit { pattern: None,
            kind: "test".into(),
            name: String::new(),
            shape: crate::ground::Shape::Line { pts: vec![[0.0, -60.0], [0.0, 60.0]], half: 60.0 },
            mode: crate::ground::Mode::Level,
            to: crate::ground::Height::Along(vec![20.0 - 60.0 * slope, 20.0 + 60.0 * slope]),
            edge: 2.0,
            color: None,
            rim: None,
            water: None,
        };
        t.set_ground(crate::ground::Layer::Places, vec![e]);
        t
    }

    #[test]
    fn the_solid_box_is_snug_inside_loose_bounds() {
        let e = entry(HUT);
        let b = solid_box(&e);
        for v in [b.hi.x, b.hi.z, -b.lo.x, -b.lo.z] {
            assert!((v - 2.5).abs() < 0.03, "walls at 2.5 m, not the bounds' 3.5: {b:?}");
        }
    }

    #[test]
    fn a_hillside_home_turns_its_door_to_the_open_side() {
        // Ground rising to the north (+z): a door facing +z opens into it.
        let t = land(0.6);
        let e = entry(HUT);
        let sb = solid_box(&e);
        let (y, rot) = seat(&t, &e, &sb, 0.0, 0.0, 0.0, 1.0);
        let door_dir = Vec2::new(rot.sin(), rot.cos());
        assert!(door_dir.y < -0.6, "the door faces downhill: {door_dir:?}");
        // Its floor meets the ground before the door.
        let front = Vec2::ZERO + door_dir * 3.7;
        assert!((y - t.height(front.x, front.y)).abs() < 0.2, "floor {y:.2} at the ground before the door {:.2}", t.height(front.x, front.y));
    }

    #[test]
    fn a_home_under_flat_ground_gets_a_ramp_to_its_door() {
        let t = land(0.0);
        let src = HUT.replace("in_ground: 0", "in_ground: 4");
        let e = entry(&src);
        let sb = solid_box(&e);
        let (y, rot) = seat(&t, &e, &sb, 0.0, 0.0, 0.0, 1.0);
        assert!((y - 16.0).abs() < 0.01 && rot == 0.0, "4 m down, as turned: {y} {rot}");
        let p = Placed { id: 1, type_id: 1, pos: Vec3::new(0.0, y, 0.0), rot_y: rot, scale: 1.0, params: [0.0, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5], version: 0 };
        let c = cave(&t, &p, &e, &sb);
        assert!(c.sane());
        // The mouth is on the surface in front, about 8 m out at a 1:2 slope.
        assert!(c.mouth[2] > 8.0 && c.mouth[2] < 12.0 && (c.mouth[1] - 20.0).abs() < 0.3, "mouth {:?}", c.mouth);
        // The room is open air; a metre over its roof is earth; and the
        // earth meets its walls: just outside them is solid.
        assert!(c.dist(Vec3::new(0.0, y + 1.3, 0.0)) < 0.0);
        assert!(c.dist(Vec3::new(0.0, y + 3.6, -1.0)) > 0.0);
        assert!(c.dist(Vec3::new(2.53, y + 1.3, -1.0)) > 0.0 && c.dist(Vec3::new(2.45, y + 1.3, -1.0)) < 0.0);
        // The way down: open along the ramp's floor to the doorway.
        for d in [1.0f32, 3.0, 6.0] {
            assert!(c.dist(Vec3::new(0.0, y + d * 0.5 + 0.8, 2.5 + d)) < 0.0, "the ramp is open {d} m out");
        }
        assert!(c.dist(Vec3::new(0.0, y + 0.8, 2.5)) < 0.0, "the doorway is open");
    }
}
