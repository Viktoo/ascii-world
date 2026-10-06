//! CPU fallback renderer (no GPU adapter). Same scene description, evaluated
//! with the Rust terrain and the bytecode evaluator; simpler shading, meant
//! to run at reduced resolution.

use super::{Frame, FrameRequest, GpuInst, Globals, RenderHandle, RenderMsg};
use crate::terrain::{Terrain, WATER_LEVEL, smoothstep};
use crate::world::{SceneTypes, TypeEntry};
use glam::Vec3;
use std::sync::Arc;

struct Scene<'a> {
    g: &'a Globals,
    terrain: &'a Terrain,
    insts: Vec<(GpuInst, Arc<TypeEntry>)>,
}

fn v3(a: [f32; 4]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

impl Scene<'_> {
    fn inst_sdf(&self, i: usize, p: Vec3) -> f32 {
        let (gi, ty) = &self.insts[i];
        gi.sdf(&ty.ct, p)
    }

    fn map(&self, p: Vec3, cands: &[usize]) -> (f32, Option<usize>) {
        let mut d = (p.y - self.terrain.height(p.x, p.z)) * 0.6;
        let mut id = None;
        for &c in cands {
            let gi = &self.insts[c].0;
            let ds = (p - gi.center()).length() - gi.radius();
            if ds >= d {
                continue;
            }
            let dj = if ds < 0.25 { self.inst_sdf(c, p) } else { ds };
            if dj < d {
                d = dj;
                id = Some(c);
            }
        }
        (d, id)
    }

    fn sky(&self, rd: Vec3) -> Vec3 {
        let g = self.g;
        let up = rd.y.max(0.0);
        let mut col = v3(g.sky_hor).lerp(v3(g.sky_zen), up.sqrt());
        let s = rd.dot(v3(g.sun_dir)).max(0.0);
        col += v3(g.sun_col) * (s.powf(900.0) * 6.0 + s.powf(10.0) * 0.22) * g.sun_dir[3];
        // Sunset glow low toward the sun, and the clouds (simpler than the GPU's).
        let sun = v3(g.wx[5]);
        let toward = (glam::Vec2::new(rd.x, rd.z).normalize_or_zero().dot(glam::Vec2::new(sun.x, sun.z).normalize_or_zero())) * 0.5 + 0.5;
        col += v3(g.wx[6]) * g.wx[4][1] * (-up * 4.5).exp() * (0.12 + 0.88 * toward.powi(4)) * 0.9;
        if g.wx[0][0] > 0.01 && rd.y > 0.0 {
            let t = (g.wx[0][3] - g.cam_pos[1]).max(50.0) / rd.y.max(0.015);
            let d = cloud_density(g, g.cam_pos[0] + rd.x * t, g.cam_pos[2] + rd.z * t);
            if d > 0.002 {
                let day = g.wx[6][3];
                let lit = v3(g.sun_col) * day * 0.6 + v3(g.wx[6]) * g.wx[4][1] * 0.7 + v3(g.sky_hor) * 0.5;
                let c = v3(g.wx[1]) * lit * (1.0 - 0.45 * d * g.wx[0][0] - 0.35 * g.wx[4][3]);
                let far = smoothstep(0.01, 0.18, rd.y);
                col = col.lerp(c, d * (0.3 + 0.7 * far));
            }
        }
        col + Vec3::new(0.6, 0.65, 0.85) * g.wx[1][3] * 0.5
    }

    fn point_light(&self, p: Vec3, n: Vec3) -> Vec3 {
        let g = self.g;
        let mut sum = Vec3::ZERO;
        for i in 0..(g.grid1[2] as usize).min(super::MAX_LIGHTS) {
            let l = g.lights[i];
            let c = g.light_cols[i];
            let d = v3(l) - p;
            let dist = d.length();
            if dist > c[3] {
                continue;
            }
            let fall = 1.0 - smoothstep(c[3] * 0.4, c[3], dist);
            let ndl = n.dot(d / dist.max(1e-3)).max(0.0) * 0.8 + 0.2;
            let col = v3(c);
            sum += col * col * l[3] * ndl * fall / (1.0 + dist * dist * 0.3);
        }
        sum
    }

    fn shade(&self, p: Vec3, n: Vec3, albedo: Vec3) -> Vec3 {
        let g = self.g;
        let sun = v3(g.sun_dir);
        let ndl = n.dot(sun).max(0.0);
        let a = albedo * albedo;
        let amb = {
            let c = v3(g.sky_hor).lerp(v3(g.sky_zen), 0.5 + 0.5 * n.y);
            c * c * g.sky_hor[3]
        };
        let sc = v3(g.sun_col);
        let strength = (0.15 + 0.85 * g.sun_dir[3]).max(0.75 * g.sun_col[3]);
        let floor = Vec3::new(0.018, 0.022, 0.035) * g.sun_col[3];
        let lin = a * (sc * sc * ndl * strength * 1.6 + amb + floor + self.point_light(p, n));
        Vec3::new(lin.x.max(0.0).sqrt(), lin.y.max(0.0).sqrt(), lin.z.max(0.0).sqrt())
    }

    fn render(&self, ro: Vec3, rd: Vec3, ground: &dyn Fn(f32, f32, f32, f32) -> Vec3) -> Vec3 {
        let tmax = self.g.cam_fwd[3];
        let mut cands: Vec<usize> = Vec::new();
        for (i, (gi, _)) in self.insts.iter().enumerate() {
            let oc = ro - gi.center();
            let r = gi.radius();
            let b = oc.dot(rd);
            let c = oc.dot(oc) - r * r;
            let disc = b * b - c;
            if disc >= 0.0 && -b + disc.sqrt() >= 0.0 && -b - disc.sqrt() <= tmax {
                cands.push(i);
                if cands.len() >= 24 {
                    break;
                }
            }
        }
        let mut t = 0.06;
        let mut hit = None;
        for _ in 0..110 {
            let p = ro + rd * t;
            let (d, id) = self.map(p, &cands);
            if d < 0.002 * t + 0.003 {
                hit = Some(id);
                break;
            }
            t += d.max(0.02 + 0.004 * t);
            if t > tmax {
                break;
            }
        }
        let mut t_hit = f32::MAX;
        let mut col = match hit {
            None => self.sky(rd),
            Some(id) => {
                t_hit = t;
                let p = ro + rd * t;
                match id {
                    None => {
                        let n = self.terrain.normal(p.x, p.z);
                        self.shade(p, n, ground(p.x, p.z, p.y, n.y))
                    }
                    Some(i) => {
                        let e = 0.01;
                        let n = Vec3::new(
                            self.inst_sdf(i, p + Vec3::X * e) - self.inst_sdf(i, p - Vec3::X * e),
                            self.inst_sdf(i, p + Vec3::Y * e) - self.inst_sdf(i, p - Vec3::Y * e),
                            self.inst_sdf(i, p + Vec3::Z * e) - self.inst_sdf(i, p - Vec3::Z * e),
                        )
                        .normalize_or_zero();
                        let (gi, ty) = &self.insts[i];
                        let (c, g) = ty.ct.color_glow(gi.to_local(p), &gi.k());
                        let base = Vec3::from(c).clamp(Vec3::ZERO, Vec3::ONE);
                        let lit = if ty.ct.marks_glow() { g } else { 1.0 };
                        let fx = gi.fx;
                        let mut a = base.lerp(Vec3::new(0.07, 0.06, 0.055), fx[0].clamp(0.0, 1.0));
                        a *= 1.0 - 0.35 * fx[1].clamp(0.0, 1.0);
                        a = a.lerp(Vec3::new(1.0, 0.97, 0.8), fx[3].clamp(0.0, 1.0) * 0.28);
                        // The dark's own (charred past 1): whatever glows on it glows red.
                        let gc = if fx[0] > 1.5 { Vec3::new(1.0, 0.07, 0.03) } else { base };
                        self.shade(p, n, a) + gc * fx[2].clamp(0.0, 2.0) * lit
                    }
                }
            }
        };
        if rd.y < 0.0 && ro.y > WATER_LEVEL {
            let tw = (WATER_LEVEL - ro.y) / rd.y;
            if tw < t_hit && tw < tmax {
                let pw = ro + rd * tw;
                let depth = (WATER_LEVEL - self.terrain.height(pw.x, pw.z)).max(0.0);
                let body = self.shade(pw, Vec3::Y, v3(self.g.water));
                let under = col * Vec3::new(0.55, 0.75, 0.8);
                let fres = 0.03 + 0.97 * (1.0 - (-rd.y).max(0.0)).powi(5);
                col = under.lerp(body, (1.0 - (-depth * 0.7).exp()).clamp(0.15, 1.0)).lerp(self.sky(Vec3::new(rd.x, -rd.y, rd.z)), fres * 0.85);
                t_hit = tw;
            }
        }
        if t_hit < f32::MAX {
            let f = (1.0 - (-t_hit * 0.0045 * self.g.sky_zen[3]).exp()).max(smoothstep(tmax * 0.72, tmax, t_hit));
            col = col.lerp(v3(self.g.sky_hor), f.clamp(0.0, 1.0));
        }
        col.clamp(Vec3::ZERO, Vec3::ONE)
    }
}

