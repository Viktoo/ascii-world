//! Pocket Universe: an infinite, colour 3D world in your terminal.

mod achievements;
mod audio;
mod app;
mod brain;
mod db;
mod decider;
#[cfg(test)]
mod e2e_tests;
mod lang;
mod llm;
mod log;
mod model;
mod noise;
mod pace;
mod picker;
mod png;
mod prompts;
mod random_world;
mod render;
mod settings;
mod sim;
mod term;
mod terrain;
mod world;

use anyhow::{Context, Result, bail};
use glam::Vec3;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

const USAGE: &str = "Pocket Universe — an infinite 3D world in your terminal

  pocket new [\"<prompt>\"] [--out FILE] [--difficulty peaceful|easy|normal(default)|hard]
                                         create a universe: check or edit the prompt
                                         (a random one if none given), then step in
  pocket prompts [--sample N]            print N random prompts (default 20)
  pocket [FILE]                          reopen FILE, or your last universe
  pocket list                            pick one of your worlds (or start a new one)
  pocket snapshot FILE --at x,z,yawDeg [--size 120x40] [--ascii|--blocks] [--mono] [--time HOUR] [--weather NAME]
  pocket describe FILE --at x,z,yawDeg [--size 120x40]   JSON of what is visible
  pocket bench FILE [--distance 2000] [--size 120x40]
  pocket sim FILE [--hours H] [--seed S] [--events out.jsonl] [--save] [--no-llm] [--verify]
                                         run the world headless, fast, and report what emerged
  pocket sim --replay out.jsonl [--from T] [--kinds ignited,caught]
  pocket act FILE --as player|ID|NAME '{\"do\": \"hold\", \"target\": {\"name\": \"stick\"}}' [--run SECS] [--dry]
  pocket act FILE --stdin                one JSON command per line (actions, inspect, look, run)
  pocket inspect FILE player|npc:ID|thing:ID|instance:ID|cell:X,Z|NAME   (or --look [--as WHO])
  pocket listen --play | --demo | FILE [--species NAME | --at x,z,yawDeg --walk --calls]   hear it (WAVs)
  pocket check TYPE.js                   validate an object type module (steps 1-4)
  pocket selftest                        GPU/CPU parity and validator checks

Environment: ANTHROPIC_API_KEY, or POCKET_LLM_BASE_URL (+ POCKET_LLM_API_KEY) for any
OpenAI-compatible endpoint; POCKET_MODEL_{BUILDER,CHARACTER,DECIDER,SUMMARIZER};
POCKET_BUDGET_USD; POCKET_REGION_RADIUS (default 2); POCKET_DECIDER_URL; POCKET_FPS;
POCKET_NO_GPU=1; POCKET_HOME (default ~/.pocket); POCKET_SIM_NEAR, POCKET_SIM_MEDIUM,
POCKET_SIM_FAR (frozen | catchup:HOURS), POCKET_LLM_PER_MIN (how alive the world is),
POCKET_NO_SOUND=1.";

fn main() {
    log::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = run(args);
    term::restore();
    if let Err(e) = r {
        eprintln!("pocket: {e:#}");
        std::process::exit(1);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn run(args: Vec<String>) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("-h") | Some("--help") | Some("help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some("new") => {
            // Every new world begins on the new-world screen: the prompt given,
            // to check, or a random one to keep or change.
            let Some(prompt) = picker::compose(args.get(1).filter(|a| !a.starts_with("--")).map(String::as_str))? else { return Ok(()) };
            let path = match flag(&args, "--out") {
                Some(p) => PathBuf::from(p),
                None => default_path(&prompt),
            };
            let difficulty = flag(&args, "--difficulty").unwrap_or_else(|| NEW_DIFFICULTY.into());
            let n = sim::night::LEVELS.iter().position(|l| l.name == difficulty.trim().to_lowercase()).with_context(|| format!("difficulty is one of peaceful, easy, normal, hard (not {difficulty})"))?;
            create_universe(&path, &prompt)?;
            db::Db::open(&path)?.kv_set("sim.difficulty", &n.to_string())?;
            play(&path, true)
        }
        Some("list") | Some("ls") => match picker::pick()? {
            picker::Choice::Open(p) => play(&p, false),
            picker::Choice::New(prompt) => {
                let path = default_path(&prompt);
                create_universe(&path, &prompt)?;
                play(&path, true)
            }
            picker::Choice::Quit => Ok(()),
        },
        Some("prompts") => {
            let n = flag(&args, "--sample").and_then(|n| n.parse().ok()).unwrap_or(20);
            for p in random_world::sample(n) {
                println!("{p}\n");
            }
            Ok(())
        }
        Some("open") => play(&PathBuf::from(args.get(1).context("usage: pocket open FILE")?), false),
        Some("snapshot") => snapshot(&args[1..]),
        Some("describe") => describe(&args[1..]),
        Some("bench") => bench(&args[1..]),
        Some("selftest") => selftest(),
        Some("gpubench") => gpubench(&args[1..]),
        Some("check") => check_file(&args[1..]),
        Some("sim") => sim::headless::run_sim_cli(&args[1..]),
        Some("act") => sim::headless::act_cli(&args[1..]),
        Some("inspect") => sim::headless::inspect_cli(&args[1..]),
        Some("listen") => audio::listen::cli(&args[1..]),
        Some(p) if !p.starts_with('-') => play(Path::new(p), false),
        Some(other) => bail!("unknown option {other}\n\n{USAGE}"),
        None => {
            let last = std::fs::read_to_string(log::dir().join("last")).ok().map(|s| PathBuf::from(s.trim()));
            match last {
                Some(p) if p.exists() => play(&p, false),
                // No universe yet: start one.
                _ => match picker::compose(None)? {
                    Some(prompt) => {
                        let path = default_path(&prompt);
                        create_universe(&path, &prompt)?;
                        play(&path, true)
                    }
                    None => Ok(()),
                },
            }
        }
    }
}

fn default_path(prompt: &str) -> PathBuf {
    let slug: String = prompt.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let slug: String = slug.split('-').filter(|s| !s.is_empty()).take(5).collect::<Vec<_>>().join("-");
    let dir = log::dir().join("universes");
    let mut p = dir.join(format!("{}.pocket", if slug.is_empty() { "universe" } else { &slug }));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{slug}-{n}.pocket"));
        n += 1;
    }
    p
}

