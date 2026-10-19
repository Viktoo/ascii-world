//! The stock loading backdrop: deep space, drawn pixel by pixel on the CPU.
//! A small light in the middle sends out a ring every few seconds. Each
//! ring brings a new handful of planets into being where it passes, and the
//! next one sweeps them away and brings others. The stars twinkle and drift,
//! as if the view were turning slowly.
//!
//! The planets come from a few templates made once, in the background, as
//! whole maps of their surface (an Earth, a Jupiter, a ringed giant, a Mars,
//! an ice giant, a moon). Each ring takes some of them and turns them into
//! someone new: another hue and depth of colour, a size, a spin, a tilt,
//! rings or none. Every planet is lit from the light in the middle, so its
//! far side is in shadow (and so is the part of its rings behind it).
//!
//! It is cheap: a planet is drawn once, off the main thread, when its ring
//! is coming; a frame is a copy of the still background, a few thousand
//! stars, the rings' thin bands and the planets laid over.

use crate::render::Frame;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

/// The widest it is drawn (the window scales it up).
const MAX_W: f32 = 1280.0;
/// Seconds between rings, and how long one is seen.
const PULSE: f32 = 3.0;
const RING_LIFE: f32 = 7.0;
/// A template's surface map, longitude × latitude.
#[cfg(not(test))]
const TEX_W: usize = 512;
#[cfg(test)]
const TEX_W: usize = 48;
const TEX_H: usize = TEX_W / 2;
/// Samples across a ring system, inner edge to outer.
const RING_N: usize = 160;
const RING_IN: f32 = 1.25;
const RING_OUT: f32 = 2.3;

fn hash(i: u32, j: u32) -> f32 {
    crate::noise::pcg(i.wrapping_mul(7919) ^ j.wrapping_mul(104_729) ^ 0x9e37_79b9) as f32 / u32::MAX as f32
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// From the colours as written (display) to light (linear), roughly.
fn lin(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| v * v)
}

/// Fractal 3D value noise, in about [0, 1].
fn fbm3(p: [f32; 3], s: u32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm) = (0.0, 0.5, 0.0);
    let mut q = p;
    for i in 0..octaves {
        sum += amp * crate::noise::vnoise3(q[0], q[1], q[2], s.wrapping_add(i * 101));
        norm += amp;
        amp *= 0.5;
        // Turned a little each octave so no grid shows.
        q = [q[0] * 1.6 + q[2] * 1.2 + 3.1, q[1] * 2.0 + 1.7, q[2] * 1.6 - q[0] * 1.2 + 5.3];
    }
    sum / norm
}

fn scale(p: [f32; 3], k: f32) -> [f32; 3] {
    p.map(|v| v * k)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Earth,
    Jovian,
    Ringed,
    Mars,
    Ice,
    Moon,
    Ocean,
}

const KINDS: [(Kind, f32); 7] = [(Kind::Earth, 0.17), (Kind::Jovian, 0.16), (Kind::Ringed, 0.15), (Kind::Mars, 0.13), (Kind::Ice, 0.12), (Kind::Moon, 0.14), (Kind::Ocean, 0.13)];

/// One planet as made: its surface (linear colour), its air, its rings.
struct Template {
    kind: Kind,
    albedo: Vec<[f32; 3]>,
    atm: [f32; 3],
    atm_k: f32,
    /// Colour (linear) and cover across the rings, inner to outer.
    rings: Vec<[f32; 4]>,
    /// How often one made from it wears its rings.
    ring_chance: f32,
}

/// Craters: directions on the sphere and sizes (radians).
fn craters(seed: u32, n: u32, big: f32) -> Vec<([f32; 3], f32)> {
    (0..n)
        .map(|i| {
            let (u, v) = (hash(seed, i * 3), hash(seed, i * 3 + 1));
            let (z, a) = (u * 2.0 - 1.0, v * TAU);
            let s = (1.0 - z * z).sqrt();
            ([s * a.cos(), z, s * a.sin()], 0.02 + hash(seed, i * 3 + 2).powi(3) * big)
        })
        .collect()
}

/// How a crater field marks a point: <1 in a floor, >1 on a rim.
fn cratered(p: [f32; 3], list: &[([f32; 3], f32)]) -> f32 {
    let mut k = 1.0;
    for (c, r) in list {
        let d = (p[0] * c[0] + p[1] * c[1] + p[2] * c[2]).clamp(-1.0, 1.0).acos() / r;
        if d < 1.25 {
            k *= if d < 1.0 { 0.82 + 0.1 * d * d } else { 1.0 + 0.12 * (1.0 - (d - 1.12).abs() / 0.13).max(0.0) };
        }
    }
    k
}

