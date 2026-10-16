//! How a settlement shapes its land: the plan's `ground` list (lagoons,
//! islands, canals, streets, squares) read into ground edits, buildings
//! lined up along a way (`frontage`), and where ways cross cuts (bridges).
//! Pure geometry: `model` places what comes out of it.

use crate::ground::{Edit, Height, Mode, Shape, lay};
use glam::{Vec2, Vec3};
use serde_json::Value;

/// One thing the plan does to the land.
#[derive(Clone, Debug)]
pub struct LandPlan {
    pub edit: Edit,
    /// Its height follows the land (a street over a hill, a square on a
    /// slope): worked out when it is laid, from the ground under it then.
    pub follow: bool,
    /// Buildings lined up along it: a type name, which side (-1 left,
    /// 1 right, 0 both, looking along the line), and how far apart.
    pub frontage: Option<(String, i8, f32)>,
}

/// The most things one plan does to the land.
pub const MAX_PLANS: usize = 24;
const MAX_POINTS: usize = 64;

fn num(v: &Value) -> Option<f32> {
    v.as_f64().map(|x| x as f32).filter(|x| x.is_finite())
}

fn pt(v: &Value, ox: f32, oz: f32) -> Option<[f32; 2]> {
    let a = v.as_array()?;
    Some([ox + num(a.first()?)?.clamp(-64.0, 320.0), oz + num(a.get(1)?)?.clamp(-64.0, 320.0)])
}

fn color(v: Option<&Value>) -> Option<[f32; 3]> {
    let a = v?.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let c = |i: usize| (num(&a[i]).unwrap_or(128.0) / 255.0).clamp(0.0, 1.0);
    Some([c(0), c(1), c(2)])
}

/// Read a plan's `ground` list (region-local coordinates, offset by
/// `ox`, `oz`). Entries it can't make sense of are skipped.
pub fn parse(list: &[Value], ox: f32, oz: f32) -> Vec<LandPlan> {
    let mut out = Vec::new();
    for v in list.iter().take(MAX_PLANS) {
        let words = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").trim().to_lowercase();
        let width = v.get("width").and_then(num).unwrap_or(4.0).clamp(0.6, 60.0);
        let shape = if let Some(c) = v.get("circle").and_then(|c| c.as_array()).filter(|c| c.len() >= 3) {
            let (Some(x), Some(z), Some(r)) = (num(&c[0]), num(&c[1]), num(&c[2])) else { continue };
            Shape::Disc { c: [ox + x.clamp(-64.0, 320.0), oz + z.clamp(-64.0, 320.0)], r: r.clamp(1.0, 220.0) }
        } else if let Some(r) = v.get("rect").and_then(|c| c.as_array()).filter(|c| c.len() >= 4) {
            let f = |i: usize| r.get(i).and_then(num);
            let (Some(x), Some(z), Some(w), Some(d)) = (f(0), f(1), f(2), f(3)) else { continue };
            let a = f(4).unwrap_or(0.0).to_radians();
            let (hw, hd) = (w.abs().clamp(1.0, 300.0) * 0.5, d.abs().clamp(1.0, 300.0) * 0.5);
            let (c, s) = (a.cos(), a.sin());
            let corner = |u: f32, w: f32| [ox + x + u * c - w * s, oz + z + u * s + w * c];
            Shape::Area { pts: vec![corner(-hw, -hd), corner(hw, -hd), corner(hw, hd), corner(-hw, hd)] }
        } else if let Some(o) = v.get("outline").and_then(|c| c.as_array()) {
            let pts: Vec<[f32; 2]> = o.iter().take(MAX_POINTS).filter_map(|p| pt(p, ox, oz)).collect();
            if pts.len() < 3 {
                continue;
            }
            Shape::Area { pts }
        } else if let Some(l) = v.get("line").and_then(|c| c.as_array()) {
            let pts: Vec<[f32; 2]> = l.iter().take(MAX_POINTS).filter_map(|p| pt(p, ox, oz)).collect();
            if pts.len() < 2 {
                continue;
            }
            Shape::Line { pts, half: width * 0.5 }
        } else {
            continue;
        };
        let line = matches!(shape, Shape::Line { .. });
        let height = v.get("height").and_then(num).map(|h| h.clamp(-20.0, 80.0));
        let mode = match words("how").as_str() {
            "cut" | "lower" | "dig" | "dredge" => Mode::Lower,
            "fill" | "raise" | "build up" | "heap" => Mode::Raise,
            "level" | "flatten" | "pave" => Mode::Level,
            "paint" | "cover" | "none" => Mode::Paint,
            _ if height.is_none() && !line => Mode::Paint,
            _ => Mode::Level,
        };
        let rim = color(v.get("rim")).map(|c| (c, v.get("rim_width").and_then(num).unwrap_or(1.0).clamp(0.2, 6.0)));
        let edge = v.get("sides").and_then(num).map(|e| e.clamp(0.0, 40.0)).unwrap_or(match (mode, line) {
            (Mode::Lower, true) if rim.is_some() => 0.3,
            (Mode::Lower, true) => 1.5,
            (Mode::Lower, false) => 8.0,
            (Mode::Raise, _) => 1.0,
            _ => 1.5,
        });
        let follow = height.is_none() && mode != Mode::Paint;
        let frontage = v.get("frontage").filter(|f| f.is_object()).and_then(|f| {
            let ty = f.get("type").and_then(|t| t.as_str())?.trim().to_string();
            let sides = match f.get("sides").and_then(|s| s.as_str()).unwrap_or("both").trim() {
                "left" => -1,
                "right" => 1,
                _ => 0,
            };
            let every = f.get("every").and_then(num).unwrap_or(0.0).clamp(0.0, 60.0);
            (!ty.is_empty() && line).then_some((ty, sides, every))
        });
        let edit = Edit {
            kind: words("what"),
            name: v.get("name").and_then(|n| n.as_str()).unwrap_or("").trim().chars().take(60).collect(),
            shape,
            mode,
            to: Height::At(height.unwrap_or(0.0)),
            edge,
            color: color(v.get("color")),
            rim,
            water: v.get("water").and_then(num).map(|w| Height::At(w.clamp(-20.0, 80.0))),
        };
        out.push(LandPlan { edit, follow, frontage });
    }
    out
}