/// New worlds start here (older worlds, made before difficulty, stay peaceful).
const NEW_DIFFICULTY: &str = "normal";

fn create_universe(path: &Path, prompt: &str) -> Result<()> {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let seed = noise::pcg((t as u32) ^ ((t >> 32) as u32) ^ std::process::id());
    let look = serde_json::to_string(&world::Look::default())?;
    let db = db::Db::create(path, seed, prompt.trim(), &look)?;
    let n = sim::night::LEVELS.iter().position(|l| l.name == NEW_DIFFICULTY).unwrap_or(0);
    db.kv_set("sim.difficulty", &n.to_string())?;
    Ok(())
}

struct Loaded {
    db: Arc<db::Db>,
    gpu: Option<Arc<render::gpu::Gpu>>,
    model: model::WorldModel,
    snap: Arc<world::WorldSnapshot>,
    live: model::LiveRef,
    /// `snap` is the quick base snapshot; the full world flips in later.
    partial: bool,
}

/// Above this many types, start from a base snapshot so the player can walk at once.
const QUICK_START_TYPES: usize = 16;

fn load(path: &Path) -> Result<Loaded> {
    load_with(path, false)
}

fn load_with(path: &Path, quick: bool) -> Result<Loaded> {
    let db = db::Db::open(path)?;
    let gpu = match render::gpu::Gpu::new() {
        Ok(g) => Some(g),
        Err(e) => {
            log::error(format!("GPU unavailable: {e:#}"));
            None
        }
    };
    let live = Arc::new(Mutex::new(model::Live::default()));
    let mut model = model::WorldModel::load(db.clone(), gpu.clone(), live.clone())?;
    let t0 = Instant::now();
    let partial = quick && model.active_type_count() > QUICK_START_TYPES;
    let snap = if partial { model.snapshot_base() } else { model.snapshot() };
    let snap = snap.map_err(|d| anyhow::anyhow!("building the world shader failed: {}", lang::format_diags(&d)))?;
    log::info(format!("loaded {} (v{}, {} types, {} instances) in {:.0} ms", path.display(), snap.version, snap.scene.types.len(), snap.instances.len(), t0.elapsed().as_secs_f64() * 1000.0));
    Ok(Loaded { db, gpu, model, snap, live, partial })
}

