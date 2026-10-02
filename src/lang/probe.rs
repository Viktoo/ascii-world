//! Step 4: probe a compiled type on the CPU evaluator at ~2,000 points in and
//! around its bounds.

use super::{CompiledType, Diag, Stage, vm::VmError};
use crate::noise::{pcg, u2f};

/// An sdf call may use at most this much fuel (keeps GPU cost bounded too).
pub const PROBE_FUEL_LIMIT: u32 = 8_000;
pub const PROBE_POINTS: usize = 2000;

#[derive(Clone, Debug)]
pub struct ProbeReport {
    /// Lowest and highest solid points found, local space.
    pub bottom: f32,
    pub top: f32,
    /// Points inside the shape (local space), for overlap checks.
    pub interior: Vec<[f32; 3]>,
    pub max_fuel: u32,
}

pub fn default_k(seed: f32) -> [f32; 8] {
    [seed, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]
}

/// The deterministic probe point set for a bounds box.
pub fn sample_points(b: [f32; 3]) -> Vec<[f32; 3]> {
    let mut pts = Vec::with_capacity(PROBE_POINTS);
    let n = 12;
    let ext = [b[0] * 1.5 + 0.3, b[1] * 1.5 + 0.3, b[2] * 1.5 + 0.3];
    for i in 0..n {
        for j in 0..n {
            for l in 0..n {
                let f = |q: usize, e: f32| ((q as f32 + 0.5) / n as f32 * 2.0 - 1.0) * e;
                pts.push([f(i, ext[0]), f(j, ext[1]), f(l, ext[2])]);
            }
        }
    }
    let mut s = 0x1234_5678u32;
    while pts.len() < PROBE_POINTS {
        let mut r = || {
            s = pcg(s);
            u2f(s) * 2.0 - 1.0
        };
        pts.push([r() * b[0], r() * b[1], r() * b[2]]);
    }
    pts
}

fn outside_bounds(p: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).any(|i| p[i].abs() > b[i] * 1.02 + 0.02)
}

fn fmt_p(p: [f32; 3]) -> String {
    format!("({:.2}, {:.2}, {:.2})", p[0], p[1], p[2])
}

pub fn probe(t: &CompiledType) -> Result<ProbeReport, Vec<Diag>> {
    probe_with(t, &default_k(0.37))
}