fn build(kind: Kind) -> Template {
    let mut albedo = Vec::with_capacity(TEX_W * TEX_H);
    let crater_list = match kind {
        Kind::Mars => craters(31, 40, 0.1),
        Kind::Moon => craters(41, 140, 0.16),
        _ => Vec::new(),
    };
    for j in 0..TEX_H {
        let lat = (j as f32 + 0.5) / TEX_H as f32 * PI - PI / 2.0;
        for i in 0..TEX_W {
            let lon = (i as f32 + 0.5) / TEX_W as f32 * TAU;
            let p = [lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin()];
            let c = match kind {
                Kind::Earth => {
                    let cont = fbm3(scale(p, 1.7), 11, 6);
                    let mut c = if cont < 0.53 {
                        mix([0.03, 0.10, 0.27], [0.08, 0.32, 0.50], smooth(0.38, 0.53, cont))
                    } else {
                        let dry = fbm3(scale(p, 3.0), 12, 4) + (1.0 - (lat.abs() - 0.35).abs() * 2.5).max(0.0) * 0.25;
                        let green = mix([0.16, 0.34, 0.10], [0.30, 0.38, 0.16], fbm3(scale(p, 7.0), 16, 3));
                        let land = mix(green, [0.68, 0.56, 0.36], smooth(0.55, 0.72, dry));
                        mix(land, [0.42, 0.35, 0.28], smooth(0.62, 0.78, fbm3(scale(p, 5.0), 13, 4)) * smooth(0.55, 0.62, cont))
                    };
                    let cap = 0.82 + (fbm3(scale(p, 4.0), 14, 3) - 0.5) * 0.18;
                    if lat.abs() / (PI / 2.0) > cap {
                        c = [0.90, 0.93, 0.97];
                    }
                    let cloud = fbm3([p[0] * 2.4, p[1] * 5.0, p[2] * 2.4], 15, 6);
                    mix(c, [0.96, 0.97, 0.99], smooth(0.5, 0.68, cloud) * 0.92)
                }
                Kind::Jovian | Kind::Ringed => {
                    let turb = fbm3(scale(p, 3.2), 21, 5) - 0.5;
                    let fine = fbm3(scale(p, 14.0), 22, 3) - 0.5;
                    let (bands, warp) = if kind == Kind::Jovian { (7.5, 1.5) } else { (10.0, 0.5) };
                    let b = p[1] * bands + turb * warp;
                    let t = 0.5 + 0.5 * (b * PI).sin();
                    let u = 0.5 + 0.5 * (b * PI * 0.37 + 1.0).sin();
                    let mut c = if kind == Kind::Jovian {
                        mix(mix([0.93, 0.88, 0.77], [0.62, 0.42, 0.30], t), mix([0.80, 0.60, 0.42], [0.96, 0.94, 0.90], t), u * 0.6)
                    } else {
                        mix(mix([0.93, 0.85, 0.64], [0.80, 0.68, 0.48], t), [0.96, 0.91, 0.76], u * 0.4)
                    };
                    c = c.map(|v| (v + fine * 0.07).clamp(0.0, 1.0));
                    if kind == Kind::Jovian {
                        // A great storm, red, in the south.
                        let dl = ((lon - 1.2 + PI).rem_euclid(TAU) - PI) * lat.cos();
                        let e = (dl / 0.24).powi(2) + ((lat + 0.36) / 0.11).powi(2);
                        if e < 1.6 {
                            let swirl = fbm3(scale(p, 9.0), 23, 3);
                            c = mix(c, if e < 1.0 { [0.78, 0.36, 0.24] } else { [0.95, 0.90, 0.82] }, (1.0 - e / 1.6).powf(0.6) * (0.65 + 0.35 * swirl));
                        }
                    }
                    c
                }
                Kind::Mars => {
                    let n = fbm3(scale(p, 2.2), 31, 6);
                    let mut c = mix([0.72, 0.38, 0.20], [0.40, 0.22, 0.14], smooth(0.45, 0.7, n));
                    c = mix(c, [0.82, 0.55, 0.36], smooth(0.62, 0.8, fbm3(scale(p, 6.0), 32, 3)) * 0.4);
                    c = c.map(|v| (v * cratered(p, &crater_list)).min(1.0));
                    if lat.abs() > 1.32 + (fbm3(scale(p, 5.0), 33, 3) - 0.5) * 0.12 {
                        c = [0.92, 0.90, 0.88];
                    }
                    c
                }
                Kind::Ice => {
                    let turb = fbm3(scale(p, 2.5), 51, 5) - 0.5;
                    let b = p[1] * 5.0 + turb * 0.6;
                    let mut c = mix([0.26, 0.48, 0.86], [0.18, 0.34, 0.72], 0.5 + 0.5 * (b * PI).sin());
                    let streak = fbm3([p[0] * 1.5, p[1] * 9.0, p[2] * 1.5], 52, 4);
                    c = mix(c, [0.86, 0.92, 1.0], smooth(0.66, 0.78, streak) * 0.8);
                    let dl = ((lon - 4.0 + PI).rem_euclid(TAU) - PI) * lat.cos();
                    let e = (dl / 0.2).powi(2) + ((lat + 0.3) / 0.09).powi(2);
                    mix(c, [0.08, 0.16, 0.42], (1.0 - e).max(0.0) * 0.8)
                }
                Kind::Ocean => {
                    // Water all over: deep and shallow, and storms wheeling in its clouds.
                    let depth = fbm3(scale(p, 2.0), 71, 5);
                    let c = mix([0.02, 0.09, 0.30], [0.06, 0.36, 0.58], smooth(0.35, 0.7, depth));
                    let swirl = fbm3([p[0] * 2.0 + p[1] * 1.5, p[1] * 6.0, p[2] * 2.0 - p[1] * 1.5], 72, 6);
                    let storm = fbm3(scale(p, 1.1), 73, 3);
                    mix(c, [0.95, 0.97, 1.0], smooth(0.52, 0.7, swirl * 0.7 + storm * 0.3) * 0.9)
                }
                Kind::Moon => {
                    let n = fbm3(scale(p, 1.4), 41, 5);
                    let c = mix([0.66, 0.65, 0.62], [0.36, 0.36, 0.36], smooth(0.52, 0.42, n));
                    let grit = fbm3(scale(p, 16.0), 42, 3) - 0.5;
                    c.map(|v| ((v + grit * 0.08) * cratered(p, &crater_list)).clamp(0.0, 1.0))
                }
            };
            albedo.push(lin(c));
        }
    }
    let rings = (0..RING_N)
        .map(|i| {
            let q = i as f32 / (RING_N - 1) as f32;
            let s = if kind == Kind::Ringed { 61 } else { 62 };
            let fine = crate::noise::vnoise2(q * 60.0, 0.5, s);
            let broad = crate::noise::vnoise2(q * 9.0, 2.5, s + 1);
            // A dark gap two thirds out, and edges that thin.
            let gap = 1.0 - (1.0 - ((q - 0.64) / 0.025).abs()).max(0.0) * 0.95;
            let edge = smooth(0.0, 0.08, q) * smooth(1.0, 0.85, q);
            let cover = (0.25 + 0.5 * broad + 0.25 * fine) * gap * edge;
            let c = lin(mix([0.86, 0.78, 0.62], [0.62, 0.55, 0.46], broad * 0.7 + fine * 0.3));
            let cover = if kind == Kind::Ringed { cover * 0.9 } else { cover * 0.35 };
            [c[0], c[1], c[2], cover]
        })
        .collect();
    let (atm, atm_k, ring_chance) = match kind {
        Kind::Earth => ([0.30, 0.55, 1.0], 1.0, 0.0),
        Kind::Jovian => ([0.95, 0.85, 0.70], 0.3, 0.3),
        Kind::Ringed => ([0.95, 0.88, 0.70], 0.3, 1.0),
        Kind::Mars => ([1.0, 0.62, 0.42], 0.35, 0.0),
        Kind::Ice => ([0.50, 0.78, 1.0], 0.7, 0.25),
        Kind::Moon => ([0.0, 0.0, 0.0], 0.0, 0.0),
        Kind::Ocean => ([0.35, 0.62, 1.0], 0.9, 0.0),
    };
    Template { kind, albedo, atm: lin(atm), atm_k, rings, ring_chance }
}