fn play(path: &Path, fresh: bool) -> Result<()> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let _ = std::fs::write(log::dir().join("last"), path.display().to_string());
    let l = load_with(&path, true)?;
    let llm = llm::Llm::from_env(l.db.clone());
    let genesis = fresh || l.db.kv_get("genesis").is_none();
    let (etx, erx) = crossbeam_channel::unbounded();
    let brain = brain::Brain::start(l.model, llm.clone(), etx, l.partial);
    let render = match &l.gpu {
        Some(g) => render::gpu::spawn(g.clone()),
        None => render::cpu::spawn(),
    };
    let stop = term::signal_flag();
    let ti = term::enter()?;
    let size = crossterm::terminal::size().unwrap_or((120, 40));
    let mut app = app::App::new(app::Setup {
        db: l.db.clone(),
        brain,
        events: erx,
        render,
        snap: l.snap,
        live: l.live,
        enhanced: ti.enhanced,
        truecolor: ti.truecolor,
        size,
        llm,
        genesis,
    });
    app.start_loading();
    let mut out = std::io::stdout();
    let r = app.run(true, &mut out, &stop, None);
    app.shutdown();
    term::restore();
    r?;
    println!("Saved {}", path.display());
    Ok(())
}

fn parse_at(args: &[String]) -> Result<(f32, f32, f32)> {
    let at = flag(args, "--at").context("--at x,z,yawDeg is required")?;
    let v: Vec<f32> = at.split(',').map(|s| s.trim().parse::<f32>()).collect::<Result<_, _>>().context("--at expects x,z,yawDeg")?;
    if v.len() != 3 {
        bail!("--at expects x,z,yawDeg");
    }
    Ok((v[0], v[1], v[2].to_radians()))
}

fn parse_size(args: &[String]) -> Result<(u16, u16)> {
    let s = flag(args, "--size").unwrap_or_else(|| "120x40".into());
    let (w, h) = s.split_once('x').context("--size expects COLSxROWS")?;
    Ok((w.parse::<u16>()?.clamp(8, 1000), h.parse::<u16>()?.clamp(4, 500)))
}

/// One frame, rendered synchronously (shared by snapshot and tests).
fn render_once(l: &Loaded, cam: render::Camera, w: u32, h: u32, pixel_aspect: f32, t_game: f64, weather: Option<&str>) -> Result<render::Frame> {
    let snap = &l.snap;
    let mut live = sim::Sim::new(l.db.clone(), snap.clone(), cam.pos, cam.yaw, t_game, 1);
    if let Some(name) = weather {
        let kind = snap.look.climate.kinds.iter().find(|k| k.name == name).cloned().or_else(|| world::weather::sample(name)).with_context(|| format!("no weather called {name} (the world's own, or clear, fair, cloudy, overcast, fog, rain, storm, snow, ash, motes)"))?;
        live.weather_st.spell = Some(world::weather::Spell { kind, from: t_game - 3.0 * world::weather::HOUR, until: t_game + 3.0 * world::weather::HOUR, by: String::new() });
        live.step_weather();
    }
    let aspect = w as f32 / h as f32 * pixel_aspect;
    let drawn = live.draw(cam.pos, render::VIEW_DIST);
    let culled = world::cull::cull(snap, &mut live.cache, &cam, aspect, &drawn.insts, &Default::default());
    let light = render::sky::lighting(t_game, &snap.look.palette);
    let sp = render::SceneParams { terrain: &snap.terrain, palette: &snap.look.palette, camera: cam, width: w, height: h, pixel_aspect, light, time: 0.0, frame: 0, shadows: l.gpu.is_some() && std::env::var("POCKET_NO_SHADOWS").is_err(), lights: &drawn.lights, weather: render::WeatherView::of(&live.wx, 0.0, !live.open_sky(cam.pos)) };
    let globals = render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
    let req = render::FrameRequest { id: 1, width: w, height: h, globals, instances: culled.insts, grid: culled.grid, scene: snap.scene.clone(), terrain: snap.terrain.clone(), look: snap.look.clone() };
    let mut handle = match &l.gpu {
        Some(g) => render::gpu::spawn(g.clone()),
        None => render::cpu::spawn(),
    };
    handle.tx.send(render::RenderMsg::Frame(Box::new(req)))?;
    let f = handle.rx.recv_timeout(Duration::from_secs(60)).context("render timed out")?;
    handle.shutdown();
    if f.width == 0 {
        bail!("render failed");
    }
    Ok(f)
}

