//! Rendering: GPU compute raymarcher (wgpu) with a CPU fallback. The renderer
//! knows nothing about terminals; it fills an RGBA pixel buffer of any size.

pub mod cpu;
pub mod gpu;
pub mod shader;
pub mod sky;

use crate::terrain::{MAX_BIOMES, Palette, Terrain, rgbv};
use bytemuck::{Pod, Zeroable};
use glam::{Quat, Vec3};
use std::sync::Arc;

pub const VIEW_DIST: f32 = 170.0;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Globals {
    pub cam_pos: [f32; 4],
    pub cam_fwd: [f32; 4],
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    pub sun_dir: [f32; 4],
    pub sun_col: [f32; 4],
    pub sky_zen: [f32; 4],
    pub sky_hor: [f32; 4],
    pub water: [f32; 4],
    pub rock: [f32; 4],
    pub sand: [f32; 4],
    pub snow: [f32; 4],
    pub toff0: [f32; 4],
    pub toff1: [f32; 4],
    pub dims: [u32; 4],
    pub seed: [u32; 4],
    pub grid0: [f32; 4],
    pub grid1: [u32; 4],
    pub probe: [u32; 4],
    pub hmap: [f32; 4],
    pub biomes: [[f32; 4]; 18],
    /// Point lights: position, intensity.
    pub lights: [[f32; 4]; MAX_LIGHTS],
    /// Point lights: colour, reach in metres.
    pub light_cols: [[f32; 4]; MAX_LIGHTS],
    /// Hollows in use, roads in use, -, -.
    pub extra: [u32; 4],
    /// Ground taken away, two per hollow: (centre x, z, cos, sin), (half x, z, floor, round).
    pub hollows: [[f32; 4]; MAX_HOLLOWS * 2],
    /// Roads, two per stretch: (a.x, a.z, b.x, b.z), (half width, r, g, b).
    pub roads: [[f32; 4]; MAX_ROADS * 2],
}

pub const MAX_LIGHTS: usize = 8;
pub const MAX_HOLLOWS: usize = 8;
pub const MAX_ROADS: usize = 16;

/// A light-emitting thing (lantern, fire) near the camera.
#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    pub pos: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    pub reach: f32,
}

pub const FLAG_GRID: u32 = 1;
pub const FLAG_SHADOWS: u32 = 2;

/// One instance as the shader sees it (192 bytes).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, Default)]
pub struct GpuInst {
    pub pos_scale: [f32; 4],
    pub rot: [f32; 4],
    /// Tilt after the yaw: the x, y, z of a unit quaternion whose w is taken
    /// to be ≥ 0 (all zero: upright). Doors swing on it, tools swing with it,
    /// a log lies on its side.
    pub tilt: [f32; 4],
    /// Static parameters k.seed, k.scale, k.a … k.f.
    pub k0: [f32; 4],
    pub k1: [f32; 4],
    /// Live state k.s0 … k.s7 (behaviour code, poses).
    pub s0: [f32; 4],
    pub s1: [f32; 4],
    /// Generic look: charred 0..1, wet 0..1, glow 0..1, highlight 0..1
    /// (below 0: corrupted, darkened with a dull red glow).
    pub fx: [f32; 4],
    pub info: [u32; 4],
    /// Cuts taken out of the shape, in its local frame: centre xyz and size
    /// w (w > 0 a sphere of that radius, w < 0 a cube of half-size -w, 0 none).
    pub cuts: [[f32; 4]; MAX_CUTS],
}

pub const MAX_CUTS: usize = 4;

/// The distance to one cut (local units); twin of `cut_sdf` in scene.wgsl.
pub fn cut_sdf(c: &[f32; 4], p: [f32; 3]) -> f32 {
    let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
    if c[3] > 0.0 {
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - c[3]
    } else {
        let h = -c[3];
        let q = [d[0].abs() - h, d[1].abs() - h, d[2].abs() - h];
        let o = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2) + q[2].max(0.0).powi(2)).sqrt();
        o + q[0].max(q[1]).max(q[2]).min(0.0)
    }
}

/// A shape's local distance with its cuts taken out.
pub fn apply_cuts(d: f32, cuts: &[[f32; 4]; MAX_CUTS], p: [f32; 3]) -> f32 {
    let mut d = d;
    for c in cuts {
        if c[3] != 0.0 {
            d = d.max(-cut_sdf(c, p));
        }
    }
    d
}