/// Cloud over a point of the cloud layer; twin of `cloud_density` in scene.wgsl.
pub fn cloud_density(g: &Globals, x: f32, z: f32) -> f32 {
    let cover = g.wx[0][0];
    if cover < 0.01 {
        return 0.0;
    }
    let (qx, qz) = ((x - g.wx[0][1]) * 0.0021, (z - g.wx[0][2]) * 0.0021);
    let n = crate::noise::fbm2(qx, qz, g.seed[0].wrapping_add(911), 4) * 0.5 + 0.5;
    let thr = 0.62 - 0.27 * cover;
    smoothstep(thr, thr + 0.13, n).max(smoothstep(0.8, 1.0, cover) * 0.9)
}

fn pack(c: Vec3) -> u32 {
    let b = |v: f32| (v * 255.0 + 0.5) as u32;
    b(c.x) | (b(c.y) << 8) | (b(c.z) << 16) | (255 << 24)
}

pub fn render(req: &FrameRequest, terrain: &Terrain, palette: &crate::terrain::Palette, scene: &SceneTypes) -> Vec<u32> {
    let g = &req.globals;
    let insts: Vec<(GpuInst, Arc<TypeEntry>)> = req.instances.iter().filter_map(|i| scene.types.get(&i.info[0]).map(|t| (*i, t.clone()))).collect();
    let sc = Scene { g, terrain, insts };
    let (w, h) = (req.width as usize, req.height as usize);
    let mut px = vec![0u32; w * h];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
    let rows_per = h.div_ceil(threads).max(1);
    let ro = v3(g.cam_pos);
    let ground = |x: f32, z: f32, hh: f32, ny: f32| terrain.ground_color(palette, x, z, hh, ny);
    std::thread::scope(|s| {
        for (ci, chunk) in px.chunks_mut(rows_per * w).enumerate() {
            let sc = &sc;
            let ground = &ground;
            s.spawn(move || {
                for (k, out) in chunk.iter_mut().enumerate() {
                    let y = ci * rows_per + k / w;
                    let x = k % w;
                    let u = 2.0 * (x as f32 + 0.5) / w as f32 - 1.0;
                    let v = 1.0 - 2.0 * (y as f32 + 0.5) / h as f32;
                    let rd = (v3(g.cam_fwd) + v3(g.cam_right) * u + v3(g.cam_up) * v).normalize();
                    *out = pack(sc.render(ro, rd, ground));
                }
            });
        }
    });
    px
}