/// The height (x, z) has once `edits` are applied over the land `raw`,
/// worked out directly (the tiles do the same, for everyone, once laid).
pub fn shaped(edits: &[Edit], raw: &dyn Fn(f32, f32) -> f32, x: f32, z: f32) -> f32 {
    shaped_water(edits, raw, x, z).0
}

/// As `shaped`, and the water standing there (the sea's level at least).
pub fn shaped_water(edits: &[Edit], raw: &dyn Fn(f32, f32) -> f32, x: f32, z: f32) -> (f32, f32) {
    let mut h = raw(x, z);
    let (mut paint, mut water) = ((Vec3::ZERO, 0.0), crate::ground::NO_WATER);
    for e in edits {
        e.apply_at(x, z, &mut h, &mut paint, &mut water);
    }
    (h, water.max(crate::terrain::WATER_LEVEL))
}

/// Lay the plans in order: heights that follow the land are read off the
/// ground as shaped by the plans before them (a street on a raised island
/// sits on the island). Cuts along lines (canals, ditches) go last, after
/// the streets and `extra` (roads), so they cut through them and bridges
/// can cross them.
pub fn realize(plans: &[LandPlan], extra: &[LandPlan], raw: &dyn Fn(f32, f32) -> f32) -> Vec<Edit> {
    let mut done: Vec<Edit> = Vec::new();
    for p in plans.iter().filter(|p| !is_cut(&p.edit)).chain(extra) {
        done.push(follow(p, &done, raw));
    }
    for p in plans.iter().filter(|p| is_cut(&p.edit)) {
        done.push(follow(p, &done, raw));
    }
    done
}

/// A cut along a line: a canal, a ditch, a moat.
pub fn is_cut(e: &Edit) -> bool {
    e.mode == Mode::Lower && matches!(e.shape, Shape::Line { .. })
}

/// A way people walk or drive along: a line laid level (a street, a road).
pub fn is_way(e: &Edit) -> bool {
    matches!(e.shape, Shape::Line { .. }) && matches!(e.mode, Mode::Level | Mode::Raise)
}

/// A road between doors as a plan: laid along the ground as shaped.
pub fn road_plan(r: &crate::terrain::Road) -> LandPlan {
    let mut edit = r.edit(&|_, _| 0.0);
    edit.shape = Shape::Line { pts: vec![r.a, r.b], half: r.half };
    LandPlan { edit, follow: true, frontage: None }
}

/// Roads from doors that are off every way to the nearest point of one
/// (within `reach`), in the road's colour and width.
pub fn spurs(fronts: &[Vec3], ways: &[&Edit], color: [f32; 3], half: f32, reach: f32) -> Vec<crate::terrain::Road> {
    let mut out = Vec::new();
    for f in fronts {
        let best = ways
            .iter()
            .filter_map(|w| {
                let Shape::Line { pts, .. } = &w.shape else { return None };
                let (d, s) = w.dist(f.x, f.z);
                let i = (s as usize).min(pts.len().saturating_sub(2));
                let (a, b) = (Vec2::from(pts[i]), Vec2::from(pts[(i + 1).min(pts.len() - 1)]));
                Some((d, a + (b - a) * (s - i as f32)))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, p)) = best.filter(|b| b.0 > 1.0 && b.0 < reach) {
            out.push(crate::terrain::Road { a: [f.x, f.z], b: p.to_array(), half, color });
        }
    }
    out
}