/// A planet drawn: premultiplied colour (as shown), placed with its top
/// left at `x0, y0`; its middle and how far it reaches.
struct Sprite {
    x0: i32,
    y0: i32,
    w: u32,
    h: u32,
    px: Vec<[f32; 4]>,
    mid: (f32, f32),
    reach: f32,
}

/// One planet made from a template.
struct Spec {
    x: f32,
    y: f32,
    r: f32,
    hue: f32,
    sat: f32,
    bright: f32,
    spin: f32,
    tilt: f32,
    rings: Option<(f32, f32)>,
}

/// Turn a colour round the grey axis by `h` radians.
fn hue_mat(h: f32) -> [[f32; 3]; 3] {
    let (c, s) = (h.cos(), h.sin());
    [
        [0.299 + 0.701 * c + 0.168 * s, 0.587 - 0.587 * c + 0.330 * s, 0.114 - 0.114 * c - 0.497 * s],
        [0.299 - 0.299 * c - 0.328 * s, 0.587 + 0.413 * c + 0.035 * s, 0.114 - 0.114 * c + 0.292 * s],
        [0.299 - 0.300 * c + 1.250 * s, 0.587 - 0.588 * c - 1.050 * s, 0.114 + 0.886 * c - 0.203 * s],
    ]
}

fn sample(t: &Template, lon: f32, lat: f32) -> [f32; 3] {
    let u = (lon / TAU).rem_euclid(1.0) * TEX_W as f32 - 0.5;
    let v = ((lat + PI / 2.0) / PI * TEX_H as f32 - 0.5).clamp(0.0, TEX_H as f32 - 1.001);
    let (i0, j0) = (u.floor(), v.floor());
    let (fu, fv) = (u - i0, v - j0);
    let i0 = (i0 as i32).rem_euclid(TEX_W as i32) as usize;
    let i1 = (i0 + 1) % TEX_W;
    let j0 = j0 as usize;
    let j1 = (j0 + 1).min(TEX_H - 1);
    let at = |i: usize, j: usize| t.albedo[j * TEX_W + i];
    let a = mix(at(i0, j0), at(i1, j0), fu);
    let b = mix(at(i0, j1), at(i1, j1), fu);
    mix(a, b, fv)
}