/// The CPU renderer behind the same channel interface as the GPU one.
pub fn spawn() -> RenderHandle {
    let (tx, rx) = crossbeam_channel::unbounded::<RenderMsg>();
    let (ftx, frx) = crossbeam_channel::unbounded::<Frame>();
    let thread = std::thread::Builder::new()
        .name("render-cpu".into())
        .spawn(move || {
            while let Ok(m) = rx.recv() {
                let mut req = match m {
                    RenderMsg::Quit => break,
                    RenderMsg::Frame(f) => f,
                };
                // Skip to the newest request.
                while let Ok(m) = rx.try_recv() {
                    match m {
                        RenderMsg::Quit => return,
                        RenderMsg::Frame(f) => {
                            let _ = ftx.send(Frame { id: req.id, width: 0, height: 0, pixels: vec![], gpu_ms: None });
                            req = f;
                        }
                    }
                }
                let t0 = std::time::Instant::now();
                let pixels = render(&req, &req.terrain, &req.look.palette, &req.scene);
                let ms = t0.elapsed().as_secs_f32() * 1000.0;
                if ftx.send(Frame { id: req.id, width: req.width, height: req.height, pixels, gpu_ms: Some(ms) }).is_err() {
                    break;
                }
            }
        })
        .expect("spawn cpu renderer");
    RenderHandle { tx, rx: frx, backend: "CPU".into(), cpu_fallback: true, thread: Some(thread) }
}