pub fn probe_with(t: &CompiledType, k: &[f32; 8]) -> Result<ProbeReport, Vec<Diag>> {
    let b = t.meta.bounds;
    let pts = sample_points(b);
    let mut diags = Vec::new();
    let mut vals = Vec::with_capacity(pts.len());
    let mut max_fuel = 0;
    let mut nan_reported = 0;
    for p in &pts {
        match t.sdf_checked(*p, k, PROBE_FUEL_LIMIT) {
            Ok((d, used)) => {
                max_fuel = max_fuel.max(used);
                if !d.is_finite() {
                    if nan_reported < 3 {
                        diags.push(Diag::new(Stage::Probe, 0, format!("sdf returns {} at {}", if d.is_nan() { "NaN" } else { "Infinity" }, fmt_p(*p))));
                    }
                    nan_reported += 1;
                }
                vals.push(d);
            }
            Err(VmError::OutOfFuel) => {
                return Err(vec![Diag::new(Stage::Probe, 0, format!("sdf is too expensive: more than {PROBE_FUEL_LIMIT} operations per call; simplify it or use fewer loop iterations"))]);
            }
            Err(VmError::NoReturn) => {
                return Err(vec![Diag::new(Stage::Probe, 0, "sdf finished without returning".into())]);
            }
        }
    }
    if nan_reported > 3 {
        diags.push(Diag::new(Stage::Probe, 0, format!("…and {} more non-finite results; guard divisions and sqrt of negatives", nan_reported - 3)));
    }
    if !diags.is_empty() {
        return Err(diags);
    }

    // Other seeds must stay finite too.
    for seed in [0.0f32, 0.91, 7.0] {
        let mut kk = *k;
        kk[0] = seed;
        for p in pts.iter().step_by(7) {
            match t.sdf_checked(*p, &kk, PROBE_FUEL_LIMIT) {
                Ok((d, _)) if d.is_finite() => {}
                Ok(_) => return Err(vec![Diag::new(Stage::Probe, 0, format!("sdf returns NaN/Infinity at {} when k.seed = {seed}", fmt_p(*p)))]),
                Err(_) => return Err(vec![Diag::new(Stage::Probe, 0, format!("sdf is too expensive when k.seed = {seed}"))]),
            }
        }
    }

    // Non-empty.
    let mut interior: Vec<[f32; 3]> = pts.iter().zip(&vals).filter(|(_, d)| **d < 0.0).map(|(p, _)| *p).collect();
    if interior.is_empty() {
        // Thin shapes can slip between grid points: descend from the closest sample.
        let (mut p, _) = pts.iter().zip(&vals).min_by(|a, b| a.1.total_cmp(b.1)).map(|(p, d)| (*p, *d)).unwrap_or(([0.0; 3], 1.0));
        for _ in 0..40 {
            let d = t.sdf(p, k);
            if d < 0.0 {
                interior.push(p);
                break;
            }
            let e = 0.01;
            let g = [
                t.sdf([p[0] + e, p[1], p[2]], k) - t.sdf([p[0] - e, p[1], p[2]], k),
                t.sdf([p[0], p[1] + e, p[2]], k) - t.sdf([p[0], p[1] - e, p[2]], k),
                t.sdf([p[0], p[1], p[2] + e], k) - t.sdf([p[0], p[1], p[2] - e], k),
            ];
            let gl = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt().max(1e-6);
            let step = d + 0.005;
            for i in 0..3 {
                p[i] -= g[i] / gl * step;
            }
        }
        if interior.is_empty() {
            let min = vals.iter().copied().fold(f32::MAX, f32::min);
            return Err(vec![Diag::new(Stage::Probe, 0, format!("shape is empty: sdf is never negative in or near the bounds (closest approach {min:.2} m)"))]);
        }
    }

    // Fits within bounds.
    let mut out_of_bounds = Vec::new();
    for (p, d) in pts.iter().zip(&vals) {
        if outside_bounds(*p, b) && *d <= 0.0 {
            out_of_bounds.push(*p);
        }
    }
    if !out_of_bounds.is_empty() {
        let mut ext = [0.0f32; 3];
        for p in &out_of_bounds {
            for i in 0..3 {
                ext[i] = ext[i].max(p[i].abs());
            }
        }
        return Err(vec![Diag::new(
            Stage::Probe,
            0,
            format!(
                "shape extends outside meta.bounds [{}, {}, {}]: solid at {} ({} samples); it reaches about [{:.1}, {:.1}, {:.1}]; enlarge bounds or shrink the shape",
                b[0], b[1], b[2], fmt_p(out_of_bounds[0]), out_of_bounds.len(), ext[0].max(b[0]), ext[1].max(b[1]), ext[2].max(b[2])
            ),
        )]);
    }

    // Colour must be valid wherever the surface can be hit.
    for (p, d) in pts.iter().zip(&vals) {
        if *d < 0.25 {
            match t.color_checked(*p, k, PROBE_FUEL_LIMIT) {
                Ok((c, _)) if c.iter().all(|v| v.is_finite() && (-1e-4..=1.0001).contains(v)) => {}
                Ok((c, _)) => {
                    return Err(vec![Diag::new(Stage::Probe, 0, format!("color returns an invalid colour ({}, {}, {}) at {}", c[0], c[1], c[2], fmt_p(*p)))]);
                }
                Err(VmError::OutOfFuel) => return Err(vec![Diag::new(Stage::Probe, 0, "color is too expensive".into())]),
                Err(VmError::NoReturn) => return Err(vec![Diag::new(Stage::Probe, 0, "color finished without returning a colour".into())]),
            }
        }
    }

    // Lowest solid point: refine downward from the lowest interior sample.
    let mut bottom = interior.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    if let Some(lp) = interior.iter().find(|p| p[1] == bottom).copied() {
        let mut lo = -b[1] - 0.05;
        let mut hi = lp[1];
        if t.sdf([lp[0], lo, lp[2]], k) >= 0.0 {
            for _ in 0..16 {
                let mid = (lo + hi) * 0.5;
                if t.sdf([lp[0], mid, lp[2]], k) < 0.0 { hi = mid } else { lo = mid }
            }
            bottom = hi;
        } else {
            bottom = lo;
        }
    }

    let mut top = interior.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    if let Some(hp) = interior.iter().find(|p| p[1] == top).copied() {
        let mut lo = hp[1];
        let mut hi = b[1] + 0.05;
        if t.sdf([hp[0], hi, hp[2]], k) >= 0.0 {
            for _ in 0..16 {
                let mid = (lo + hi) * 0.5;
                if t.sdf([hp[0], mid, hp[2]], k) < 0.0 { lo = mid } else { hi = mid }
            }
            top = lo;
        } else {
            top = hi;
        }
    }

    interior.truncate(400);
    Ok(ProbeReport { bottom, top, interior, max_fuel })
}