/// Draw one planet, lit from `light` (the middle of the picture).
fn draw_planet(t: &Template, s: &Spec, light: (f32, f32)) -> Sprite {
    let (lx, ly) = (light.0 - s.x, light.1 - s.y);
    let ld = (lx * lx + ly * ly).sqrt().max(1.0);
    // The light is in the picture's plane, a little towards the viewer.
    let lz = 0.12f32;
    let ln = (1.0 + lz * lz).sqrt();
    let l = [lx / ld / ln, ly / ld / ln, lz / ln];
    let reach = if s.rings.is_some() { RING_OUT + 0.05 } else { 1.12 };
    let e = s.r * reach;
    let (x0, y0) = ((s.x - e).floor() as i32, (s.y - e).floor() as i32);
    let size = (2.0 * e).ceil() as u32 + 2;
    let m = hue_mat(s.hue);
    let (tc, ts) = (s.tilt.cos(), s.tilt.sin());
    let ring = s.rings.map(|(a, open)| (a.cos(), a.sin(), open, (1.0 - open * open).sqrt()));
    let mut px = Vec::with_capacity((size * size) as usize);
    let tone = |c: [f32; 3]| -> [f32; 3] {
        let c = [0, 1, 2].map(|i| m[i][0] * c[0] + m[i][1] * c[1] + m[i][2] * c[2]);
        let g = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        c.map(|v| ((g + (v - g) * s.sat) * s.bright).max(0.0))
    };
    let atm = tone(t.atm);
    for py in 0..size {
        for qx in 0..size {
            let sx = ((x0 + qx as i32) as f32 + 0.5 - s.x) / s.r;
            let sy = ((y0 + py as i32) as f32 + 0.5 - s.y) / s.r;
            let rr = sx * sx + sy * sy;
            let d = rr.sqrt();
            // The planet.
            let mut body = [0.0f32; 4];
            if d < 1.0 + 1.0 / s.r {
                let nz = (1.0 - rr).max(0.0).sqrt();
                let ndl = sx * l[0] + sy * l[1] + nz * l[2];
                let (bx, by) = (sx * tc + sy * ts, -sx * ts + sy * tc);
                let lon = bx.atan2(nz) + s.spin;
                let lat = (-by).clamp(-1.0, 1.0).asin();
                let a = tone(sample(t, lon, lat));
                let shade = ((ndl + 0.03) / 1.03).max(0.0).powf(0.85);
                let rim = (1.0 - nz).powf(2.5) * t.atm_k * smooth(-0.15, 0.45, ndl);
                let c = [0, 1, 2].map(|i| a[i] * (shade * 1.35 + 0.004) + atm[i] * rim * 0.7);
                let cover = ((1.0 - d) * s.r + 0.5).clamp(0.0, 1.0);
                body = [c[0] * cover, c[1] * cover, c[2] * cover, cover];
            } else if t.atm_k > 0.0 && d < 1.09 {
                // Its air, a thin glow at the lit edge.
                let k = (1.0 - (d - 1.0) / 0.09).powi(2) * t.atm_k * 0.45 * smooth(-0.3, 0.6, (sx * l[0] + sy * l[1]) / d);
                body = [atm[0] * k, atm[1] * k, atm[2] * k, k * 0.5];
            }
            // Its rings, with the planet's shadow across them.
            let mut band = [0.0f32; 4];
            let mut front = false;
            if let Some((rc, rs, open, depth)) = ring {
                let (u, v) = (sx * rc + sy * rs, -sx * rs + sy * rc);
                let vv = v / open;
                let q = (u * u + vv * vv).sqrt();
                if (RING_IN..RING_OUT).contains(&q) {
                    let k = ((q - RING_IN) / (RING_OUT - RING_IN) * (RING_N - 1) as f32) as usize;
                    let [r, g, b, cover] = t.rings[k.min(RING_N - 1)];
                    front = vv > 0.0;
                    // Where it is in space, and whether the planet stands between it and the light.
                    let p = [sx, sy, vv * depth];
                    let along = p[0] * l[0] + p[1] * l[1] + p[2] * l[2];
                    let off = [p[0] - along * l[0], p[1] - along * l[1], p[2] - along * l[2]];
                    let shadow = along < 0.0 && (off[0] * off[0] + off[1] * off[1] + off[2] * off[2]) < 1.0;
                    let lit = if shadow { 0.03 } else { 0.95 };
                    let c = tone([r, g, b]);
                    band = [c[0] * lit * cover, c[1] * lit * cover, c[2] * lit * cover, cover];
                }
            }
            let (top, under) = if front { (band, body) } else { (body, band) };
            let a = top[3] + under[3] * (1.0 - top[3]);
            let lin = [0, 1, 2].map(|i| top[i] + under[i] * (1.0 - top[3]));
            // To colours as shown, still premultiplied.
            let shown = if a > 0.0 { lin.map(|v| (v / a).sqrt().min(1.0) * a) } else { [0.0; 3] };
            px.push([shown[0], shown[1], shown[2], a]);
        }
    }
    Sprite { x0, y0, w: size, h: size, px, mid: (s.x, s.y), reach: e }
}