fn camera_at(snap: &world::WorldSnapshot, x: f32, z: f32, yaw: f32) -> render::Camera {
    let y = snap.terrain.height(x, z).max(terrain::WATER_LEVEL - 0.4) + 1.65;
    render::Camera { pos: Vec3::new(x, y, z), yaw, pitch: -0.06, fov_y: 1.05 }
}

fn snapshot(args: &[String]) -> Result<()> {
    let path = PathBuf::from(args.first().context("usage: pocket snapshot FILE --at x,z,yawDeg")?);
    let (x, z, yaw) = parse_at(args)?;
    let (w, h) = parse_size(args)?;
    let ascii = has(args, "--ascii");
    let mono = has(args, "--mono");
    let l = load(&path)?;
    let mut t_game = l.db.player().map(|p| p.t_game).unwrap_or(0.33 * render::sky::DAY_SECONDS);
    if let Some(h) = flag(args, "--time").and_then(|h| h.parse::<f64>().ok()) {
        t_game = (h / 24.0).rem_euclid(1.0) * render::sky::DAY_SECONDS;
    }
    let cam = camera_at(&l.snap, x, z, yaw);
    let png_out = flag(args, "--png");
    // Quadrant cells: 2×2 pixels each. A PNG gets square pixels at the same detail.
    let (pw, ph, pa) = match (ascii, png_out.is_some()) {
        (true, _) => (w as u32, h as u32, 0.5),
        (false, true) => (w as u32 * 2, h as u32 * 4, 1.0),
        (false, false) => (w as u32 * 2, h as u32 * 2, 0.5),
    };
    let f = render_once(&l, cam, pw, ph, pa, t_game, flag(args, "--weather").as_deref())?;
    if let Some(png) = png_out {
        let s = if ascii { 4u32 } else { 2 };
        let mut rgb = Vec::with_capacity((f.width * s * f.height * s * 3) as usize);
        for y in 0..f.height * s {
            for x in 0..f.width * s {
                rgb.extend_from_slice(&render::unpack(f.pixels[((y / s) * f.width + x / s) as usize]));
            }
        }
        std::fs::write(&png, png::encode(f.width * s, f.height * s, &rgb))?;
        return Ok(());
    }
    print!("{}", app::frame_to_text(&f, ascii, mono, term::truecolor() || std::env::var("COLORTERM").is_err()));
    Ok(())
}

