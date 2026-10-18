//! `pocket author DIR`: a world written by hand, made the way every world
//! is made. The folder holds what the builder model would have answered
//! (genesis, each region's plan, each kind of thing's code) and the tool runs
//! the ordinary generation with those files answering in its place: the same
//! seed-made land, the same checks on every type, the same placing, ground,
//! roads and people. Nothing is written into the world any other way.
//!
//! DIR/world.json  { "seed": 7, "prompt": "…", "spawn": [x, z], "regions": [[rx, rz], …],
//!                   "difficulty": "normal" }
//! DIR/genesis.json  the genesis reply's json; DIR/base/*.js its base types
//! DIR/regions/RX_RZ.md  the reply to each region's plan request (a json block)
//! DIR/types/NAME.js  each new kind of thing (or body) a plan names, by its name
//!                    (lower case, spaces and odd characters as '-')
//!
//! A type that fails its checks, a file that is missing, a request the
//! folder can't answer, and every placement the world drops are reported;
//! the build fails if anything was missing or failed. The result is a clean
//! world file (no traveler, no spending), ready to ship.

use crate::brain::{Brain, Cmd, Event};
use crate::db::Db;
use crate::llm::{Llm, Msg};
use crate::model::{Live, WorldModel};
use anyhow::{Context, Result, bail};
use parking_lot::Mutex;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct Spec {
    seed: u32,
    prompt: String,
    #[serde(default)]
    spawn: Option<[f32; 2]>,
    #[serde(default)]
    regions: Vec<[i32; 2]>,
    #[serde(default)]
    difficulty: Option<String>,
}

const USAGE: &str = "usage: pocket author DIR [--out FILE] [--genesis-only]
       pocket author heights FILE x0,z0,x1,z1 STEP
       pocket author heights FILE - < points   (\"x z\" lines)";

pub fn cli(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("heights") => heights(&args[1..]),
        Some(d) if !d.starts_with('-') => {
            let dir = PathBuf::from(d);
            let out = crate::flag(args, "--out").map(PathBuf::from);
            build(&dir, out, crate::has(args, "--genesis-only"))
        }
        _ => bail!(USAGE),
    }
}

/// A type's file name from its name.
pub fn slug(name: &str) -> String {
    let s: String = name.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

#[derive(Default)]
struct Report {
    missing: Vec<String>,
    failed: Vec<String>,
    unknown: Vec<String>,
}

/// The builder's answers, from the folder.
fn answer(dir: &Path, sys: &str, msgs: &[Msg], report: &Mutex<Report>) -> String {
    let user = msgs.first().map(|m| m.text.as_str()).unwrap_or("");
    let read = |p: PathBuf, r: &Mutex<Report>| match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) => {
            r.lock().missing.push(p.display().to_string());
            String::new()
        }
    };
    // A second message is the checker's errors fed back: what is written
    // here is what was meant, so report it and give up on it.
    if msgs.len() > 1 || user.contains("failed validation") {
        let last = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        let name = ["Object type to write: \"", "Body type to write: \""].iter().find_map(|k| user.split(k).nth(1).and_then(|s| s.split('"').next())).or_else(|| user.split("name:").nth(1).and_then(|s| s.split('"').nth(1))).unwrap_or("?");
        let line = format!("{name}: {}", last.chars().take(1200).collect::<String>());
        let mut r = report.lock();
        if !r.failed.iter().any(|f| f.starts_with(&format!("{name}: "))) {
            r.failed.push(line);
        }
        return String::new();
    }
    if user.contains("Design the base layer") {
        // genesis.json and the base types in base/*.js, as one reply.
        let mut reply = format!("```json\n{}\n```\n", read(dir.join("genesis.json"), report));
        let mut base: Vec<PathBuf> = std::fs::read_dir(dir.join("base")).map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "js")).collect()).unwrap_or_default();
        base.sort();
        for p in base {
            reply += &format!("```js\n{}\n```\n", read(p, report));
        }
        return reply;
    }
    if user.contains("Plan the story layer") {
        let r: Vec<i32> = user.split("Region (").nth(1).and_then(|s| s.split(')').next()).map(|s| s.split(", ").filter_map(|v| v.parse().ok()).collect()).unwrap_or_default();
        if r.len() == 2 {
            return read(dir.join("regions").join(format!("{}_{}.md", r[0], r[1])), report);
        }
    }
    if let Some(name) = user.split("Object type to write: \"").nth(1).or_else(|| user.split("Body type to write: \"").nth(1)).and_then(|s| s.split('"').next()) {
        let code = read(dir.join("types").join(format!("{}.js", slug(name))), report);
        return if code.is_empty() { code } else { format!("```js\n{code}\n```") };
    }
    // People's looks are written into the plans; nothing to read.
    if sys.contains("drawn with faces, hair and clothes set by numbers") || user.contains("drawn with faces, hair and clothes set by numbers") {
        return "```json\n{}\n```".into();
    }
    report.lock().unknown.push(user.lines().next().unwrap_or("").chars().take(160).collect());
    String::new()
}