pub const FX_CHAR: usize = 0;
pub const FX_WET: usize = 1;
pub const FX_GLOW: usize = 2;
pub const FX_HIGHLIGHT: usize = 3;

impl GpuInst {
    pub fn k(&self) -> [f32; 16] {
        let mut k = [0.0; 16];
        k[0..4].copy_from_slice(&self.k0);
        k[4..8].copy_from_slice(&self.k1);
        k[8..12].copy_from_slice(&self.s0);
        k[12..16].copy_from_slice(&self.s1);
        k
    }
    /// The shape's distance at world point `p` (CPU), cuts included.
    pub fn sdf(&self, ct: &crate::lang::CompiledType, p: Vec3) -> f32 {
        let lp = self.to_local(p);
        apply_cuts(ct.sdf(lp, &self.k()), &self.cuts, lp) * self.pos_scale[3]
    }
    pub fn set_state(&mut self, s: &[f32; 8]) {
        self.s0.copy_from_slice(&s[0..4]);
        self.s1.copy_from_slice(&s[4..8]);
    }
    pub fn pos(&self) -> Vec3 {
        Vec3::new(self.pos_scale[0], self.pos_scale[1], self.pos_scale[2])
    }
    pub fn radius(&self) -> f32 {
        self.rot[2]
    }
    /// Bounding sphere centre (the type's sphere is offset vertically from its origin).
    pub fn center(&self) -> Vec3 {
        let off = Vec3::Y * (f32::from_bits(self.info[1]) * self.pos_scale[3]);
        match self.tilt_q() {
            Some(q) => self.pos() + self.yaw_out(q * off),
            None => self.pos() + off,
        }
    }
    /// The tilt as a rotation, if there is one.
    pub fn tilt_q(&self) -> Option<Quat> {
        let v = Vec3::new(self.tilt[0], self.tilt[1], self.tilt[2]);
        if v == Vec3::ZERO {
            return None;
        }
        let w = (1.0 - v.length_squared()).max(0.0).sqrt();
        Some(Quat::from_xyzw(v.x, v.y, v.z, w))
    }
    pub fn set_tilt(&mut self, q: Quat) {
        let q = q.normalize();
        let q = if q.w < 0.0 { -q } else { q };
        self.tilt = if q.xyz().length_squared() < 1e-10 { [0.0; 4] } else { [q.x, q.y, q.z, 0.0] };
    }
    /// The yaw turn of a local offset (no tilt, no scale).
    fn yaw_out(&self, l: Vec3) -> Vec3 {
        let (c, s) = (self.rot[0], self.rot[1]);
        Vec3::new(c * l.x + s * l.z, l.y, -s * l.x + c * l.z)
    }
    /// World → local transform; twin of `to_local` in scene.wgsl.
    pub fn to_local(&self, p: Vec3) -> [f32; 3] {
        let d = p - self.pos();
        let (c, s) = (self.rot[0], self.rot[1]);
        let sc = self.pos_scale[3];
        let mut l = Vec3::new(c * d.x - s * d.z, d.y, s * d.x + c * d.z);
        if let Some(q) = self.tilt_q() {
            l = q.inverse() * l;
        }
        (l / sc).to_array()
    }
    /// Local → world; inverse of `to_local`.
    pub fn from_local(&self, l: Vec3) -> Vec3 {
        let mut l = l * self.pos_scale[3];
        if let Some(q) = self.tilt_q() {
            l = q * l;
        }
        self.pos() + self.yaw_out(l)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Vertical field of view, radians.
    pub fov_y: f32,
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), self.yaw.cos() * self.pitch.cos())
    }
    /// Unit right/up vectors for a view.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let f = self.forward();
        let r = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        let u = r.cross(f).normalize() * -1.0;
        (f, r, u)
    }
}

/// Coarse XZ grid of instance indices (used when many instances are visible).
#[derive(Clone, Debug, Default)]
pub struct Grid {
    pub origin: [f32; 2],
    pub cell: f32,
    pub w: u32,
    pub h: u32,
    pub cells: Vec<[u32; 2]>,
    pub items: Vec<u32>,
}

pub const GRID_CELL: f32 = 8.0;
pub const GRID_THRESHOLD: usize = 64;

