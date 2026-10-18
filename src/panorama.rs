//! Behind the title: a world, slowly turning, seen from above its trees.
//! The same world for everyone (a showcase made from the game's own land and
//! pieces, no AI needed), so a first-time player sees it too. It loads in the
//! background; until the first frame the land of words is shown, and the
//! world fades in over it. Only on a GPU: the CPU renderer is too slow.

use crate::render::{self, Camera, Frame, FrameRequest, RenderHandle, RenderMsg};
use crate::sim::Sim;
use crate::world::WorldSnapshot;
use glam::Vec3;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Radians a second: a whole turn in about two minutes.
const TURN: f32 = 0.05;
/// Metres above a standing eye.
const LIFT: f32 = 14.0;
/// The time of day it opens at (afternoon: blue sky, long light).
const HOUR: f64 = 16.5;
/// Lines drawn (the window scales them up, softly: it is a backdrop).
const LINES: f32 = 300.0;

struct Ready {
    snap: Arc<WorldSnapshot>,
    sim: Sim,
    render: RenderHandle,
    eye: Vec3,
    yaw: f32,
    t_game: f64,
}

type Loaded = (Arc<crate::db::Db>, Arc<render::gpu::Gpu>, Arc<WorldSnapshot>);

pub struct Panorama {
    loading: Option<crossbeam_channel::Receiver<Option<Loaded>>>,
    ready: Option<Ready>,
    frame: Option<Frame>,
    waiting: bool,
    next_id: u64,
    start: Instant,
    /// When the first frame came (the world fades in from then).
    first: Option<Instant>,
}

impl Panorama {
    /// Begin making (the first time) and loading the showcase in the background.
    pub fn start() -> Panorama {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let _ = std::thread::Builder::new().name("panorama".into()).spawn(move || {
            let got = showcase().and_then(|p| crate::load_with(&p, false).ok()).and_then(|l| l.gpu.map(|g| (l.db, g, l.snap)));
            if got.is_none() {
                crate::log::info("panorama: no view (no GPU, or the showcase didn't load)");
            }
            let _ = tx.send(got);
        });
        Panorama { loading: Some(rx), ready: None, frame: None, waiting: false, next_id: 1, start: Instant::now(), first: None }
    }

    /// Take what came back and ask for the next frame, `size` pixels of window.
    pub fn tick(&mut self, size: (f32, f32)) {
        if let Some(rx) = &self.loading {
            if let Ok(got) = rx.try_recv() {
                self.loading = None;
                if let Some((db, gpu, snap)) = got {
                    let (stand, yaw) = vista(&snap.terrain);
                    // Above the treetops, for the lie of the land.
                    let eye = stand + Vec3::Y * LIFT;
                    let t_game = HOUR / 24.0 * render::sky::DAY_SECONDS;
                    let sim = Sim::new(db, snap.clone(), stand, yaw, t_game, 1);
                    self.ready = Some(Ready { snap, sim, render: render::gpu::spawn(gpu), eye, yaw, t_game });
                }
            }
        }
        let Some(r) = self.ready.as_mut() else { return };
        while let Ok(f) = r.render.rx.try_recv() {
            self.waiting = false;
            if f.width > 0 {
                self.first.get_or_insert_with(Instant::now);
                self.frame = Some(f);
            }
        }
        if self.waiting {
            return;
        }
        let k = (LINES / size.1.max(1.0)).min(1.0);
        let (w, h) = (((size.0 * k).round() as u32).max(8), ((size.1 * k).round() as u32).max(8));
        let t = self.start.elapsed().as_secs_f32();
        let cam = Camera { pos: r.eye, yaw: r.yaw + t * TURN, pitch: -0.14, fov_y: 1.1, roll: 0.0 };
        let aspect = w as f32 / h as f32;
        let drawn = r.sim.draw(cam.pos, render::VIEW_DIST);
        let culled = crate::world::cull::cull(&r.snap, &mut r.sim.cache, &cam, aspect, &drawn.insts, &Default::default());
        // The day goes on, slowly: the light shifts while you look.
        let t_game = r.t_game + t as f64 * 4.0;
        let light = render::sky::lighting(t_game, &r.snap.look.palette);
        let sp = render::SceneParams {
            terrain: &r.snap.terrain,
            palette: &r.snap.look.palette,
            camera: cam,
            width: w,
            height: h,
            pixel_aspect: 1.0,
            light,
            time: t,
            frame: self.next_id as u32,
            shadows: true,
            lights: &drawn.lights,
            weather: render::WeatherView::of(&r.sim.wx, 0.0, !r.sim.open_sky(cam.pos)),
        };
        let globals = render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
        let req = FrameRequest { id: self.next_id, width: w, height: h, globals, instances: culled.insts, grid: culled.grid, scene: r.snap.scene.clone(), terrain: r.snap.terrain.clone(), look: r.snap.look.clone() };
        self.next_id += 1;
        self.waiting = r.render.tx.send(RenderMsg::Frame(Box::new(req))).is_ok();
    }