struct Run {
    brain: Brain,
    rx: crossbeam_channel::Receiver<Event>,
}

fn start(db: &Arc<Db>, dir: &Path, report: &Arc<Mutex<Report>>) -> Result<Run> {
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(Live::default()));
    let mut model = WorldModel::load(db.clone(), gpu, live.clone())?;
    let snap = model.snapshot().map_err(|d| anyhow::anyhow!(crate::lang::format_diags(&d)))?;
    // Nobody stands anywhere yet: keep the traveler out of what is placed.
    live.lock().player = snap.spawn + glam::Vec3::new(0.0, -500.0, 0.0);
    let (dir, rep) = (dir.to_path_buf(), report.clone());
    let llm = Llm::scripted(db.clone(), Arc::new(move |s: &str, m: &[Msg]| answer(&dir, s, m, &rep)));
    let (tx, rx) = crossbeam_channel::unbounded();
    let brain = Brain::start(model, Some(llm), tx, false);
    Ok(Run { brain, rx })
}

/// Wait for `done`, printing what the world says meanwhile.
fn wait(rx: &crossbeam_channel::Receiver<Event>, secs: u64, mut done: impl FnMut(&Event) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        let Ok(ev) = rx.recv_timeout(Duration::from_millis(200)) else { continue };
        if let Event::Log(l) = &ev {
            eprintln!("  {l}");
        }
        if done(&ev) {
            return true;
        }
    }
    false
}

fn build(dir: &Path, out: Option<PathBuf>, genesis_only: bool) -> Result<()> {
    let spec: Spec = serde_json::from_str(&std::fs::read_to_string(dir.join("world.json")).with_context(|| format!("read {}", dir.join("world.json").display()))?).context("world.json")?;
    let out = out.unwrap_or_else(|| dir.join(format!("{}.pocket", dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "world".into()))));
    let work = out.with_extension("making.pocket");
    let _ = std::fs::remove_file(&work);
    crate::brain::AUTHORED.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::log::echo(true);
    let t0 = Instant::now();
    let db = Db::create(&work, spec.seed, spec.prompt.trim(), &serde_json::to_string(&crate::world::Look::default())?)?;
    let level = spec.difficulty.as_deref().unwrap_or(crate::NEW_DIFFICULTY).trim().to_lowercase();
    let n = crate::sim::night::LEVELS.iter().position(|l| l.name == level).with_context(|| format!("difficulty is one of peaceful, easy, normal, hard (not {level})"))?;
    db.kv_set("sim.difficulty", &n.to_string())?;
    let report = Arc::new(Mutex::new(Report::default()));

    eprintln!("genesis (seed {})", spec.seed);
    let run = start(&db, dir, &report)?;
    run.brain.send(Cmd::Genesis);
    if !wait(&run.rx, 600, |e| matches!(e, Event::GenesisDone)) {
        bail!("genesis didn't finish: {}", summary(&report.lock()));
    }
    if db.kv_get("genesis").as_deref() != Some("done") {
        bail!("genesis failed: {}", summary(&report.lock()));
    }
    drop(run);
    // Where the traveler begins, if the world says.
    if let Some([x, z]) = spec.spawn {
        let model = WorldModel::load(db.clone(), None, Arc::new(Mutex::new(Live::default())))?;
        let y = model.terrain().height(x, z);
        db.kv_set("spawn", &serde_json::to_string(&[x, y, z])?)?;
    }
    if !genesis_only {
        let run = start(&db, dir, &report)?;
        for [rx, rz] in &spec.regions {
            let r = (*rx, *rz);
            eprintln!("region ({rx}, {rz})");
            run.brain.send(Cmd::Region(r));
            let mut ok = false;
            if !wait(&run.rx, 1800, |e| matches!(e, Event::RegionFinished(rr, o) if { ok = *o; *rr == r })) {
                bail!("region ({rx}, {rz}) didn't finish: {}", summary(&report.lock()));
            }
            if !ok {
                report.lock().failed.push(format!("region ({rx}, {rz}) failed (see above)"));
            }
        }
        drop(run);
    }
    let rep = report.lock();
    if !rep.missing.is_empty() || !rep.failed.is_empty() || !rep.unknown.is_empty() {
        eprintln!("{}", summary(&rep));
    }
    drop(db);
    // A clean copy: no traveler, no spending.
    let _ = std::fs::remove_file(&out);
    let tmp = out.with_extension("clean");
    let _ = std::fs::remove_dir_all(&tmp);
    let made = crate::share::make_copy(&work, &tmp)?;
    std::fs::rename(&made, &out)?;
    let _ = std::fs::remove_dir_all(&tmp);
    let _ = std::fs::remove_file(&work);
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    eprintln!("made {} ({:.1} MB) in {:.0} s", out.display(), size as f64 / 1e6, t0.elapsed().as_secs_f64());
    if !rep.missing.is_empty() || !rep.failed.is_empty() {
        bail!("{} missing, {} failed", rep.missing.len(), rep.failed.len());
    }
    Ok(())
}