impl Grid {
    pub fn build(insts: &[GpuInst], center: Vec3, half: f32) -> Grid {
        let cell = GRID_CELL;
        let n = ((half * 2.0) / cell).ceil() as u32;
        let origin = [(center.x - half).floor(), (center.z - half).floor()];
        let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); (n * n) as usize];
        for (i, inst) in insts.iter().enumerate() {
            let p = inst.center();
            let r = inst.radius();
            let x0 = (((p.x - r) - origin[0]) / cell).floor().max(0.0) as i64;
            let x1 = (((p.x + r) - origin[0]) / cell).floor().min(n as f32 - 1.0) as i64;
            let z0 = (((p.z - r) - origin[1]) / cell).floor().max(0.0) as i64;
            let z1 = (((p.z + r) - origin[1]) / cell).floor().min(n as f32 - 1.0) as i64;
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if x >= 0 && z >= 0 && x < n as i64 && z < n as i64 {
                        buckets[(z as u32 * n + x as u32) as usize].push(i as u32);
                    }
                }
            }
        }
        let mut cells = Vec::with_capacity(buckets.len());
        let mut items = Vec::new();
        for b in buckets {
            cells.push([items.len() as u32, b.len() as u32]);
            items.extend(b);
        }
        Grid { origin, cell, w: n, h: n, cells, items }
    }
}

/// Lighting state for a moment in the day/night cycle.
#[derive(Clone, Copy, Debug)]
pub struct Lighting {
    pub sun_dir: Vec3,
    pub daylight: f32,
    pub night: f32,
    pub sun_col: Vec3,
    pub zenith: Vec3,
    pub horizon: Vec3,
    pub ambient: f32,
}

pub struct SceneParams<'a> {
    pub terrain: &'a Terrain,
    pub palette: &'a Palette,
    pub camera: Camera,
    pub width: u32,
    pub height: u32,
    /// Width / height of one pixel as displayed (0.5 for quadrant cells and ASCII, 1.0 for square pixels).
    pub pixel_aspect: f32,
    pub light: Lighting,
    pub time: f32,
    pub frame: u32,
    pub shadows: bool,
    /// Nearest first; at most MAX_LIGHTS are used.
    pub lights: &'a [PointLight],
}

/// Identifies the terrain function (the renderer rebuilds its heightmap when it changes).
pub fn terrain_epoch(t: &Terrain) -> u32 {
    let mut h = crate::noise::pcg(t.seed);
    for b in &t.biomes {
        for v in [b.base, b.amp, b.rough] {
            h = crate::noise::pcg(h ^ v.to_bits());
        }
    }
    h.max(1)
}