/// The planets one ring brings, for a picture `w` × `h` lit from `light`.
fn generation(templates: &[Template], g: u64, w: u32, h: u32, light: (f32, f32)) -> Vec<Sprite> {
    let (wf, hf) = (w as f32, h as f32);
    let gs = g as u32 * 977 + 13;
    let n = 2 + (hash(gs, 1) * 3.0) as u32;
    let mut placed: Vec<(f32, f32, f32)> = Vec::new();
    let mut used: Vec<usize> = Vec::new();
    let mut out = Vec::new();
    for i in 0..n {
        let k = gs * 31 + i * 7;
        let pick = hash(k, 2);
        let mut acc = 0.0;
        let ti = KINDS.iter().position(|(_, p)| {
            acc += p;
            pick < acc
        });
        let mut ti = ti.unwrap_or(0);
        // No two of a kind in one ring's set.
        while used.contains(&ti) {
            ti = (ti + 1) % KINDS.len();
        }
        used.push(ti);
        let t = &templates[ti];
        let rings = (hash(k, 3) < t.ring_chance).then(|| ((hash(k, 4) - 0.5) * 0.9, 0.16 + hash(k, 5) * 0.3));
        let r = hf * (0.022 + 0.09 * hash(k, 6).powf(2.2));
        let e = r * if rings.is_some() { RING_OUT } else { 1.1 };
        // Somewhere in the picture, clear of the light and of the others.
        let mut at = None;
        for tr in 0..16u32 {
            let (x, y) = (e + hash(k, 20 + tr) * (wf - 2.0 * e), e + hash(k, 40 + tr) * (hf - 2.0 * e));
            let dl = ((x - light.0).powi(2) + (y - light.1).powi(2)).sqrt();
            if dl < e + hf * 0.12 {
                continue;
            }
            if placed.iter().all(|&(px, py, pe)| ((x - px).powi(2) + (y - py).powi(2)).sqrt() > pe + e + hf * 0.03) {
                at = Some((x, y));
                break;
            }
        }
        let Some((x, y)) = at else { continue };
        placed.push((x, y, e));
        let hue_range = match t.kind {
            Kind::Earth | Kind::Ocean => 0.25,
            Kind::Moon => 0.1,
            _ => 0.9,
        };
        let spec = Spec {
            x,
            y,
            r,
            hue: (hash(k, 7) - 0.5) * hue_range,
            sat: 0.75 + hash(k, 8) * 0.5,
            bright: 0.85 + hash(k, 9) * 0.3,
            spin: hash(k, 10) * TAU,
            tilt: (hash(k, 11) - 0.5) * 0.9,
            rings,
        };
        out.push(draw_planet(t, &spec, light));
    }
    out
}

struct Star {
    x: f32,
    y: f32,
    c: [f32; 3],
    b: f32,
    f: [f32; 2],
    ph: [f32; 2],
    depth: f32,
}

fn star_color(u: f32) -> [f32; 3] {
    const C: [[f32; 3]; 5] = [[0.70, 0.80, 1.0], [0.86, 0.91, 1.0], [1.0, 0.98, 0.95], [1.0, 0.89, 0.74], [1.0, 0.75, 0.56]];
    let w = [0.12, 0.3, 0.33, 0.17, 0.08];
    let mut acc = 0.0;
    for (c, w) in C.iter().zip(w) {
        acc += w;
        if u < acc {
            return *c;
        }
    }
    C[4]
}

fn add(px: &mut u32, c: [f32; 3], k: f32) {
    let p = *px;
    let ch = |s: u32, v: f32| (((p >> s) & 0xff) as f32 + v * k * 255.0).min(255.0) as u32;
    *px = ch(0, c[0]) | (ch(8, c[1]) << 8) | (ch(16, c[2]) << 16) | (255 << 24);
}

pub struct Cosmos {
    size: (u32, u32),
    bg: Vec<u32>,
    core: Vec<f32>,
    core_r: i32,
    stars: Vec<Star>,
    ask: Option<crossbeam_channel::Sender<(u64, u32, u32)>>,
    made: crossbeam_channel::Receiver<(u64, (u32, u32), Vec<Sprite>)>,
    gens: HashMap<u64, Vec<Sprite>>,
    asked: Vec<u64>,
    frame: Frame,
}

impl Cosmos {
    /// Start making the templates in the background (planets come once they are).
    pub fn new() -> Cosmos {
        let (atx, arx) = crossbeam_channel::unbounded::<(u64, u32, u32)>();
        let (mtx, mrx) = crossbeam_channel::unbounded();
        let _ = std::thread::Builder::new().name("cosmos".into()).spawn(move || {
            let templates: Vec<Template> = KINDS.iter().map(|(k, _)| build(*k)).collect();
            while let Ok((g, w, h)) = arx.recv() {
                let light = light_at(w, h);
                if mtx.send((g, (w, h), generation(&templates, g, w, h, light))).is_err() {
                    break;
                }
            }
        });
        Cosmos { size: (0, 0), bg: Vec::new(), core: Vec::new(), core_r: 0, stars: Vec::new(), ask: Some(atx), made: mrx, gens: HashMap::new(), asked: Vec::new(), frame: Frame::default() }
    }