fn describe(args: &[String]) -> Result<()> {
    let path = PathBuf::from(args.first().context("usage: pocket describe FILE --at x,z,yawDeg")?);
    let (x, z, yaw) = parse_at(args)?;
    let l = load(&path)?;
    let t_game = l.db.player().map(|p| p.t_game).unwrap_or(0.33 * render::sky::DAY_SECONDS);
    let cam = camera_at(&l.snap, x, z, yaw);
    let mut live = sim::Sim::new(l.db.clone(), l.snap.clone(), cam.pos, cam.yaw, t_game, 1);
    let npcs = live.npc_views();
    let (w, h) = parse_size(args)?;
    let aspect = w as f32 / (h as f32 * 2.0);
    let v = world::describe::describe(&l.snap, &mut live.cache, &npcs, &cam, aspect, t_game);
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn bench(args: &[String]) -> Result<()> {
    let path = PathBuf::from(args.first().context("usage: pocket bench FILE")?);
    let distance: f32 = flag(args, "--distance").and_then(|d| d.parse().ok()).unwrap_or(2000.0);
    let (w, h) = parse_size(args)?;
    let l = load(&path)?;
    let llm = if has(args, "--with-llm") { llm::Llm::from_env(l.db.clone()) } else { None };
    let (etx, erx) = crossbeam_channel::unbounded();
    let brain = brain::Brain::start(l.model, llm.clone(), etx, l.partial);
    let render = match &l.gpu {
        Some(g) => render::gpu::spawn(g.clone()),
        None => render::cpu::spawn(),
    };
    let backend = render.backend.clone();
    let mut app = app::App::new(app::Setup { db: l.db.clone(), brain, events: erx, render, snap: l.snap, live: l.live, enhanced: true, truecolor: true, size: (w, h), llm, genesis: false });
    app.noclip = true;
    let start = app.pos();
    let stop = AtomicBool::new(false);
    let t0 = Instant::now();
    let mut warm = 0.0f32;
    let mut walked = 0.0f32;
    let mut script = |a: &mut app::App, dt: f32| -> bool {
        warm += dt;
        if warm < 1.0 {
            return true; // let the first frames settle
        }
        let f = Vec3::new(a.yaw().sin(), 0.0, a.yaw().cos());
        let p = a.pos() + f * app::WALK_SPEED * 2.0 * dt;
        a.sim.player.pos = Vec3::new(p.x, a.snap.terrain.height(p.x, p.z), p.z);
        walked = Vec3::new(a.pos().x - start.x, 0.0, a.pos().z - start.z).length();
        if warm > 1.0 && warm < 1.05 {
            a.stats.max_loop_ms = 0.0;
            a.stats.max_gap_ms = 0.0;
        }
        walked < distance && t0.elapsed() < Duration::from_secs(600)
    };
    let mut sink = CountingSink::default();
    app.run(false, &mut sink, &stop, Some(&mut script))?;
    let secs = t0.elapsed().as_secs_f32() - 1.0;
    let frames = app.stats.shown;
    println!("backend        {backend}");
    println!("viewport       {w}x{h} cells");
    println!("walked         {walked:.0} m in {secs:.1} s");
    println!("frames shown   {} ({:.1} fps)", frames, frames as f32 / secs.max(0.01));
    println!("worst loop     {:.1} ms", app.stats.max_loop_ms);
    println!("worst gap      {:.1} ms between displayed frames", app.stats.max_gap_ms);
    println!("gpu (avg)      {:.2} ms", app.stats.gpu_ms);
    println!("terminal bytes {:.1} KB/frame", sink.0 as f32 / frames.max(1) as f32 / 1024.0);
    app.shutdown();
    Ok(())
}

#[derive(Default)]
struct CountingSink(usize);
impl std::io::Write for CountingSink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0 += b.len();
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Render one view repeatedly at full GPU load; report GPU time per frame.
fn gpubench(args: &[String]) -> Result<()> {
    let path = PathBuf::from(args.first().context("usage: pocket gpubench FILE --at x,z,yawDeg")?);
    let (x, z, yaw) = parse_at(args)?;
    let (w, h) = parse_size(args)?;
    let l = load(&path)?;
    let gpu = l.gpu.clone().context("no GPU")?;
    let snap = &l.snap;
    let cam = camera_at(snap, x, z, yaw);
    let (pw, ph) = (w as u32, h as u32 * 2);
    let mut cache = world::scatter::ScatterCache::default();
    let culled = world::cull::cull(snap, &mut cache, &cam, pw as f32 / ph as f32, &[], &Default::default());
    let light = render::sky::lighting(400.0, &snap.look.palette);
    let sp = render::SceneParams { terrain: &snap.terrain, palette: &snap.look.palette, camera: cam, width: pw, height: ph, pixel_aspect: 1.0, light, time: 0.0, frame: 0, shadows: std::env::var("POCKET_NO_SHADOWS").is_err(), lights: &[], weather: Default::default() };
    let globals = render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
    let mut handle = render::gpu::spawn(gpu);
    let mut times = Vec::new();
    let mut t_start = Instant::now();
    let mut best = f64::MAX;
    for i in 0..900u64 {
        if i % 200 == 100 {
            if i > 100 {
                best = best.min(t_start.elapsed().as_secs_f64() * 1000.0 / 200.0);
            }
            t_start = Instant::now();
        }
        let req = render::FrameRequest { id: i, width: pw, height: ph, globals, instances: culled.insts.clone(), grid: culled.grid.clone(), scene: snap.scene.clone(), terrain: snap.terrain.clone(), look: snap.look.clone() };
        handle.tx.send(render::RenderMsg::Frame(Box::new(req)))?;
        // Strictly one frame in flight, so pass timestamps never overlap.
        let f = handle.rx.recv_timeout(Duration::from_secs(10))?;
        if i > 50 {
            times.push(f.gpu_ms.unwrap_or(0.0));
        }
    }
    let wall = best;
    handle.shutdown();
    times.sort_by(|a, b| a.total_cmp(b));
    println!(
        "{}x{} px, {} instances: throughput {:.2} ms/frame; pass time min {:.2}, median {:.2}, p90 {:.2} ms",
        pw, ph, culled.insts.len(), wall, times[0], times[times.len() / 2], times[times.len() * 9 / 10]
    );
    Ok(())
}

fn check_file(args: &[String]) -> Result<()> {
    let p = args.first().context("usage: pocket check TYPE.js")?;
    let src = std::fs::read_to_string(p)?;
    let ct = match lang::compile(&src) {
        Ok(ct) => ct,
        Err(d) => bail!("{}", lang::format_diags(&d)),
    };
    let rep = lang::probe::probe(&ct).map_err(|d| anyhow::anyhow!(lang::format_diags(&d)))?;
    println!("ok: {} bounds {:?} tags {:?}; solid from y={:.2} to y={:.2}; max {} ops per sdf call", ct.meta.name, ct.meta.bounds, ct.meta.tags, rep.bottom, rep.top, rep.max_fuel);
    Ok(())
}

fn selftest() -> Result<()> {
    use lang::{compile, probe::probe};
    println!("built-in types:");
    let mut types = Vec::new();
    for (i, src) in model::BUILTIN_SOURCES.iter().enumerate() {
        let ct = compile(src).map_err(|d| anyhow::anyhow!(lang::format_diags(&d)))?;
        let rep = probe(&ct).map_err(|d| anyhow::anyhow!(lang::format_diags(&d)))?;
        println!("  {:<12} ok (probe max fuel {})", ct.meta.name, rep.max_fuel);
        types.push((i as u32 + 1, ct));
    }
    let gpu = render::gpu::Gpu::new()?;
    println!("GPU: {}", gpu.name);
    let refs: Vec<(u32, &lang::CompiledType)> = types.iter().map(|(i, t)| (*i, t)).collect();
    let t0 = Instant::now();
    let pipe = gpu.build_pipeline(&render::shader::assemble(&refs), 1).map_err(|e| anyhow::anyhow!(e))?;
    println!("pipeline built in {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    let terrain = terrain::Terrain::new(4242, terrain::default_biomes());
    for (id, t) in &types {
        model::gpu_parity(&gpu, &pipe, &terrain, *id, t).map_err(|d| anyhow::anyhow!(lang::format_diags(&d)))?;
        println!("  parity {:<12} ok", t.meta.name);
    }
    let pts: Vec<[f32; 4]> = (0..2000).map(|i| {
        let a = i as f32 * 2.399;
        let r = (i as f32).sqrt() * 120.0;
        [a.cos() * r, 0.0, a.sin() * r, 0.0]
    }).collect();
    let out = gpu.probe(&pipe, &model::probe_globals(&terrain), 1, 0, &render::GpuInst::default(), &pts)?;
    let worst = pts.iter().zip(&out).map(|(p, g)| (g[0] - terrain.height(p[0], p[2])).abs()).fold(0.0f32, f32::max);
    println!("  parity terrain      worst |Δh| = {worst:.2e} m ({})", if worst <= 1e-3 { "ok" } else { "FAIL" });
    if worst > 1e-3 {
        bail!("terrain parity failed");
    }
    println!("all good");
    Ok(())
}