fn follow(p: &LandPlan, before: &[Edit], raw: &dyn Fn(f32, f32) -> f32) -> Edit {
    let mut e = p.edit.clone();
    if !p.follow {
        return e;
    }
    let ground = |x: f32, z: f32| shaped(before, raw, x, z);
    match &e.shape {
        Shape::Line { pts, half } => {
            // A way stays above the water: across the sea it is a causeway;
            // over a river it keeps its height (and the river is cut through
            // again, under a bridge).
            let (pts, hs) = lay(pts, 3.0, 12.0, &|x, z| {
                let (h, w) = shaped_water(before, raw, x, z);
                h.max(w + 0.35)
            });
            let lower = e.mode == Mode::Lower;
            e.shape = Shape::Line { pts, half: *half };
            e.to = Height::Along(hs.into_iter().map(|h| if lower { h - 1.6 } else { h }).collect());
        }
        Shape::Area { pts } => {
            let n = pts.len() as f32;
            let c = pts.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
            let hs: Vec<f32> = pts.iter().chain(std::iter::once(&c)).map(|p| ground(p[0], p[1])).collect();
            e.to = Height::At(hs.iter().sum::<f32>() / hs.len() as f32);
        }
        Shape::Disc { c, r } => {
            let hs: Vec<f32> = (0..9).map(|i| if i == 8 { ground(c[0], c[1]) } else {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                ground(c[0] + a.cos() * r * 0.7, c[1] + a.sin() * r * 0.7)
            }).collect();
            e.to = Height::At(hs.iter().sum::<f32>() / hs.len() as f32);
        }
    }
    e
}

/// Where buildings stand along a line: spots `every` metres apart on the
/// chosen sides, set back past its edge (`clear`, from the middle) by half
/// a building's depth, each with the way it should face (toward the line).
pub fn frontage(pts: &[[f32; 2]], clear: f32, sides: i8, every: f32, width: f32, depth: f32) -> Vec<(Vec2, Vec2)> {
    let mut out = Vec::new();
    let step = every.max(width + 0.6);
    for side in [-1.0f32, 1.0] {
        if sides != 0 && sides as f32 != side {
            continue;
        }
        let mut carry = step * 0.5;
        for w in pts.windows(2) {
            let (a, b) = (Vec2::from(w[0]), Vec2::from(w[1]));
            let len = (b - a).length();
            if len < 0.01 {
                continue;
            }
            let dir = (b - a) / len;
            // Left of the direction of travel is (-dz, dx).
            let n = Vec2::new(-dir.y, dir.x) * side;
            let mut t = carry;
            while t <= len - width * 0.5 {
                if t >= width * 0.5 {
                    out.push((a + dir * t + n * (clear + depth * 0.5), -n));
                }
                t += step;
            }
            carry = (t - len).max(0.0);
        }
    }
    out
}