    fn make(&mut self, w: u32, h: u32) {
        let (wf, hf) = (w as f32, h as f32);
        let mut bg = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                let (u, v) = (x as f32 / wf - 0.5, y as f32 / hf - 0.5);
                let vig = 1.0 - (u * u + v * v) * 0.9;
                // The dark, and the faintest drifts of far gas in it.
                let n = crate::noise::fbm2(x as f32 / (hf * 0.35), y as f32 / (hf * 0.35), 5, 4) * 0.5 + 0.5;
                let m = crate::noise::fbm2(x as f32 / (hf * 0.5) + 9.0, y as f32 / (hf * 0.5), 6, 3) * 0.5 + 0.5;
                let gas = smooth(0.5, 0.85, n);
                let c = mix([3.0, 4.0, 9.0], mix([14.0, 8.0, 22.0], [6.0, 14.0, 20.0], m), gas * 0.8).map(|c| c * vig);
                let d = hash(x * 3 + 1, y * 7 + 2) - 0.5;
                let [r, g, b] = c.map(|c| (c + d).clamp(0.0, 255.0) as u32);
                bg.push(r | (g << 8) | (b << 16) | (255 << 24));
            }
        }
        let count = (w * h / 300).clamp(60, 6000);
        let stars = (0..count)
            .map(|s| {
                let m = hash(s, 3).powf(9.0);
                Star {
                    x: hash(s, 1),
                    y: hash(s, 2),
                    c: star_color(hash(s, 4)),
                    b: 0.05 + m * 1.3,
                    f: [0.8 + hash(s, 5) * 2.8, 0.5 + hash(s, 6) * 1.7],
                    ph: [hash(s, 7) * TAU, hash(s, 8) * TAU],
                    depth: if hash(s, 9) < 0.6 { 0.35 } else { 1.0 },
                }
            })
            .collect();
        // The light: no point, only a soft haze, densest in the middle.
        let rc = (hf * 0.022).max(1.5);
        let core_r = (hf * 0.12) as i32;
        let mut core = Vec::new();
        for dy in -core_r..=core_r {
            for dx in -core_r..=core_r {
                let d2 = (dx * dx + dy * dy) as f32;
                let fade = (1.0 - d2.sqrt() / core_r as f32).max(0.0);
                core.push((0.22 * (-d2 / (rc * rc)).exp() + 0.1 * (-d2 / (6.0 * rc * rc)).exp()) * fade * fade);
            }
        }
        self.size = (w, h);
        self.bg = bg;
        self.stars = stars;
        self.core = core;
        self.core_r = core_r;
        self.gens.clear();
        self.asked.clear();
        while self.made.try_recv().is_ok() {}
    }

    /// The picture at time `t` (seconds), for a window `px` pixels big.
    pub fn frame(&mut self, px: (f32, f32), t: f32) -> &Frame {
        let k = (MAX_W / px.0.max(1.0)).min(1.0);
        let (w, h) = (((px.0 * k).round() as u32).max(16), ((px.1 * k).round() as u32).max(16));
        if self.size != (w, h) {
            self.make(w, h);
        }
        let (wf, hf) = (w as f32, h as f32);
        let light = light_at(w, h);
        let sc = hf / 800.0;
        let f = &mut self.frame;
        f.width = w;
        f.height = h;
        f.pixels.clear();
        f.pixels.extend_from_slice(&self.bg);
        let img = &mut f.pixels;
        // The stars, drifting as the view turns, each twinkling its own way.
        for s in &self.stars {
            let x = (s.x * wf + t * 3.0 * sc * s.depth).rem_euclid(wf);
            let y = (s.y * hf + (t * 0.021).sin() * 9.0 * sc * s.depth).rem_euclid(hf);
            let tw = 1.0 + 0.4 * (t * s.f[0] + s.ph[0]).sin() * (t * s.f[1] + s.ph[1]).sin();
            let b = s.b * tw;
            let r = if b > 0.5 { 2 } else { 1 };
            let (cx, cy) = (x.floor() as i32, y.floor() as i32);
            for dy in -r..=r {
                for dx in -r..=r {
                    let (qx, qy) = (cx + dx, cy + dy);
                    if qx < 0 || qy < 0 || qx >= w as i32 || qy >= h as i32 {
                        continue;
                    }
                    let (ex, ey) = (qx as f32 + 0.5 - x, qy as f32 + 0.5 - y);
                    let g = (-(ex * ex + ey * ey) / 0.6).exp() + if b > 0.8 { 0.04 * (-(ex * ex + ey * ey) / 4.0).exp() } else { 0.0 };
                    add(&mut img[(qy as u32 * w + qx as u32) as usize], s.c, b * g);
                }
            }
        }
        // The rings, out from the light.
        let now = (t / PULSE).floor() as i64;
        let radius = |g: i64| -> f32 {
            let a = t - g as f32 * PULSE;
            if a < 0.0 { -1e9 } else { (wf * wf + hf * hf).sqrt() * 0.62 * (1.0 - (-a / 1.7).exp()) }
        };
        for g in ((now as f32 - RING_LIFE / PULSE).floor() as i64).max(0)..=now {
            let a = t - g as f32 * PULSE;
            if !(0.0..RING_LIFE).contains(&a) {
                continue;
            }
            let r = radius(g);
            // Its edge stays crisp; the glow behind it widens as it goes.
            let wd = (1.2 + a * 0.5) * sc.max(0.3);
            let trail = (34.0 + a * 46.0) * sc.max(0.3);
            let i = 1.1 * (1.0 - a / RING_LIFE).powf(1.5) * (a * 8.0).min(1.0);
            let (dmin, dmax) = ((r - trail).max(0.0), r + 5.0 * wd);
            // Its colours turn slowly round it: violet, cyan, gold.
            let (ca, sa) = ((t * 0.35 + g as f32).cos(), (t * 0.35 + g as f32).sin());
            for y in 0..h {
                let dy = y as f32 + 0.5 - light.1;
                if dy.abs() > dmax {
                    continue;
                }
                let xo = (dmax * dmax - dy * dy).sqrt();
                let xi = if dy.abs() < dmin { (dmin * dmin - dy * dy).sqrt() } else { 0.0 };
                for (a0, a1) in [(light.0 - xo, light.0 - xi), (light.0 + xi, light.0 + xo)] {
                    for x in (a0.max(0.0) as u32)..(a1.min(wf) as u32) {
                        let dx = x as f32 + 0.5 - light.0;
                        let d = (dx * dx + dy * dy).sqrt().max(1e-3);
                        let q = (d - r) / wd;
                        let (u, v) = (dx / d, dy / d);
                        let k1 = 0.5 + 0.5 * (u * ca + v * sa);
                        let k2 = 0.5 + 0.5 * (u * sa - v * ca);
                        let hue = mix(mix([0.62, 0.40, 1.0], [0.30, 0.86, 1.0], k1), [1.0, 0.80, 0.46], k2 * k2 * 0.8);
                        // The edge: bright and sharp, a cyan fringe outside it
                        // and a rose one just inside.
                        let edge = (1.0 - (q / 1.1).powi(2)).max(0.0).powi(3);
                        let outer = (1.0 - ((q - 1.3) / 1.0).powi(2)).max(0.0).powi(2) * 0.45;
                        let inner = (1.0 - ((q + 1.4) / 1.0).powi(2)).max(0.0).powi(2) * 0.35;
                        // The glow it leaves, fading towards the light.
                        let back = if d < r { (1.0 - (r - d) / trail).max(0.0).powi(2) * 0.55 } else { 0.0 };
                        // A soft bloom round the edge.
                        let bloom = (1.0 - (q / 5.0).powi(2)).max(0.0).powi(2) * 0.3;
                        let c = [0, 1, 2].map(|j| {
                            let white = [1.0, 0.97, 0.92][j];
                            white * edge + [0.45, 0.9, 1.0][j] * outer + [1.0, 0.5, 0.8][j] * inner + hue[j] * (back + bloom)
                        });
                        add(&mut img[(y * w + x) as usize], c, i);
                    }
                }
            }
            // Sparks it leaves behind, twinkling out.
            let gs = g as u32 * 131 + 7;
            for m in 0..(160.0 * sc.max(0.4)) as u32 {
                let ang = hash(gs, m) * TAU;
                let lag = hash(gs, m + 500).powf(1.5) * trail * 1.4;
                let rr = r - lag - a * 4.0 * hash(gs, m + 900);
                if rr <= 0.0 {
                    continue;
                }
                let (x, y) = (light.0 + ang.cos() * rr, light.1 + ang.sin() * rr);
                let tw = (0.5 + 0.5 * (t * (6.0 + hash(gs, m + 77) * 9.0) + m as f32).sin()).powi(3);
                let b = i * tw * (1.0 - lag / (trail * 1.4)) * 2.2;
                let (cx, cy) = (x as i32, y as i32);
                let hue = mix([0.75, 0.6, 1.0], [0.6, 0.95, 1.0], hash(gs, m + 33));
                for (ox, oy, k) in [(0, 0, 1.0), (1, 0, 0.35), (-1, 0, 0.35), (0, 1, 0.35), (0, -1, 0.35)] {
                    let (qx, qy) = (cx + ox, cy + oy);
                    if qx >= 0 && qy >= 0 && qx < w as i32 && qy < h as i32 {
                        add(&mut img[(qy as u32 * w + qx as u32) as usize], hue, b * k);
                    }
                }
            }
        }
        // The planets: each ring brings its own and takes the last ring's.
        while let Ok((g, size, sprites)) = self.made.try_recv() {
            if size == (w, h) {
                self.gens.insert(g, sprites);
            }
        }
        for g in [now, now + 1] {
            if g >= 0 && !self.asked.contains(&(g as u64)) {
                if let Some(a) = &self.ask {
                    let _ = a.send((g as u64, w, h));
                }
                self.asked.push(g as u64);
            }
        }
        self.gens.retain(|g, _| *g as i64 >= now - 1);
        self.asked.retain(|g| *g as i64 >= now - 1);
        let soft = 10.0 * sc.max(0.3);
        for g in [now - 1, now] {
            let Some(sprites) = self.gens.get(&(g.max(0) as u64)).filter(|_| g >= 0) else { continue };
            let (rin, rout) = (radius(g), radius(g + 1));
            for s in sprites {
                let dm = ((s.mid.0 - light.0).powi(2) + (s.mid.1 - light.1).powi(2)).sqrt();
                let (near, far) = (dm - s.reach, dm + s.reach);
                if rin < near - soft || rout > far + soft {
                    continue;
                }
                let whole = rin > far + soft && rout < near - soft;
                for py in 0..s.h {
                    let y = s.y0 + py as i32;
                    if y < 0 || y >= h as i32 {
                        continue;
                    }
                    for qx in 0..s.w {
                        let x = s.x0 + qx as i32;
                        if x < 0 || x >= w as i32 {
                            continue;
                        }
                        let p = s.px[(py * s.w + qx) as usize];
                        if p[3] <= 0.002 {
                            continue;
                        }
                        let k = if whole {
                            1.0
                        } else {
                            let d = ((x as f32 + 0.5 - light.0).powi(2) + (y as f32 + 0.5 - light.1).powi(2)).sqrt();
                            ((rin - d) / soft + 0.5).clamp(0.0, 1.0) * (1.0 - ((rout - d) / soft + 0.5).clamp(0.0, 1.0))
                        };
                        if k <= 0.0 {
                            continue;
                        }
                        let o = &mut img[(y as u32 * w + x as u32) as usize];
                        let q = *o;
                        let keep = 1.0 - p[3] * k;
                        let ch = |sh: u32, v: f32| (((q >> sh) & 0xff) as f32 * keep + v * k * 255.0).min(255.0) as u32;
                        *o = ch(0, p[0]) | (ch(8, p[1]) << 8) | (ch(16, p[2]) << 16) | (255 << 24);
                    }
                }
            }
        }
        // The light itself, swelling a little as each ring leaves it, in
        // the rings' colours, drifting slowly from one to the next.
        let flare = 0.85 + 0.1 * (t * 0.9).sin() + 0.45 * (-(t.rem_euclid(PULSE)) / 0.5).exp();
        let tint = mix(mix([0.70, 0.52, 1.0], [0.42, 0.88, 1.0], 0.5 + 0.5 * (t * 0.21).cos()), [1.0, 0.84, 0.55], (0.5 + 0.5 * (t * 0.13).sin()).powi(2) * 0.7);
        let (lx, ly) = (light.0 as i32, light.1 as i32);
        let n = 2 * self.core_r + 1;
        for dy in -self.core_r..=self.core_r {
            for dx in -self.core_r..=self.core_r {
                let (x, y) = (lx + dx, ly + dy);
                if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                    continue;
                }
                let v = self.core[((dy + self.core_r) * n + dx + self.core_r) as usize];
                add(&mut img[(y as u32 * w + x as u32) as usize], tint, v * flare);
            }
        }
        f.id = f.id.wrapping_add(1);
        &self.frame
    }
}