fn summary(r: &Report) -> String {
    let mut s = String::new();
    for m in &r.missing {
        s += &format!("\nmissing: {m}");
    }
    for f in &r.failed {
        s += &format!("\nfailed: {f}");
    }
    for u in &r.unknown {
        s += &format!("\nunanswered: {u}");
    }
    s
}

/// The ground's height (and water, where it stands over it) on a grid, to
/// plan against: `pocket author heights FILE x0,z0,x1,z1 STEP`.
fn heights(args: &[String]) -> Result<()> {
    // `-`: points ("x z" lines) from stdin, each answered "x z height water".
    if let (Some(path), Some("-")) = (args.first(), args.get(1).map(String::as_str)) {
        let db = Db::open(Path::new(path))?;
        let model = WorldModel::load(db, None, Arc::new(Mutex::new(Live::default())))?;
        let t = model.terrain();
        let mut out = String::new();
        for line in std::io::read_to_string(std::io::stdin())?.lines() {
            let v: Vec<f32> = line.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            if let [x, z] = v[..] {
                out += &format!("{x} {z} {:.3} {:.3}\n", t.height(x, z), t.water(x, z));
            }
        }
        print!("{out}");
        return Ok(());
    }
    let (Some(path), Some(b), Some(step)) = (args.first(), args.get(1), args.get(2)) else { bail!(USAGE) };
    let b: Vec<f32> = b.split(',').filter_map(|v| v.trim().parse().ok()).collect();
    let step: f32 = step.parse().context("STEP")?;
    if b.len() != 4 || step <= 0.0 {
        bail!(USAGE);
    }
    let db = Db::open(Path::new(path))?;
    let model = WorldModel::load(db, None, Arc::new(Mutex::new(Live::default())))?;
    let t = model.terrain();
    let (x0, z0, x1, z1) = (b[0].min(b[2]), b[1].min(b[3]), b[0].max(b[2]), b[1].max(b[3]));
    // North (+z) at the top, east (+x) to the right.
    print!("{:>7}", "z\\x");
    let mut x = x0;
    while x <= x1 + 1e-3 {
        print!("{x:>7.0}");
        x += step;
    }
    println!();
    let mut z = z1;
    while z >= z0 - 1e-3 {
        print!("{z:>7.0}");
        let mut x = x0;
        while x <= x1 + 1e-3 {
            let h = t.height(x, z);
            let w = t.water(x, z);
            if w > h { print!("{:>6.1}~", h) } else { print!("{h:>7.1}") }
            x += step;
        }
        println!();
        z -= step;
    }
    Ok(())
}