    /// The latest view, once there is one.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// 0 → 1 over the first second and a half of the view.
    pub fn shown(&self) -> f32 {
        self.first.map_or(0.0, |t| (t.elapsed().as_secs_f32() / 1.5).min(1.0))
    }
}

impl Drop for Panorama {
    fn drop(&mut self) {
        if let Some(r) = self.ready.as_mut() {
            r.render.shutdown();
        }
    }
}

/// The world behind the title for everyone: the same land each time, made
/// from the game's own pieces (no AI needed), kept beside your worlds.
pub fn showcase() -> Option<PathBuf> {
    let path = crate::log::dir().join("showcase.pocket");
    if !path.exists() {
        let _ = std::fs::create_dir_all(crate::log::dir());
        let look = serde_json::to_string(&crate::world::Look::default()).ok()?;
        let tmp = path.with_extension("making");
        let _ = std::fs::remove_file(&tmp);
        crate::db::Db::create(&tmp, SEED, "", &look).ok()?;
        std::fs::rename(&tmp, &path).ok()?;
    }
    Some(path)
}

/// The land the showcase is made from.
pub const SEED: u32 = 7;

/// Where to stand for the showcase: a gentle hilltop above the shore that
/// looks out over lower land (and water, when there is some), and the way
/// to face first. The same for everyone (the land is the same).
pub fn vista(t: &crate::terrain::Terrain) -> (Vec3, f32) {
    let mut best = (f32::MIN, Vec3::ZERO, 0.0);
    for i in -12..=12 {
        for j in -12..=12 {
            let (x, z) = (i as f32 * 40.0, j as f32 * 40.0);
            let h = t.height(x, z);
            if h < t.sea + 6.0 || t.water(x, z) > h - 0.5 {
                continue;
            }
            // Level enough to stand on.
            let slope = (t.height(x + 3.0, z) - t.height(x - 3.0, z)).abs() + (t.height(x, z + 3.0) - t.height(x, z - 3.0)).abs();
            if slope > 2.5 {
                continue;
            }
            // How far the land falls away around, and which way it falls most.
            let (mut drop, mut low, mut low_yaw) = (0.0, f32::MAX, 0.0);
            for k in 0..16 {
                let a = k as f32 / 16.0 * std::f32::consts::TAU;
                let (dx, dz) = (a.sin(), -a.cos());
                let far = t.height(x + dx * 120.0, z + dz * 120.0).max(t.water(x + dx * 120.0, z + dz * 120.0));
                drop += (h - far).max(0.0);
                if far < low {
                    low = far;
                    low_yaw = a;
                }
            }
            let score = drop / 16.0 - (h - t.sea - 25.0).max(0.0) * 0.5 - (x * x + z * z).sqrt() * 0.004;
            if score > best.0 {
                best = (score, Vec3::new(x, h, z), low_yaw);
            }
        }
    }
    let p = best.1;
    (Vec3::new(p.x, p.y + 1.65 + 1.0, p.z), best.2)
}