pub fn build_globals(sp: &SceneParams, n_inst: usize, grid: Option<&Grid>) -> Globals {
    let (f, r, u) = sp.camera.basis();
    let tan = (sp.camera.fov_y * 0.5).tan();
    let aspect = sp.width as f32 / sp.height.max(1) as f32 * sp.pixel_aspect;
    let t = sp.terrain;
    let pal = sp.palette;
    let l = &sp.light;
    let mut biomes = [[0.0f32; 4]; 18];
    for (i, b) in t.biomes.iter().enumerate().take(MAX_BIOMES) {
        let c = Terrain::center(i);
        let g1 = rgbv(b.ground);
        let g2 = rgbv(b.ground2);
        biomes[i * 3] = [c[0], c[1], b.base, b.amp];
        biomes[i * 3 + 1] = [b.rough, g1.x, g1.y, g1.z];
        biomes[i * 3 + 2] = [g2.x, g2.y, g2.z, 0.0];
    }
    let v4 = |v: Vec3, w: f32| [v.x, v.y, v.z, w];
    let mut flags = 0;
    if grid.is_some() {
        flags |= FLAG_GRID;
    }
    if sp.shadows {
        flags |= FLAG_SHADOWS;
    }
    if let Ok(v) = std::env::var("POCKET_DEBUG_FLAGS") {
        flags |= v.parse::<u32>().unwrap_or(0);
    }
    let g = grid.cloned().unwrap_or_default();
    let cp = sp.camera.pos;
    let (hollows, carve_version) = t.hollows_near(cp.x, cp.z, MAX_HOLLOWS);
    let mut hollow_buf = [[0.0f32; 4]; MAX_HOLLOWS * 2];
    for (i, h) in hollows.iter().enumerate() {
        hollow_buf[i * 2] = [h.c[0], h.c[1], h.rot[0], h.rot[1]];
        hollow_buf[i * 2 + 1] = [h.half[0], h.half[1], h.floor, if h.round { 1.0 } else { 0.0 }];
    }
    let roads = t.roads_near(cp.x, cp.z, VIEW_DIST * 0.6, MAX_ROADS);
    let mut road_buf = [[0.0f32; 4]; MAX_ROADS * 2];
    for (i, r) in roads.iter().enumerate() {
        road_buf[i * 2] = [r.a[0], r.a[1], r.b[0], r.b[1]];
        road_buf[i * 2 + 1] = [r.half, r.color[0], r.color[1], r.color[2]];
    }
    let mut lights = [[0.0f32; 4]; MAX_LIGHTS];
    let mut light_cols = [[0.0f32; 4]; MAX_LIGHTS];
    let nl = sp.lights.len().min(MAX_LIGHTS);
    for (i, l) in sp.lights.iter().take(MAX_LIGHTS).enumerate() {
        lights[i] = [l.pos.x, l.pos.y, l.pos.z, l.intensity];
        light_cols[i] = [l.color.x, l.color.y, l.color.z, l.reach.max(0.5)];
    }
    Globals {
        cam_pos: v4(sp.camera.pos, sp.time),
        cam_fwd: v4(f, VIEW_DIST),
        cam_right: v4(r * tan * aspect, 0.0),
        cam_up: v4(u * tan, 0.0),
        sun_dir: v4(l.sun_dir, l.daylight),
        sun_col: v4(l.sun_col, l.night),
        sky_zen: v4(l.zenith, pal.fog),
        sky_hor: v4(l.horizon, l.ambient),
        water: v4(rgbv(pal.water), crate::terrain::WATER_LEVEL),
        rock: v4(rgbv(pal.rock), 0.0),
        sand: v4(rgbv(pal.sand), 0.0),
        snow: v4(rgbv(pal.snow), 0.0),
        toff0: [t.off[0], t.off[1], t.off[2], t.off[3]],
        toff1: [t.off[4], t.off[5], 0.0, 0.0],
        dims: [sp.width, sp.height, n_inst as u32, flags],
        seed: [t.seed, t.biomes.len().min(MAX_BIOMES) as u32, sp.frame, terrain_epoch(t) ^ carve_version.wrapping_mul(0x9E37_79B9)],
        grid0: [g.origin[0], g.origin[1], g.cell.max(1.0), 0.0],
        grid1: [g.w, g.h, nl as u32, 0],
        probe: [0; 4],
        hmap: [0.0; 4],
        biomes,
        lights,
        light_cols,
        extra: [hollows.len() as u32, roads.len() as u32, 0, 0],
        hollows: hollow_buf,
        roads: road_buf,
    }
}

/// Everything one frame needs. The pipeline and the instances come from the
/// same world snapshot, which is what makes version flips atomic.
pub struct FrameRequest {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub globals: Globals,
    pub instances: Vec<GpuInst>,
    pub grid: Option<Grid>,
    pub scene: Arc<crate::world::SceneTypes>,
    /// Used by the CPU renderer (the GPU gets everything via `globals`).
    pub terrain: Arc<Terrain>,
    pub look: Arc<crate::world::Look>,
}

pub struct Frame {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    /// RGBA8 packed little-endian (r in the low byte).
    pub pixels: Vec<u32>,
    pub gpu_ms: Option<f32>,
}

pub enum RenderMsg {
    Frame(Box<FrameRequest>),
    Quit,
}

/// The main thread's view of whichever renderer is running.
pub struct RenderHandle {
    pub tx: crossbeam_channel::Sender<RenderMsg>,
    pub rx: crossbeam_channel::Receiver<Frame>,
    pub backend: String,
    pub cpu_fallback: bool,
    pub thread: Option<std::thread::JoinHandle<()>>,
}

impl RenderHandle {
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(RenderMsg::Quit);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub fn unpack(px: u32) -> [u8; 3] {
    [(px & 0xff) as u8, ((px >> 8) & 0xff) as u8, ((px >> 16) & 0xff) as u8]
}

#[cfg(test)]
mod tests;