/// Where a way (`over`, not a cut) crosses a line cut (`cut`): each point,
/// the way's direction there, and how far the cut reaches across from its
/// middle (half its width, its rim and its sides).
pub fn crossings(over: &Edit, cut: &Edit) -> Vec<(Vec2, Vec2, f32)> {
    let (Shape::Line { pts: a, .. }, Shape::Line { pts: b, half }) = (&over.shape, &cut.shape) else { return Vec::new() };
    let reach = half + cut.rim.map(|r| r.1).unwrap_or(0.0) + cut.edge;
    let mut out: Vec<(Vec2, Vec2, f32)> = Vec::new();
    for s in a.windows(2) {
        let (p, p2) = (Vec2::from(s[0]), Vec2::from(s[1]));
        for t in b.windows(2) {
            let (q, q2) = (Vec2::from(t[0]), Vec2::from(t[1]));
            let (r, sv) = (p2 - p, q2 - q);
            let den = r.perp_dot(sv);
            if den.abs() < 1e-6 {
                continue;
            }
            let u = (q - p).perp_dot(sv) / den;
            let v = (q - p).perp_dot(r) / den;
            if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
                let x = p + r * u;
                if out.iter().all(|o| (o.0 - x).length() > 3.0) {
                    // Across a slanting cut a bridge has further to go.
                    let cos = r.normalize_or_zero().perp_dot(sv.normalize_or_zero()).abs().max(0.35);
                    out.push((x, r.normalize_or_zero(), reach / cos));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn flat(_: f32, _: f32) -> f32 {
        4.0
    }

    #[test]
    fn a_venetian_plan_reads_into_edits() {
        let plan = json!([
            { "what": "lagoon", "circle": [128, 128, 110], "how": "cut", "height": -1.8 },
            { "what": "island", "rect": [128, 128, 120, 90, 10], "how": "fill", "height": 0.8 },
            { "what": "canal", "name": "Rio Novo", "line": [[70, 120], [190, 135]], "width": 7, "how": "cut", "height": -1.6, "rim": [210, 205, 190], "color": [40, 50, 45],
              "frontage": { "type": "palazzo", "sides": "both", "every": 12 } },
            { "what": "street", "line": [[128, 80], [128, 180]], "width": 3, "color": [180, 170, 150] },
            { "what": "nonsense" },
            { "what": "square", "circle": [100, 100, 9] }
        ]);
        let p = parse(plan.as_array().unwrap(), 1000.0, -500.0);
        assert_eq!(p.len(), 5, "the shapeless one is skipped");
        assert_eq!(p[0].edit.mode, Mode::Lower);
        assert_eq!(p[0].edit.edge, 8.0, "a lagoon's shore shelves");
        assert!(matches!(&p[1].edit.shape, Shape::Area { pts } if pts.len() == 4 && pts.iter().all(|q| q[0] > 1000.0 && q[1] < -300.0)), "in world space");
        assert_eq!(p[2].edit.edge, 0.3, "a quayed canal has walls");
        assert_eq!(p[2].frontage, Some(("palazzo".into(), 0, 12.0)));
        assert!(p[3].follow && p[3].edit.mode == Mode::Level, "a street follows the land");
        assert_eq!(p[4].edit.mode, Mode::Paint, "a square with no height only paints");
        // Laid: the canal goes last and the street sits on the island.
        let edits = realize(&p, &[], &flat);
        assert!(is_cut(edits.last().unwrap()) && is_way(&edits[2]));
        assert_eq!(edits.last().unwrap().kind, "canal");
        let Height::Along(hs) = &edits[2].to else { panic!("{:?}", edits[2].to) };
        assert!(hs.iter().all(|h| (h - 0.8).abs() < 0.3 || *h >= 0.35), "on the island or a causeway: {hs:?}");
        assert!((shaped(&edits, &flat, 1100.0, -400.0) - 0.8).abs() < 0.2, "the island");
        assert!((shaped(&edits, &flat, 1128.0, -372.0) + 1.6).abs() < 0.05, "the canal through it");
    }

    #[test]
    fn frontage_lines_both_sides_facing_in() {
        let spots = frontage(&[[0.0, 0.0], [60.0, 0.0]], 5.0, 0, 10.0, 8.0, 6.0);
        let left: Vec<_> = spots.iter().filter(|s| s.0.y > 0.0).collect();
        let right: Vec<_> = spots.iter().filter(|s| s.0.y < 0.0).collect();
        assert_eq!(left.len(), right.len());
        assert!(left.len() >= 5, "{} a side", left.len());
        for (p, f) in &spots {
            assert!((p.y.abs() - 8.0).abs() < 1e-4, "set back past the edge: {p}");
            assert!(f.dot(-Vec2::new(0.0, p.y.signum())) > 0.99, "facing the line");
        }
        let mut xs: Vec<f32> = left.iter().map(|s| s.0.x).collect();
        xs.sort_by(f32::total_cmp);
        assert!(xs.windows(2).all(|w| w[1] - w[0] >= 9.99), "spaced out: {xs:?}");
        assert!(xs[0] >= 4.0 && *xs.last().unwrap() <= 56.0, "within the line: {xs:?}");
    }

    #[test]
    fn streets_cross_canals_at_bridges() {
        let e = |pts: Vec<[f32; 2]>, half: f32, mode: Mode| Edit { kind: String::new(), name: String::new(), shape: Shape::Line { pts, half }, mode, to: Height::At(0.0), edge: 0.3, color: None, rim: Some(([1.0; 3], 1.0)), water: None };
        let street = e(vec![[0.0, -30.0], [0.0, 30.0]], 1.5, Mode::Level);
        let canal = e(vec![[-50.0, 5.0], [50.0, 5.0]], 3.0, Mode::Lower);
        let x = crossings(&street, &canal);
        assert_eq!(x.len(), 1);
        assert!((x[0].0 - Vec2::new(0.0, 5.0)).length() < 1e-4);
        assert!((x[0].1 - Vec2::Y).length() < 1e-4);
        assert!((x[0].2 - 4.3).abs() < 1e-4, "half, rim and sides: {}", x[0].2);
        let away = e(vec![[-50.0, 50.0], [50.0, 60.0]], 3.0, Mode::Lower);
        assert!(crossings(&street, &away).is_empty());
    }
}