impl Drop for Cosmos {
    fn drop(&mut self) {
        // The worker ends once nothing more can be asked of it.
        self.ask = None;
    }
}

/// Where the light stands: the middle.
fn light_at(w: u32, h: u32) -> (f32, f32) {
    (w as f32 * 0.5, h as f32 * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every template makes a planet lit on the side facing the light.
    #[test]
    fn planets_face_the_light() {
        for (kind, _) in KINDS {
            let t = build(kind);
            let spec = Spec { x: 100.0, y: 50.0, r: 20.0, hue: 0.3, sat: 1.0, bright: 1.0, spin: 1.0, tilt: 0.2, rings: (kind == Kind::Ringed).then_some((0.2, 0.3)) };
            // The light to the left.
            let s = draw_planet(&t, &spec, (0.0, 50.0));
            let at = |x: f32| {
                let (qx, qy) = ((x - s.x0 as f32) as u32, (50.0 - s.y0 as f32) as u32);
                let p = s.px[(qy * s.w + qx) as usize];
                p[0] + p[1] + p[2]
            };
            assert!(at(88.0) > at(112.0) + 0.1, "{kind:?}: lit towards the light");
        }
    }

    #[test]
    fn a_frame_draws_at_any_size() {
        let mut c = Cosmos::new();
        for (w, h) in [(40.0, 24.0), (320.0, 200.0)] {
            let f = c.frame((w, h), 4.1);
            assert_eq!(f.pixels.len(), (f.width * f.height) as usize);
        }
    }
}
