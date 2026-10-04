//! Proof scenarios: each phase of the emergence plan, as a test. Worlds are
//! built in code (no LLM unless scripted), run headless and checked through
//! the event log and the live state.

use super::actions::Action;
use super::headless::{Session, act_once};
use super::props::*;
use super::{ActorId, Sim, SimEvent, Target};
use crate::db::{self, Db};
use crate::lang::{compile, probe::probe};
use crate::llm::{Llm, Msg};
use crate::terrain::{Terrain, WATER_LEVEL};
use glam::Vec3;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

fn fixture(p: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(p)).unwrap()
}

struct W {
    db: Arc<Db>,
    path: PathBuf,
    terrain: Terrain,
    spawn: Vec3,
    version: i64,
}

/// A fresh universe file with the default land and no LLM-made layer.
fn world(tag: &str, seed: u32) -> W {
    world_with(tag, seed, crate::world::Look::default())
}

/// Low, wet grassland: meadows cut by pools and streams.
fn marsh() -> crate::world::Look {
    let mut b = crate::terrain::default_biomes()[0].clone();
    b.name = "wet meadow".into();
    b.base = 0.9;
    b.amp = 3.0;
    b.rough = 0.1;
    b.scatter = [("grass".to_string(), 3.5), ("bush".to_string(), 0.5)].into_iter().collect();
    crate::world::Look { name: "Marsh".into(), biomes: vec![b], ..Default::default() }
}

fn world_with(tag: &str, seed: u32, look: crate::world::Look) -> W {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pocket-simtest-{}-{tag}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("w.pocket");
    let db = Db::create(&path, seed, "a quiet test valley", &serde_json::to_string(&look).unwrap()).unwrap();
    db.kv_set("genesis", "done").unwrap();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let model = crate::model::WorldModel::load(db.clone(), None, live).unwrap();
    let terrain = (*model.terrain()).clone();
    let spawn = model.spawn;
    let version = db.with(|c| db::add_version(c, None, "region", "test scene", None)).unwrap();
    W { db, path, terrain, spawn, version }
}

fn add_type(w: &W, src: &str) -> u32 {
    let ct = compile(src).unwrap_or_else(|d| panic!("{}", crate::lang::format_diags(&d)));
    let rep = probe(&ct).unwrap_or_else(|d| panic!("{}", crate::lang::format_diags(&d)));
    let meta = serde_json::json!({ "name": ct.meta.name, "bounds": ct.meta.bounds, "tags": ct.meta.tags, "bottom": rep.bottom, "top": rep.top }).to_string();
    w.db.with(|c| db::add_type(c, Some(w.version), &ct.meta.name, src, &meta, "ok", "")).unwrap() as u32
}

fn builtin_id(w: &W, name: &str) -> u32 {
    w.db.types().unwrap().into_iter().find(|t| t.name == name && t.status == "builtin").unwrap().id as u32
}

/// Place an instance standing on the ground.
fn place(w: &W, tid: u32, at: Vec3, rot: f32) -> i64 {
    let src = w.db.types().unwrap().into_iter().find(|t| t.id as u32 == tid).unwrap().code;
    let ct = compile(&src).unwrap();
    let rep = probe(&ct).unwrap();
    let y = w.terrain.height(at.x, at.z) - rep.bottom - 0.02;
    w.db.with(|c| db::add_instance(c, tid as i64, w.version, at.x, y, at.z, rot, 1.0, &[at.x.abs() % 97.0, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5])).unwrap()
}

fn add_char(w: &W, name: &str, personality: &str, relationships: &[&str], home: Vec3) -> i64 {
    let persona = crate::world::Persona { name: name.into(), age: 30, personality: personality.into(), relationships: relationships.iter().map(|s| s.to_string()).collect(), ..Default::default() };
    w.db.with(|c| db::add_character(c, crate::world::region_of(home.x, home.z), &serde_json::to_string(&persona)?, home.x, home.z, w.version)).unwrap()
}

fn session(w: &W, seed: u64, llm: Option<Arc<Llm>>) -> Session {
    let mut s = Session::with_db(w.db.clone(), Some(seed), llm).unwrap();
    s.sim.cfg.llm_per_min = 600.0;
    s
}

fn ground(w: &W, x: f32, z: f32) -> Vec3 {
    Vec3::new(x, w.terrain.height(x, z), z)
}

fn events(sim: &Sim, kind: &str) -> Vec<SimEvent> {
    sim.log.recent.iter().filter(|e| e.kind == kind).cloned().collect()
}

/// Keep every event of a run (the in-memory log only holds the latest).
fn record(s: &mut Session) -> Arc<Mutex<Vec<SimEvent>>> {
    let all: Arc<Mutex<Vec<SimEvent>>> = Arc::new(Mutex::new(Vec::new()));
    struct Rec(Arc<Mutex<Vec<SimEvent>>>, Vec<u8>);
    impl std::io::Write for Rec {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.1.extend_from_slice(b);
            while let Some(i) = self.1.iter().position(|c| *c == b'\n') {
                let line: Vec<u8> = self.1.drain(..=i).collect();
                if let Ok(e) = serde_json::from_slice::<SimEvent>(&line) {
                    self.0.lock().push(e);
                }
            }
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    s.sim.log.sink = Some(Box::new(Rec(all.clone(), Vec::new())));
    all
}

fn sound(s: &Session) {
    let bad = s.sim.check_invariants();
    assert!(bad.is_empty(), "invariants: {bad:?}");
}

fn of(all: &Arc<Mutex<Vec<SimEvent>>>, kind: &str) -> Vec<SimEvent> {
    all.lock().iter().filter(|e| e.kind == kind).cloned().collect()
}

/// A dry, gentle spot `d` metres from the spawn in some direction.
fn dry_spot(w: &W, d: f32, angle: f32) -> Vec3 {
    for k in 0..64 {
        let a = angle + k as f32 * 0.4;
        let p = w.spawn + Vec3::new(a.cos(), 0.0, a.sin()) * d;
        let h = w.terrain.height(p.x, p.z);
        if h > WATER_LEVEL + 0.8 && w.terrain.normal(p.x, p.z).y > 0.92 {
            return Vec3::new(p.x, h, p.z);
        }
    }
    w.spawn
}

// ------------------------------------------------------------------ phase 0

/// The same seed gives the same history, event for event.
#[test]
fn same_seed_same_history() {
    let w = world("det", 31);
    let a = dry_spot(&w, 6.0, 0.0);
    let b = dry_spot(&w, 9.0, 2.0);
    add_char(&w, "Ada", "cheerful and playful", &["Ben: brother"], a);
    add_char(&w, "Ben", "quiet but kind", &["Ada: sister"], b);
    let ball = add_type(&w, &fixture("sims/ball.js"));
    place(&w, ball, dry_spot(&w, 5.0, 1.0), 0.0);
    let run = |w: &W| -> Vec<String> {
        let copy = w.path.with_file_name(format!("copy-{}.pocket", crate::db::now() as u64 % 100000 + rand_n()));
        std::fs::copy(&w.path, &copy).unwrap();
        let db = Db::open(&copy).unwrap();
        let mut s = Session::with_db(db, Some(99), None).unwrap();
        for n in s.sim.cast.npcs.iter_mut() {
            n.needs.fun = 0.9;
            n.needs.social = 0.8;
        }
        let out: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        struct Sink(Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Sink {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        s.sim.log.sink = Some(Box::new(Sink(out.clone())));
        s.run(150.0, 0.1);
        let text = String::from_utf8(out.lock().clone()).unwrap();
        text.lines().map(str::to_string).collect()
    };
    let first = run(&w);
    let second = run(&w);
    assert!(first.len() > 5, "something happened: {first:?}");
    assert_eq!(first, second, "same seed, same history");
}

fn rand_n() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    N.fetch_add(1, std::sync::atomic::Ordering::SeqCst) * 7919
}

// ------------------------------------------------------------------ phase 1

/// The agent drives the player and a character through identical calls.
#[test]
fn agent_drives_player_and_character_the_same_way() {
    let w = world("act", 32);
    let home = dry_spot(&w, 8.0, 1.0);
    let cid = add_char(&w, "Mara", "gruff but kind", &[], home);
    let stick = builtin_id(&w, "stick");
    let s1 = place(&w, stick, w.spawn + Vec3::new(1.0, 0.0, 0.5), 0.0);
    let s2 = place(&w, stick, home + Vec3::new(1.0, 0.0, -0.5), 0.0);
    let s3 = place(&w, stick, home + Vec3::new(10.0, 0.0, 0.0), 0.0);
    let mut s = session(&w, 1, None);
    let me = ActorId::Player;
    let mara = ActorId::Npc(cid);
    if let Some(n) = s.sim.cast.get_mut(cid) {
        n.think_at = f64::MAX;
    }
    let r = act_once(&mut s, me, Action::Hold { target: Target::Instance(s1) }, 0.5);
    assert_eq!(r["ok"], true, "{r}");
    assert!(s.sim.player.held.is_some());
    let r = act_once(&mut s, mara, Action::Hold { target: Target::Instance(s2) }, 0.5);
    assert_eq!(r["ok"], true, "{r}");
    let held = s.sim.cast.get(cid).unwrap().a.held.unwrap();
    assert_eq!(s.sim.things.get(held).unwrap().holder, Some(mara));
    // Too far: drop, then the executor walks there and picks it up.
    let _ = act_once(&mut s, mara, Action::Drop, 0.5);
    let r = act_once(&mut s, mara, Action::Hold { target: Target::Instance(s3) }, 20.0);
    assert_eq!(r["ok"], true, "{r}");
    let held = s.sim.cast.get(cid).unwrap().a.held;
    assert!(held.is_some_and(|h| s.sim.things.get(h).unwrap().origin.instance == Some(s3)), "walked over and picked it up");
    // Same inspect for both.
    let a = s.sim.inspect(&Target::Actor(me));
    let b = s.sim.inspect(&Target::Actor(mara));
    assert_eq!(a["holding"]["name"], "stick");
    assert_eq!(b["holding"]["name"], "stick");
    assert!(b["needs"].is_object() && b["doing"].is_string());
    // Names work as targets too.
    let r = act_once(&mut s, me, Action::Throw { at: Some(Target::Name("Mara".into())), dir: None, force: None }, 4.0);
    assert_eq!(r["ok"], true, "{r}");
    assert!(!events(&s.sim, "threw").is_empty());
}

// ------------------------------------------------------------------ phase 2

/// Throw a stone at an LLM-made lighthouse: it bounces off and comes to rest.
#[test]
fn thrown_stone_bounces_off_a_lighthouse_and_sleeps() {
    let w = world("phys", 33);
    let lh = add_type(&w, &fixture("good/lighthouse.js"));
    let at = dry_spot(&w, 14.0, 0.3);
    let inst = place(&w, lh, at, 0.0);
    let stone = builtin_id(&w, "stone");
    let mut s = session(&w, 2, None);
    let p = s.sim.player.pos;
    let id = s.sim.spawn_thing(stone, p + Vec3::new(0.3, 0.0, 0.3), 0.0, 1.0, Default::default(), true).unwrap();
    let r = act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Thing(id) }, 0.2);
    assert_eq!(r["ok"], true, "{r}");
    let r = act_once(&mut s, ActorId::Player, Action::Throw { at: Some(Target::Instance(inst)), dir: None, force: Some(14.0) }, 10.0);
    assert_eq!(r["ok"], true, "{r}");
    let hits = events(&s.sim, "hit");
    assert!(hits.iter().any(|e| e.data["with"] == "lighthouse"), "hit the lighthouse: {hits:?}");
    let t = s.sim.things.get(id).unwrap();
    assert!(t.asleep, "came to rest: {t:?}\n{:?}", s.sim.log.recent.iter().map(|e| (e.t, e.text.clone())).collect::<Vec<_>>());
    let ty = s.sim.snap.type_of(lh).unwrap().clone();
    let pl = s.sim.snap.instances.iter().find(|p| p.id == inst).unwrap().clone();
    let gi = pl.gpu(&ty, 1.0);
    let (c, r) = t.proxy(s.sim.snap.type_of(stone).unwrap());
    let d = ty.ct.sdf(gi.to_local(c), &gi.k());
    assert!(d > -0.02, "not inside the lighthouse (sdf {d}, r {r})");
    assert!((c - at).length() < 25.0, "bounced back, not through");
}

// ------------------------------------------------------------------ phase 3

/// Find a stretch of water (8–160 m: this land makes lakes, not brooks)
/// between two dry shores. Returns (a dry
/// spot on the near shore, the direction across, distance to the water, and
/// to the far shore).
fn shore(w: &W) -> Option<(Vec3, Vec3, f32, f32)> {
    let dirs: Vec<Vec3> = (0..8).map(|i| {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        Vec3::new(a.cos(), 0.0, a.sin())
    }).collect();
    for i in 0..3000 {
        let a = i as f32 * 2.399;
        let r = (i as f32).sqrt() * 8.0;
        let p = w.spawn + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        if w.terrain.height(p.x, p.z) < WATER_LEVEL + 0.3 {
            continue;
        }
        for dir in &dirs {
            let h = |k: f32| {
                let q = p + *dir * k;
                w.terrain.height(q.x, q.z)
            };
            let Some(k1) = (1..60).map(|k| k as f32).find(|k| h(*k) < WATER_LEVEL) else { continue };
            let Some(k2) = (k1 as i32..k1 as i32 + 160).map(|k| k as f32).find(|k| h(*k) > WATER_LEVEL + 0.15) else { continue };
            let width = k2 - k1;
            if !(8.0..=160.0).contains(&width) || k1 < 12.0 {
                continue;
            }
            // Dry all the way from 12 m before the water to the water.
            if (0..12).any(|j| h(k1 - 12.0 + j as f32) < WATER_LEVEL) {
                continue;
            }
            let start = p + *dir * (k1 - 10.0);
            return Some((Vec3::new(start.x, w.terrain.height(start.x, start.z), start.z), *dir, 10.0, 10.0 + width));
        }
    }
    None
}

/// Plant grass tufts every 2.5 m over dry ground in a box along `dir`.
fn plant_grass(w: &W, origin: Vec3, dir: Vec3, from: f32, to: f32, half_width: f32) -> usize {
    let grass = builtin_id(w, "grass tuft");
    let side = Vec3::new(-dir.z, 0.0, dir.x);
    let mut n = 0;
    let mut a = from;
    while a <= to {
        let mut b = -half_width;
        while b <= half_width {
            let p = origin + dir * a + side * b;
            if w.terrain.height(p.x, p.z) > WATER_LEVEL + 0.05 {
                place(w, grass, p, a * 1.3 + b);
                n += 1;
            }
            b += 2.5;
        }
        a += 2.5;
    }
    n
}

/// A lantern dropped in dry grass breaks and spills fire; the fire spreads
/// through the grass to a wooden hut and stops at the water.
#[test]
fn dropped_lantern_starts_a_fire_that_spreads_to_a_hut_and_stops_at_water() {
    let mut found = None;
    for seed in 41u32..80 {
        let w = if seed % 2 == 0 { world_with("fire", seed, marsh()) } else { world("fire", seed) };
        if let Some(sh) = shore(&w).filter(|s| s.3 - s.2 <= 40.0) {
            found = Some((w, sh));
            break;
        }
    }
    let (w, (a, dir, wstart, wend)) = found.expect("water between two shores");
    // Dry grass up to the water's edge on this side, and on the far side.
    assert!(plant_grass(&w, a, dir, -6.0, wstart - 0.5, 6.0) > 10);
    assert!(plant_grass(&w, a, dir, wend + 0.5, wend + 8.0, 6.0) > 5);
    let lantern = add_type(&w, &fixture("sims/lantern.js"));
    let hut = add_type(&w, &fixture("sims/hut.js"));
    let side = Vec3::new(-dir.z, 0.0, dir.x);
    let hut_at = ground(&w, a.x - dir.x * 6.5 + side.x * 2.0, a.z - dir.z * 6.5 + side.z * 2.0);
    let hut_id = place(&w, hut, hut_at, 0.0);
    let mut s = session(&w, 3, None);
    s.sim.player.pos = a - dir * 30.0;
    // Only the planted grass and the hut can burn: clear the wild scatter
    // around, so fire can't walk around the lake.
    let snap = s.sim.snap.clone();
    for it in s.sim.cache.items_near(&snap, a + dir * (wend * 0.5), wend + 60.0) {
        s.sim.cache.overlay.taken.insert(it.cell);
        s.sim.things.taken.insert(it.cell, -1);
    }
    // Drop it from a height onto the grass.
    let id = s.sim.spawn_thing(lantern, a + Vec3::Y * 3.0, 0.0, 1.0, Default::default(), false).unwrap();
    s.sim.things.get_mut(id).unwrap().vel = Vec3::new(0.0, -6.0, 0.0);
    s.sim.things.get_mut(id).unwrap().asleep = false;
    s.sim.things.get_mut(id).unwrap().thrown_by = Some((ActorId::Player, s.sim.t));
    let all = record(&mut s);
    s.run(150.0, 0.1);
    assert!(!of(&all, "broke").is_empty(), "the lantern broke");
    let ignited = of(&all, "ignited");
    assert!(ignited.len() >= 5, "the grass caught: {} ignitions", ignited.len());
    let hut_thing = s.sim.things.by_instance.get(&hut_id).copied().expect("the hut joined the live layer");
    assert!(ignited.iter().any(|e| e.subject.as_deref() == Some(&format!("thing:{hut_thing}"))), "the hut caught fire");
    // It burnt right up to the water, and nothing beyond it.
    let reach = ignited.iter().map(|e| (Vec3::from(e.pos.unwrap()) - a).dot(dir)).fold(f32::MIN, f32::max);
    assert!(reach > wstart - 3.5, "burnt up to the shore ({reach:.1} m, water at {wstart} m)");
    assert!(reach < wstart + 1.0, "and stopped there ({reach:.1} m)");
    for e in &ignited {
        let p = Vec3::from(e.pos.unwrap());
        let along = (p - a).dot(dir);
        assert!(along < wend, "fire crossed the water: {} at {along:.1} m (water {wstart}–{wend} m)", e.text);
    }
    assert!(!of(&all, "burnt_out").is_empty(), "grass burnt out");
    sound(&s);
    assert!(s.sim.things.live().any(|t| t.props[P_CHAR] > 0.9), "what burnt is charred");
    eprintln!("fire: {} ignitions, reached {reach:.1} m, water from {wstart} to {wend} m, {} events in all", ignited.len(), all.lock().len());
}

/// A universe's own property and rule: a curse spreads by touch; and a rule
/// that would spread to everything at once is rejected.
#[test]
fn universe_rules_spread_cursed_and_explosive_rules_are_rejected() {
    use super::rules::RuleSpec;
    let w = world("curse", 34);
    let spec = RuleSpec { name: "curses spread by touch".into(), near: Some(1.5), when: "self.cursed > 0.5 && other.cursed < self.cursed".into(), effects: vec!["other.cursed += 0.1 * dt".into()] };
    let (props, rules) = crate::brain::check_universe_rules(&[("cursed".into(), 0.0, "how cursed it is".into())], &[spec.clone()]);
    assert_eq!(rules.len(), 1);
    super::persist::set_universe_rules(&w.db, &props, &rules).unwrap();
    let boom = RuleSpec { name: "plague".into(), near: Some(10.0), when: "other.cursed < 1".into(), effects: vec!["other.cursed = 1".into()] };
    let (_, kept) = crate::brain::check_universe_rules(&[("cursed".into(), 0.0, "".into())], &[boom]);
    assert!(kept.is_empty(), "an instant plague is not a world");
    let idol_src = r#"
export const meta = { name: "idol", bounds: [0.2, 0.3, 0.2], tags: ["item"], props: { cursed: 1, mass: 2 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.15, 0.25, 0.15, 0.03); }
export function color(x, y, z, k) { return rgb(80, 20, 90); }
"#;
    let idol = add_type(&w, idol_src);
    let stick = builtin_id(&w, "stick");
    let p = dry_spot(&w, 6.0, 0.5);
    let mut s = session(&w, 4, None);
    assert!(s.sim.vocab.id("cursed").is_some());
    let a = s.sim.spawn_thing(idol, p, 0.0, 1.0, Default::default(), true).unwrap();
    let b = s.sim.spawn_thing(stick, p + Vec3::new(0.8, 0.0, 0.0), 0.0, 1.0, Default::default(), true).unwrap();
    let c = s.sim.spawn_thing(stick, p + Vec3::new(8.0, 0.0, 0.0), 0.0, 1.0, Default::default(), true).unwrap();
    s.run(20.0, 0.1);
    let ci = s.sim.vocab.id("cursed").unwrap();
    assert!(s.sim.things.get(a).unwrap().props[ci] > 0.9);
    assert!(s.sim.things.get(b).unwrap().props[ci] > 0.3, "touching things get cursed");
    assert!(s.sim.things.get(c).unwrap().props[ci] < 0.01, "far things do not");
    let v = s.sim.inspect(&Target::Thing(b));
    assert!(v["rules_fired"].to_string().contains("curses spread"), "{v}");
}

// ------------------------------------------------------------------ phase 4

/// Behaviour code: an acorn left on the ground turns into a sapling, which grows.
#[test]
fn an_acorn_on_the_ground_becomes_a_sapling_and_grows() {
    let w = world("seed", 35);
    let acorn = add_type(&w, &fixture("sims/seed.js"));
    let sapling = add_type(&w, &fixture("sims/sapling.js"));
    let p = dry_spot(&w, 5.0, 2.0);
    let mut s = session(&w, 5, None);
    let id = s.sim.spawn_thing(acorn, p, 0.0, 1.0, Default::default(), true).unwrap();
    s.run(30.0, 0.1);
    let t = s.sim.things.get(id).unwrap();
    assert_eq!(t.type_id, sapling, "transformed");
    assert!(!events(&s.sim, "transformed").is_empty());
    let g0 = t.props[P_GROWTH];
    s.run(30.0, 0.1);
    let g1 = s.sim.things.get(id).unwrap().props[P_GROWTH];
    assert!(g1 > g0, "living things grow ({g0} → {g1})");
}

// ------------------------------------------------------------------ phase 5

fn interp_script(calls: Arc<Mutex<usize>>) -> Arc<Llm> {
    let w = world("interp-llm", 1);
    Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("physics and common sense") {
            *calls.lock() += 1;
            assert!(user.contains("flint") && user.contains("lantern"), "{user}");
            return r#"```json
{"narration": "Sparks fly and the wick catches.", "changes": [{"target": "target", "props": {"light": 1, "heat": 150}}], "create": [], "remove": [], "say": null, "cache": true}
```"#
                .into();
        }
        r#"{"goal": "", "steps": []}"#.into()
    }))
}

/// "Use flint on lantern": no rule covers it, so the interpreter decides; the
/// second time, the cached answer is used without asking.
#[test]
fn interpreter_lights_the_lantern_and_caches_the_answer() {
    let w = world("interp", 36);
    let flint = add_type(&w, &fixture("sims/flint.js"));
    let src = fixture("sims/lantern.js").replace("light: 1, heat: 150", "light: 0, heat: 0");
    let lantern = add_type(&w, &src);
    let calls = Arc::new(Mutex::new(0usize));
    let llm = interp_script(calls.clone());
    let mut s = session(&w, 6, Some(llm));
    let p = s.sim.player.pos;
    let f = s.sim.spawn_thing(flint, p + Vec3::new(0.4, 0.0, 0.0), 0.0, 1.0, Default::default(), true).unwrap();
    let l1 = s.sim.spawn_thing(lantern, p + Vec3::new(0.0, 0.0, 0.8), 0.0, 1.0, Default::default(), true).unwrap();
    let l2 = s.sim.spawn_thing(lantern, p + Vec3::new(-0.6, 0.0, 0.6), 0.0, 1.0, Default::default(), true).unwrap();
    act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Thing(f) }, 0.2);
    let r = act_once(&mut s, ActorId::Player, Action::Use { target: None, on: Some(Target::Thing(l1)), at: None }, 1.0);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(*calls.lock(), 1);
    assert_eq!(s.sim.things.get(l1).unwrap().props[P_LIGHT], 1.0, "lit");
    assert!(r["heard"].to_string().contains("wick catches"), "{r}");
    let r = act_once(&mut s, ActorId::Player, Action::Use { target: None, on: Some(Target::Thing(l2)), at: None }, 1.0);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(*calls.lock(), 1, "cached: no second call");
    assert_eq!(s.sim.things.get(l2).unwrap().props[P_LIGHT], 1.0, "the same cause, the same effect");
    assert!(s.sim.interp.hits >= 1);
    // A soaked lantern is another matter: not answered from the dry one's case.
    let l3 = s.sim.spawn_thing(lantern, p + Vec3::new(0.6, 0.0, -0.6), 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.things.get_mut(l3).unwrap().props[P_WET] = 0.6;
    act_once(&mut s, ActorId::Player, Action::Use { target: None, on: Some(Target::Thing(l3)), at: None }, 1.0);
    assert_eq!(*calls.lock(), 2, "a wet lantern is asked about afresh");
    sound(&s);
}

// ------------------------------------------------------------------ phase 6

fn play_script(ball_src: String, calls: Arc<Mutex<Vec<String>>>) -> Arc<Llm> {
    let w = world("play-llm", 1);
    Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You decide what a character") {
            calls.lock().push(user.lines().next().unwrap_or("").to_string());
            if user.contains("bored") {
                return r#"{"goal": "make something to play with", "say": "I'll make a ball.", "steps": [{"do": "create", "text": "a leather ball"}]}"#.into();
            }
            return r#"{"goal": "", "steps": []}"#.into();
        }
        if sys.contains("physics and common sense") && user.contains("ball") {
            calls.lock().push("interpret".into());
            return r#"{"narration": "", "make": [{"text": "a leather ball"}]}"#.into();
        }
        if user.contains("is making something in the world") {
            calls.lock().push("create".into());
            return format!("```json\n{{\"summary\": \"a leather ball\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": -4}}]}}\n```\n```js\n{ball_src}\n```");
        }
        r#"{"lines": []}"#.into()
    }))
}

/// Nobody wrote "basketball": a bored character makes a ball, and people who
/// like to play start throwing it at a hoop.
#[test]
fn a_made_ball_ends_up_thrown_at_a_hoop() {
    let w = world("hoop", 37);
    let hoop = add_type(&w, &fixture("sims/hoop.js"));
    let court = dry_spot(&w, 10.0, 0.7);
    let hoop_id = place(&w, hoop, court, 0.0);
    let a = add_char(&w, "Tam", "a restless tinkerer, playful", &["Bren: rival"], court + Vec3::new(6.0, 0.0, 2.0));
    let b = add_char(&w, "Bren", "a playful, lively youth", &["Tam: rival"], court + Vec3::new(-5.0, 0.0, 3.0));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let llm = play_script(fixture("sims/ball.js"), calls.clone());
    let mut s = session(&w, 7, Some(llm));
    s.sim.player.pos = court + Vec3::new(0.0, 0.0, -14.0);
    for (cid, fun) in [(a, 0.97), (b, 0.9)] {
        let n = s.sim.cast.get_mut(cid).unwrap();
        n.needs.fun = fun;
        n.needs.hunger = 0.0;
        n.needs.social = 0.0;
        n.needs.curiosity = 0.0;
    }
    let mut made = false;
    let mut thrown_at_hoop = Vec::new();
    for _ in 0..(400.0 / 0.1) as usize {
        s.step(0.1);
        made |= !events(&s.sim, "made").is_empty();
        thrown_at_hoop = events(&s.sim, "threw").into_iter().filter(|e| e.data["at"] == "hoop").collect();
        if made && thrown_at_hoop.len() >= 3 {
            break;
        }
        // Keep it daytime.
        if s.sim.night() {
            s.sim.t += crate::render::sky::DAY_SECONDS * 0.5;
        }
    }
    let log: Vec<String> = s.sim.log.recent.iter().map(|e| format!("{:.0} {} {}", e.t, e.kind, e.text)).collect();
    assert!(made, "Tam made a ball: calls {:?}\n{}", calls.lock(), log.join("\n"));
    assert!(!thrown_at_hoop.is_empty(), "someone threw it at the hoop:\n{}", log.join("\n"));
    let _ = hoop_id;
    eprintln!("throws at the hoop: {}, through: {}", thrown_at_hoop.len(), events(&s.sim, "through").len());
    sound(&s);
}

/// Asked in conversation for a ball, a character makes one and hands it over
/// once it is made (the giving waits for the making).
#[test]
fn asked_for_a_ball_they_make_it_and_hand_it_over() {
    let w = world("asked", 38);
    let spot = dry_spot(&w, 10.0, 0.4);
    let a = add_char(&w, "Tam", "a kind toymaker", &[], spot);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let ball = fixture("sims/ball.js");
    let c2 = calls.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You decide what a character") {
            if user.contains("The traveler just said") {
                c2.lock().push("asked".to_string());
                return r#"{"goal": "make the traveler a ball", "say": null, "steps": [{"do": "create", "text": "a leather ball"}, {"do": "give", "to": "the traveler"}]}"#.into();
            }
            return r#"{"goal": "", "steps": []}"#.into();
        }
        if sys.contains("physics and common sense") && user.contains("ball") {
            return r#"{"narration": "", "make": [{"text": "a leather ball"}]}"#.into();
        }
        if user.contains("is making something in the world") {
            return format!("```json\n{{\"summary\": \"a leather ball\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": 1}}]}}\n```\n```js\n{ball}\n```");
        }
        r#"{"lines": []}"#.into()
    }));
    let mut s = session(&w, 8, Some(llm));
    s.sim.player.pos = spot + Vec3::new(0.0, 0.0, -3.0);
    s.sim.asked(a, "Could you make me a ball?", "Of course, I'll make you one.");
    let mut got = None;
    for _ in 0..(120.0 / 0.1) as usize {
        s.step(0.1);
        if let Some(h) = s.sim.player.held {
            got = Some(h);
            break;
        }
    }
    let log: Vec<String> = s.sim.log.recent.iter().map(|e| format!("{:.0} {} {}", e.t, e.kind, e.text)).collect();
    assert_eq!(calls.lock().as_slice(), ["asked"], "{}", log.join("\n"));
    let h = got.unwrap_or_else(|| panic!("the traveler was handed the ball:\n{}", log.join("\n")));
    assert!(s.sim.thing_name(h).contains("ball"), "{}", s.sim.thing_name(h));
    sound(&s);
}

/// A cook asked for a burrito with no grill at hand walks to the grill,
/// makes it there and hands it over; the "not here" answer isn't kept.
#[test]
fn a_deed_that_needs_a_place_is_done_there() {
    let w = world("grill", 38);
    let spot = dry_spot(&w, 10.0, 0.4);
    let a = add_char(&w, "Vic", "a cook who makes burritos for everyone", &[], spot);
    let grill_src = fixture("sims/hut.js").replacen("name: \"wooden hut\"", "name: \"grill\"", 1);
    let grill = add_type(&w, &grill_src);
    let far = spot + Vec3::new(30.0, 0.0, 0.0);
    place(&w, grill, far, 0.0);
    let burrito = fixture("sims/ball.js").replacen("name: \"leather ball\"", "name: \"burrito\"", 1);
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You decide what a character") {
            if user.contains("The traveler just said") {
                return r#"{"goal": "make the traveler a burrito", "say": null, "steps": [{"do": "create", "text": "a burrito"}, {"do": "give", "to": "the traveler"}]}"#.into();
            }
            return r#"{"goal": "", "steps": []}"#.into();
        }
        if sys.contains("physics and common sense") && user.contains("burrito") {
            if user.contains("\"name\":\"grill\"") {
                return r#"{"narration": "Vic grills one.", "make": [{"text": "a burrito"}]}"#.into();
            }
            return r#"{"narration": "No grill here.", "needs": {"kind": "place", "what": "grill"}}"#.into();
        }
        if user.contains("is making something in the world") {
            return format!("```json\n{{\"summary\": \"a burrito\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": 1}}]}}\n```\n```js\n{burrito}\n```");
        }
        r#"{"lines": []}"#.into()
    }));
    let mut s = session(&w, 8, Some(llm));
    s.sim.player.pos = spot + Vec3::new(0.0, 0.0, -3.0);
    s.sim.asked(a, "Could you make me a burrito?", "Sure, coming up.");
    let mut got = None;
    for _ in 0..(240.0 / 0.1) as usize {
        s.step(0.1);
        if let Some(h) = s.sim.player.held {
            got = Some(h);
            break;
        }
    }
    let log: Vec<String> = s.sim.log.recent.iter().map(|e| format!("{:.0} {} {}", e.t, e.kind, e.text)).collect();
    let h = got.unwrap_or_else(|| panic!("the traveler was handed a burrito:\n{}", log.join("\n")));
    assert!(s.sim.thing_name(h).contains("burrito"), "{}", s.sim.thing_name(h));
    let refusals: i64 = w.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM interp_cache WHERE effect_json LIKE '%No grill%'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(refusals, 0, "the refusal was not cached");
    sound(&s);
}

/// The cook, a busy carpenter and a handy welder; no grill anywhere.
fn cooks_town(name: &str) -> (W, Vec3, i64) {
    let w = world(name, 38);
    let spot = dry_spot(&w, 10.0, 0.4);
    let vic = add_char(&w, "Vic", "a cook who makes burritos for everyone", &[], spot);
    add_char(&w, "Rosa", "a busy carpenter", &[], spot + Vec3::new(6.0, 0.0, 0.0));
    add_char(&w, "Ben", "a handy, generous welder", &[], spot + Vec3::new(-6.0, 0.0, 0.0));
    (w, spot, vic)
}

/// Rosa says no to anything; Ben builds a grill; Vic cooks at a grill.
fn cooks_llm(w: &W) -> Arc<Llm> {
    let grill_src = fixture("sims/hut.js").replacen("name: \"wooden hut\"", "name: \"grill\"", 1);
    let burrito = fixture("sims/ball.js").replacen("name: \"leather ball\"", "name: \"burrito\"", 1);
    Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You decide what a character") {
            if user.contains("The traveler just said") {
                return r#"{"goal": "make the traveler a burrito", "say": null, "steps": [{"do": "create", "text": "a burrito"}, {"do": "give", "to": "the traveler"}]}"#.into();
            }
            if user.contains("You set out to") {
                return r#"{"goal": "get a grill", "say": "I need a grill.", "steps": [{"do": "ask", "who": ["Rosa", "Ben"], "for": "a grill"}]}"#.into();
            }
            if user.contains("asks you for") && user.contains("\"character\": \"Rosa\"") {
                return r#"{"goal": "", "say": "Sorry, I'm busy.", "steps": []}"#.into();
            }
            if user.contains("asks you for") && user.contains("\"character\": \"Ben\"") {
                return r#"{"goal": "build Vic a grill", "say": "Sure thing.", "steps": [{"do": "make", "what": "a grill"}, {"do": "give", "to": "Vic"}]}"#.into();
            }
            return r#"{"goal": "", "steps": []}"#.into();
        }
        if sys.contains("physics and common sense") {
            if user.contains("does this: \"make a burrito") {
                if user.contains("\"name\":\"grill\"") {
                    return r#"{"narration": "Vic grills one.", "make": [{"text": "a burrito"}]}"#.into();
                }
                return r#"{"narration": "No grill here.", "needs": {"kind": "place", "what": "a grill"}}"#.into();
            }
            if user.contains("does this: \"make a grill") {
                return r#"{"narration": "Ben welds a grill.", "make": [{"text": "a grill"}]}"#.into();
            }
        }
        if user.contains("is making something in the world") {
            // The burrito on the cook's side of the (hut-sized) grill, the grill out in front of the welder.
            let (src, right, fwd) = if user.contains("burrito") { (&burrito, 0, -3) } else { (&grill_src, 0, 4) };
            return format!("```json\n{{\"summary\": \"it\", \"reuse\": null, \"placements\": [{{\"right\": {right}, \"forward\": {fwd}}}]}}\n```\n```js\n{src}\n```");
        }
        r#"{"lines": []}"#.into()
    }))
}

/// With no grill anywhere, the cook asks people in turn: the first says no,
/// the second builds one, and the cook makes the burrito there and hands it over.
#[test]
fn a_need_nobody_has_is_asked_for_in_turn() {
    let (w, spot, vic) = cooks_town("askround");
    let llm = cooks_llm(&w);
    let mut s = session(&w, 8, Some(llm));
    s.sim.player.pos = spot + Vec3::new(0.0, 0.0, -3.0);
    s.sim.asked(vic, "Could you make me a burrito?", "Sure, coming up.");
    let mut got = None;
    for _ in 0..(400.0 / 0.1) as usize {
        s.step(0.1);
        if let Some(h) = s.sim.player.held {
            got = Some(h);
            break;
        }
    }
    let log: Vec<String> = s.sim.log.recent.iter().map(|e| format!("{:.0} {} {}", e.t, e.kind, e.text)).collect();
    let h = got.unwrap_or_else(|| panic!("the traveler was handed a burrito:\n{}", log.join("\n")));
    assert!(s.sim.thing_name(h).contains("burrito"), "{}", s.sim.thing_name(h));
    assert!(log.iter().any(|l| l.contains("Rosa wouldn't help Vic")), "Rosa was asked first and said no:\n{}", log.join("\n"));
    assert!(log.iter().any(|l| l.contains("Ben agreed to help Vic")), "{}", log.join("\n"));
    assert!(s.sim.cast.get(vic).unwrap().mission.is_none(), "the mission is over");
    sound(&s);
}

/// Quitting mid-mission and coming back: the cook still asks, gets the grill
/// built, and hands the burrito over.
#[test]
fn a_mission_carries_on_after_a_restart() {
    let (w, spot, vic) = cooks_town("restart");
    let mut s = session(&w, 8, Some(cooks_llm(&w)));
    s.sim.player.pos = spot + Vec3::new(0.0, 0.0, -3.0);
    s.sim.asked(vic, "Could you make me a burrito?", "Sure, coming up.");
    let mut asking = false;
    for _ in 0..(120.0 / 0.1) as usize {
        s.step(0.1);
        if s.sim.cast.get(vic).unwrap().mission.as_ref().is_some_and(|m| matches!(m.stage, super::needs::Stage::Asking(_))) {
            asking = true;
            break;
        }
    }
    assert!(asking, "Vic set out to ask someone");
    s.save();
    drop(s);
    let mut s = session(&w, 8, Some(cooks_llm(&w)));
    assert!(s.sim.cast.get(vic).unwrap().mission.is_some(), "the mission was kept");
    let mut got = None;
    for _ in 0..(400.0 / 0.1) as usize {
        s.step(0.1);
        if let Some(h) = s.sim.player.held {
            got = Some(h);
            break;
        }
    }
    let log: Vec<String> = s.sim.log.recent.iter().map(|e| format!("{:.0} {} {}", e.t, e.kind, e.text)).collect();
    let h = got.unwrap_or_else(|| panic!("the traveler was handed a burrito after the restart:\n{}", log.join("\n")));
    assert!(s.sim.thing_name(h).contains("burrito"), "{}", s.sim.thing_name(h));
    sound(&s);
}

/// A deed that can only be done at an hour waits for it, then is done.
#[test]
fn a_deed_for_a_later_hour_waits_for_it() {
    let w = world("hour", 38);
    let spot = dry_spot(&w, 10.0, 0.4);
    let a = add_char(&w, "Ysolde", "a hedge witch", &[], spot);
    let water = fixture("sims/ball.js").replacen("name: \"leather ball\"", "name: \"moon water\"", 1);
    let tries = Arc::new(Mutex::new(0));
    let t2 = tries.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("physics and common sense") && user.contains("moon water") {
            let mut n = t2.lock();
            *n += 1;
            if *n == 1 {
                return r#"{"narration": "Only when the sun is low.", "needs": {"kind": "time", "what": "late afternoon", "hour": 16}}"#.into();
            }
            return r#"{"narration": "She draws it.", "make": [{"text": "moon water"}]}"#.into();
        }
        if user.contains("is making something in the world") {
            return format!("```json\n{{\"summary\": \"moon water\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": 1}}]}}\n```\n```js\n{water}\n```");
        }
        if sys.contains("You decide what a character") {
            return r#"{"goal": "", "steps": []}"#.into();
        }
        r#"{"lines": []}"#.into()
    }));
    let mut s = session(&w, 8, Some(llm));
    s.sim.player.pos = spot + Vec3::new(0.0, 0.0, -3.0);
    let me = ActorId::Npc(a);
    // An hour before it can be done, still up and about.
    s.sim.t = crate::render::sky::DAY_SECONDS * 15.0 / 24.0;
    s.sim.plan(me, vec![Action::Do { text: "make moon water".into(), on: None, at: None }], "moon water", true);
    let mut made_at = None;
    for _ in 0..(200.0 / 0.1) as usize {
        s.step(0.1);
        if s.sim.log.recent.iter().any(|e| e.kind == "made") {
            made_at = Some(s.sim.hour());
            break;
        }
    }
    let log: Vec<String> = s.sim.log.recent.iter().map(|e| format!("{:.0} {} {}", e.t, e.kind, e.text)).collect();
    let h = made_at.unwrap_or_else(|| panic!("the moon water was made:\n{}", log.join("\n")));
    assert_eq!(*tries.lock(), 2, "tried once, then again at the hour");
    assert!(h >= 15.95, "made at the hour, at {h:.1}");
    sound(&s);
}

#[test]
fn ask_steps_parse() {
    assert_eq!(
        crate::sim::npc::parse_step(&serde_json::json!({"do": "ask", "who": ["Rosa", "Ben"], "for": "a grill"})),
        Some(Action::Ask { who: vec![Target::Name("Rosa".into()), Target::Name("Ben".into())], what: "a grill".into() })
    );
    assert_eq!(crate::sim::npc::parse_step(&serde_json::json!({"do": "ask", "to": "Rosa", "what": "a ladder"})), Some(Action::Ask { who: vec![Target::Name("Rosa".into())], what: "a ladder".into() }));
}

// ------------------------------------------------------------------ phase 7

/// Two characters who love each other play catch; two others carry a log
/// that neither can lift alone; a couple hugs.
#[test]
fn people_play_catch_carry_together_and_hug() {
    let w = world("together", 38);
    let p = dry_spot(&w, 8.0, 0.2);
    let mum = add_char(&w, "Ilse", "warm and playful", &["Ola: daughter"], p);
    let kid = add_char(&w, "Ola", "a lively, playful girl", &["Ilse: mother"], p + Vec3::new(6.0, 0.0, 0.0));
    let q = dry_spot(&w, 30.0, 2.5);
    let x = add_char(&w, "Bo", "kind and generous", &["Rune: friend"], q);
    let y = add_char(&w, "Rune", "generous, steady", &["Bo: friend"], q + Vec3::new(3.0, 0.0, 0.0));
    let ball = add_type(&w, &fixture("sims/ball.js"));
    let log = add_type(&w, &fixture("sims/log.js"));
    let ball_id = place(&w, ball, p + Vec3::new(2.0, 0.0, 1.0), 0.0);
    let log_id = place(&w, log, q + Vec3::new(1.5, 0.0, 2.0), 0.0);
    let mut s = session(&w, 8, None);
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -15.0);
    for n in s.sim.cast.npcs.iter_mut() {
        n.needs = crate::world::characters::Needs { hunger: 0.0, fatigue: 0.0, social: 0.0, fun: 0.2, curiosity: 0.0 };
        n.think_at = f64::MAX;
    }
    let (m, o) = (ActorId::Npc(mum), ActorId::Npc(kid));
    let ball_t = s.sim.liven(&Target::Instance(ball_id)).unwrap();
    s.sim.cast.get_mut(kid).unwrap().needs.fun = 0.9;
    let r = s.sim.propose(m, o, "catch", Some(ball_t));
    assert!(r.is_ok(), "{r:?}");
    s.run(60.0, 0.1);
    let caught = events(&s.sim, "caught");
    assert!(caught.len() >= 2, "they played catch: {:?}", s.sim.log.recent.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    assert!(s.sim.social.affection(m, o) > 0.75);
    // Carry: neither can lift it alone.
    let (bo, rune) = (ActorId::Npc(x), ActorId::Npc(y));
    let log_t = s.sim.liven(&Target::Instance(log_id)).unwrap();
    let start = s.sim.things.get(log_t).unwrap().pos;
    let r = s.sim.act(bo, Action::Hold { target: Target::Thing(log_t) });
    let _ = r;
    s.sim.release(log_t);
    s.sim.propose(bo, rune, "carry", Some(log_t)).unwrap();
    s.run(60.0, 0.1);
    assert!(!events(&s.sim, "carry").is_empty(), "lifted together: {:?}", s.sim.log.recent.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    let end = s.sim.things.get(log_t).unwrap().pos;
    assert!((end - start).length() > 3.0, "and carried it ({:.1} m)", (end - start).length());
    // A hug between mother and daughter.
    let r = s.sim.act(m, Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(o)) });
    assert!(r.is_ok(), "{r:?}");
    s.run(30.0, 0.1);
    assert!(!events(&s.sim, "hug").is_empty(), "they hugged");
    // Rivals refuse.
    let r = s.sim.act(bo, Action::Gesture { kind: "kiss".into(), to: Some(Target::Actor(o)) });
    assert!(r.is_err(), "a kiss needs love");
    sound(&s);
}

// ------------------------------------------------------------------ phase 8

/// 1,000 things and 50 characters run headless well above real time.
#[test]
fn a_thousand_things_and_fifty_people_run_fast() {
    let w = world("scale", 39);
    for i in 0..50 {
        let a = i as f32 * 0.7;
        let p = dry_spot(&w, 20.0 + (i % 10) as f32 * 8.0, a);
        add_char(&w, &format!("Villager {i}"), if i % 3 == 0 { "playful" } else { "kind" }, &[], p);
    }
    let mut s = session(&w, 9, None);
    let stone = s.sim.type_by_name("stone").unwrap().id;
    let ball = add_type(&w, &fixture("sims/ball.js"));
    let _ = ball;
    for i in 0..1000 {
        let a = i as f32 * 2.399;
        let r = (i as f32).sqrt() * 2.5;
        let p = s.sim.player.pos + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        s.sim.spawn_thing(stone, p, a, 1.0, Default::default(), true);
    }
    assert!(s.sim.things.len() >= 990);
    let t0 = std::time::Instant::now();
    let game = 60.0;
    s.run(game, 0.1);
    let real = t0.elapsed().as_secs_f32();
    eprintln!("1000 things, 50 people: {game} s in {real:.2} s ({:.0}×)", game / real);
    assert!(game / real > 10.0, "{:.1}× real time", game / real);
    sound(&s);
}

/// Leave a village for two game days (far mode: catch up); when you come
/// back, the campfire you left burning has burnt out and people have lived
/// their days.
#[test]
fn a_village_left_for_two_days_has_changed() {
    let w = world("away", 40);
    let p = dry_spot(&w, 10.0, 1.3);
    let cid = add_char(&w, "Wren", "curious", &[], p + Vec3::new(8.0, 0.0, 0.0));
    let mut s = session(&w, 10, None);
    s.sim.cfg.far = super::config::FarMode::CatchUp { max_hours: 72.0 };
    s.sim.cfg.near = 120.0;
    s.sim.cfg.medium = 200.0;
    // A lone campfire (nothing else around to catch).
    let snap = s.sim.snap.clone();
    for it in s.sim.cache.items_near(&snap, p, 30.0) {
        s.sim.cache.overlay.taken.insert(it.cell);
        s.sim.things.taken.insert(it.cell, -1);
    }
    let stick = s.sim.type_by_name("stick").unwrap().id;
    let fire = s.sim.spawn_thing(stick, p, 0.0, 2.0, Default::default(), true).unwrap();
    s.sim.things.get_mut(fire).unwrap().props[P_FIRE] = 0.5;
    s.sim.things.get_mut(fire).unwrap().props[P_FUEL] = 5.0;
    let all = record(&mut s);
    s.run(3.0, 0.1);
    assert!(s.sim.things.get(fire).unwrap().props[P_FIRE] > 0.0, "burning when we leave");
    // Walk far away and let two days pass.
    s.sim.player.pos = p + Vec3::new(3000.0, 0.0, 0.0);
    s.run(2.0, 0.1);
    let hunger0 = s.sim.cast.get(cid).unwrap().needs.hunger;
    s.sim.t += crate::render::sky::DAY_SECONDS * 2.0;
    s.run(2.0, 0.1);
    assert!(s.sim.things.get(fire).unwrap().props[P_FIRE] > 0.0, "time stood still while far away");
    // Come back.
    s.sim.player.pos = p;
    s.run(3.0, 0.1);
    let caught = of(&all, "caught_up");
    assert!(!caught.is_empty(), "the region ran the missed time");
    let t = s.sim.things.get(fire).unwrap();
    assert_eq!(t.props[P_FIRE], 0.0, "the campfire has burnt out");
    assert!(t.props[P_CHAR] > 0.9, "leaving charcoal");
    assert!(!of(&all, "burnt_out").is_empty());
    let n = s.sim.cast.get(cid).unwrap();
    assert!(n.needs.hunger > hunger0 + 0.3, "time passed for Wren too ({hunger0} → {})", n.needs.hunger);
}

/// The live world survives closing and reopening the file.
#[test]
fn live_things_cells_and_relationships_persist() {
    let w = world("persist", 41);
    let p = dry_spot(&w, 6.0, 0.1);
    let a = add_char(&w, "Pell", "kind", &["Quill: friend"], p);
    let b = add_char(&w, "Quill", "kind", &["Pell: friend"], p + Vec3::new(3.0, 0.0, 0.0));
    let ball = add_type(&w, &fixture("sims/ball.js"));
    let ball_id = place(&w, ball, p + Vec3::new(1.0, 0.0, 0.0), 0.0);
    let (tid, moved_to, aff) = {
        let mut s = session(&w, 11, None);
        let t = s.sim.liven(&Target::Instance(ball_id)).unwrap();
        s.sim.things.get_mut(t).unwrap().pos += Vec3::new(2.0, 0.0, 0.0);
        s.sim.things.get_mut(t).unwrap().props[P_WET] = 0.8;
        s.sim.things.get_mut(t).unwrap().dirty = true;
        s.sim.social.bond(ActorId::Npc(a), ActorId::Npc(b), 0.2, s.sim.t);
        let pos = s.sim.things.get(t).unwrap().pos;
        let aff = s.sim.social.affection(ActorId::Npc(a), ActorId::Npc(b));
        s.save();
        (t, pos, aff)
    };
    let s = session(&w, 11, None);
    let t = s.sim.things.get(tid).expect("the ball is still live");
    assert!((t.pos - moved_to).length() < 0.01);
    assert!((t.props[P_WET] - 0.8).abs() < 0.01);
    assert!(s.sim.cache.overlay.hidden.contains(&ball_id), "the static copy stays hidden");
    assert!((s.sim.social.affection(ActorId::Npc(a), ActorId::Npc(b)) - aff).abs() < 1e-3);
}

/// F2 / pocket inspect show the raw truth.
#[test]
fn inspect_shows_props_state_origin_and_minds() {
    let w = world("inspect", 42);
    let p = dry_spot(&w, 6.0, 0.9);
    let cid = add_char(&w, "Nell", "curious and kind", &[], p);
    let seed = add_type(&w, &fixture("sims/seed.js"));
    let mut s = session(&w, 12, None);
    let id = s.sim.spawn_thing(seed, s.sim.player.pos + Vec3::X, 0.0, 1.0, super::things::Origin { made_by: Some("Nell".into()), ..Default::default() }, true).unwrap();
    s.run(3.0, 0.1);
    let v = s.sim.inspect(&Target::Thing(id));
    assert_eq!(v["type"], "acorn");
    assert!(v["props"]["alive"].as_f64() == Some(1.0));
    assert!(v["state"][0].as_f64().unwrap() > 0.0, "the tick advanced its state: {v}");
    assert_eq!(v["origin"]["made_by"], "Nell");
    assert!(v["behavior"].as_str().unwrap().contains("transform(0)"));
    let lines = s.sim.inspect_lines(&Target::Thing(id));
    assert!(lines.iter().any(|l| l.contains("state")), "{lines:?}");
    let n = s.sim.inspect(&Target::Actor(ActorId::Npc(cid)));
    assert_eq!(n["name"], "Nell");
    assert!(n["needs"]["hunger"].is_number() && n["traits"]["curious"].as_f64().unwrap() > 0.5);
    let me = s.sim.inspect(&Target::Name("Nell".into()));
    assert_eq!(me["name"], "Nell");
}




/// Fire reaches only so far: a strip of bare ground wider than the heat's
/// reach stops it (which is why water does).
#[test]
fn a_firebreak_wider_than_the_heat_reach_stops_fire() {
    let w = world("break", 43);
    let a = dry_spot(&w, 8.0, 0.4);
    let mut dir = Vec3::new(1.0, 0.0, 0.0);
    // Pick the flattest dry direction.
    for d in [Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z] {
        if (0..30).all(|k| w.terrain.height(a.x + d.x * k as f32, a.z + d.z * k as f32) > WATER_LEVEL + 0.4) {
            dir = d;
            break;
        }
    }
    assert!(plant_grass(&w, a, dir, 0.0, 7.5, 5.0) > 8);
    assert!(plant_grass(&w, a, dir, 17.0, 25.0, 5.0) > 8, "beyond a 9.5 m gap");
    let mut s = session(&w, 13, None);
    s.sim.player.pos = a - dir * 20.0;
    // Clear the procedural scatter in the gap and around (only planted grass burns).
    let snap = s.sim.snap.clone();
    for it in s.sim.cache.items_near(&snap, a + dir * 12.0, 40.0) {
        s.sim.cache.overlay.taken.insert(it.cell);
        s.sim.things.taken.insert(it.cell, -1);
    }
    let p = s.sim.snap.instances.iter().min_by(|x, y| (x.pos - a).length().total_cmp(&(y.pos - a).length())).unwrap().id;
    let id = s.sim.promote_instance(p).unwrap();
    s.sim.things.get_mut(id).unwrap().props[P_FIRE] = 0.6;
    s.run(90.0, 0.1);
    let ignited = events(&s.sim, "ignited");
    let reach = ignited.iter().map(|e| (Vec3::from(e.pos.unwrap()) - a).dot(dir)).fold(f32::MIN, f32::max);
    assert!(reach > 6.0, "it burnt across the near patch ({reach:.1} m)");
    assert!(reach < 12.0, "and stopped at the gap ({reach:.1} m)");
}

/// A grass patch on a flat dry spot, with the wild scatter around cleared
/// (only what is planted burns). Returns the spot, the direction and a session.
fn grass_patch(tag: &str, seed: u32, to: f32) -> (W, Vec3, Vec3) {
    let w = world(tag, seed);
    let a = dry_spot(&w, 8.0, 0.4);
    let mut dir = Vec3::X;
    for d in [Vec3::X, Vec3::Z, -Vec3::X, -Vec3::Z] {
        if (0..30).all(|k| w.terrain.height(a.x + d.x * k as f32, a.z + d.z * k as f32) > WATER_LEVEL + 0.4) {
            dir = d;
            break;
        }
    }
    assert!(plant_grass(&w, a, dir, 0.0, to, 5.0) > 8);
    (w, a, dir)
}

fn clear_scatter(s: &mut Session, at: Vec3, r: f32) {
    let snap = s.sim.snap.clone();
    for it in s.sim.cache.items_near(&snap, at, r) {
        s.sim.cache.overlay.taken.insert(it.cell);
        s.sim.things.taken.insert(it.cell, -1);
    }
}

/// A kiln at 900° warms the grass around it and never sets it alight:
/// heat warms, only flames burn.
#[test]
fn a_kiln_warms_the_grass_but_does_not_light_it() {
    let (w, a, dir) = grass_patch("kiln", 44, 10.0);
    let kiln = add_type(&w, r#"// A kiln.
export const meta = { name: "root kiln", bounds: [1.2, 1.6, 1.2], tags: ["building"], props: { heat: 900, conducts: 0.6, light: 0.5 } };
export function sdf(x, y, z, k) { return box(x, y - 0.8, z, 0.6, 0.8, 0.6); }
export function color(x, y, z, k) { return rgb(140, 90, 60); }
"#);
    let kiln_at = ground(&w, a.x + dir.x * 5.0 + 1.2, a.z + dir.z * 5.0 + 1.2);
    let inst = place(&w, kiln, kiln_at, 0.0);
    let mut s = session(&w, 14, None);
    s.sim.player.pos = a - dir * 20.0;
    clear_scatter(&mut s, a + dir * 5.0, 40.0);
    s.sim.promote_instance(inst).unwrap();
    s.run(90.0, 0.1);
    assert!(events(&s.sim, "ignited").is_empty(), "nothing caught: {:?}", events(&s.sim, "ignited").iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    let warm = s.sim.things.live().filter(|t| t.props[P_TEMP] > 25.0 && t.props[P_BURNS] > 0.0).count();
    assert!(warm > 0, "the grass by it is warm");
    assert!(s.sim.incidents.list.is_empty());
}

/// A fire that eats a patch of grass is one incident: it knows what started
/// it, tells the log one line (rewritten as it grows), keeps one event in the
/// save, people remember its cause, and it ends.
#[test]
fn a_fire_is_one_incident_with_one_cause_and_one_line() {
    let (w, a, dir) = grass_patch("incident", 45, 12.0);
    let lantern = add_type(&w, &fixture("sims/lantern.js"));
    let watcher = add_char(&w, "Wenna", "calm, timid", &[], a - dir * 45.0);
    let mut s = session(&w, 15, None);
    s.sim.player.pos = a - dir * 16.0;
    clear_scatter(&mut s, a + dir * 6.0, 40.0);
    // A lantern thrown into the grass by the traveler.
    let id = s.sim.spawn_thing(lantern, a + Vec3::Y * 3.0, 0.0, 1.0, Default::default(), false).unwrap();
    s.sim.things.get_mut(id).unwrap().vel = Vec3::new(0.0, -6.0, 0.0);
    s.sim.things.get_mut(id).unwrap().asleep = false;
    s.sim.things.get_mut(id).unwrap().thrown_by = Some((ActorId::Player, s.sim.t));
    let all = record(&mut s);
    s.run(40.0, 0.1);
    let ignited = of(&all, "ignited");
    assert!(ignited.len() >= 5, "the grass caught: {}", ignited.len());
    assert_eq!(s.sim.incidents.list.len(), 1, "one fire, one incident: {:?}", s.sim.incidents.list.iter().map(|i| i.line()).collect::<Vec<_>>());
    let inc = s.sim.incidents.list[0].clone();
    assert_eq!(inc.count("ignited") as usize, ignited.len(), "every tuft counted on it");
    assert!(ignited.iter().all(|e| e.data["incident"] == inc.id), "every ignition filed under it");
    assert_eq!(inc.cause.name, "the oil lantern", "started by the lantern (a piece of it): {:?}", inc.cause);
    assert_eq!(inc.cause.by.as_deref(), Some("the traveler"), "thrown by the traveler");
    assert!(inc.line().contains("oil lantern (the traveler)"), "{}", inc.line());
    // The log: one line per incident, however many tufts.
    let notes = s.sim.drain_notes();
    let lines: std::collections::BTreeSet<u32> = notes.iter().filter_map(|n| if let super::Note::Incident { id, .. } = n { Some(*id) } else { None }).collect();
    assert_eq!(lines.len(), 1);
    assert!(!notes.iter().any(|n| matches!(n, super::Note::Notable(t) | super::Note::Far { text: t, .. } if t.contains("catches fire"))), "no line per tuft");
    // It burns out and ends; the save keeps its start and end, not every tuft.
    s.run(200.0, 0.1);
    let inc = s.sim.incidents.list[0].clone();
    assert!(inc.ended.is_some(), "it ended: {}", inc.line());
    assert!(inc.line().contains("burnt itself out"), "{}", inc.line());
    // Who saw it remembers what started it.
    let mems = s.sim.db.memories(watcher).unwrap_or_default();
    assert!(mems.iter().any(|m| m.text.contains("oil lantern")), "the watcher knows what started it: {:?}", mems.iter().map(|m| m.text.clone()).collect::<Vec<_>>());
    s.save();
    let saved: i64 = s.sim.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM events WHERE kind = 'ignited'", [], |r| r.get(0))?)).unwrap();
    assert_eq!(saved, 1, "only the first ignition is saved");
    let ends: i64 = s.sim.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM events WHERE kind IN ('incident', 'incident_end')", [], |r| r.get(0))?)).unwrap();
    assert_eq!(ends, 2);
    sound(&s);
}

/// People whose homes a fire comes near fight it: the brave first, and
/// those who see a neighbour at it join in. They beat it out before it eats
/// the whole meadow.
#[test]
fn villagers_fight_a_fire_near_their_homes() {
    let (w, a, dir) = grass_patch("fight", 46, 22.5);
    let home = a + dir * 26.0;
    let names = ["Tarn", "Ilse", "Bram"];
    for n in names {
        add_char(&w, n, "brave, steady, a good neighbour", &["Tarn: neighbour", "Ilse: neighbour", "Bram: neighbour"], home);
    }
    let mut s = session(&w, 16, None);
    s.sim.player.pos = a - dir * 20.0;
    clear_scatter(&mut s, a + dir * 10.0, 50.0);
    let p = s.sim.snap.instances.iter().min_by(|x, y| (x.pos - a).length().total_cmp(&(y.pos - a).length())).unwrap().id;
    let id = s.sim.promote_instance(p).unwrap();
    s.sim.things.get_mut(id).unwrap().props[P_FIRE] = 0.6;
    let all = record(&mut s);
    s.run(150.0, 0.1);
    // Nobody was told how: they worked out by the rules that beating at it with their hands pushes it back.
    let fought: Vec<SimEvent> = of(&all, "applied").into_iter().filter(|e| e.data["incidents"].as_array().is_some_and(|a| !a.is_empty())).collect();
    assert!(!fought.is_empty(), "someone fought it: {:?}", of(&all, "applied").iter().take(3).map(|e| e.text.clone()).collect::<Vec<_>>());
    let inc = s.sim.incidents.list.first().cloned().expect("an incident");
    assert!(!inc.fought_by.is_empty(), "{}", inc.line());
    assert!(inc.ended.is_some(), "it's out: {}", inc.line());
    assert!(inc.saved.len() >= 5, "they saved some of the meadow: {}", inc.line());
    assert!(s.sim.trouble_line(home).is_some_and(|l| l.contains("Fought by")), "{:?}", s.sim.trouble_line(home));
    eprintln!("{}", inc.line());
    sound(&s);
}

/// The same machinery for a force the engine has never heard of: a world's
/// own curse spreads by touch and is told as one incident; a brave villager
/// works out, by imagining each tool against it, that the holy charm lying
/// about lifts it (bare hands do nothing), fetches it and lifts the curse.
#[test]
fn a_villager_lifts_a_spreading_curse_with_a_charm_nobody_told_them_about() {
    use super::props::{Crossing, PropMeta, Words};
    use super::rules::RuleSpec;
    let w = world("curse-fight", 47);
    let props = vec![("cursed".to_string(), 0.0, "how cursed it is, 0..1".to_string()), ("holy".to_string(), 0.0, "how holy it is, 0..1".to_string())];
    let rules = vec![
        RuleSpec { name: "curses spread by touch".into(), near: Some(1.5), when: "self.cursed > 0.5 && other.cursed < self.cursed".into(), effects: vec!["other.cursed += 0.1 * dt".into()] },
        RuleSpec { name: "holiness lifts curses".into(), near: Some(0.5), when: "self.cursed > 0 && other.holy > 0.5 && other.force > 0".into(), effects: vec!["self.cursed -= 1 * dt".into()] },
    ];
    let (props, rules) = crate::brain::check_universe_rules(&props, &rules);
    assert_eq!(rules.len(), 2);
    super::persist::set_universe_rules(&w.db, &props, &rules).unwrap();
    let curse = PropMeta {
        rises: vec![Crossing { kind: "cursed_rose".into(), text: "fell under the curse".into(), when: None, saved: false }],
        falls: vec![Crossing { kind: "cursed_fell".into(), text: "was freed of the curse".into(), when: None, saved: true }],
        incident: Some(Words { noun: "curse".into(), big: "blight".into(), active: "cursed".into(), spent: "withered".into(), ended: "faded away".into(), stopped: "lifted".into() }),
        hazard: 0.4,
        keep: true,
        range: Some([0.0, 1.0]),
        ..Default::default()
    };
    super::persist::set_universe_meta(&w.db, &[("cursed".into(), curse)]).unwrap();
    let idol = add_type(&w, r#"
export const meta = { name: "black idol", bounds: [0.2, 0.3, 0.2], tags: ["item"], props: { cursed: 1, mass: 30 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.15, 0.25, 0.15, 0.03); }
export function color(x, y, z, k) { return rgb(30, 20, 40); }
"#);
    let charm = add_type(&w, r#"
export const meta = { name: "sun charm", bounds: [0.06, 0.06, 0.02], tags: ["item"], props: { holy: 1, mass: 0.2 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.05, 0.05, 0.01, 0.01); }
export function color(x, y, z, k) { return rgb(230, 200, 90); }
"#);
    let stick = builtin_id(&w, "stick");
    let p = dry_spot(&w, 8.0, 0.9);
    let hero = add_char(&w, "Oda", "brave, devout, steady", &[], p + Vec3::new(6.0, 0.0, 0.0));
    let mut s = session(&w, 17, None);
    s.sim.player.pos = p + Vec3::new(-25.0, 0.0, 0.0);
    clear_scatter(&mut s, p, 30.0);
    s.sim.spawn_thing(idol, p, 0.0, 1.0, Default::default(), true).unwrap();
    for k in 0..6 {
        let a = k as f32 * 1.05;
        s.sim.spawn_thing(stick, p + Vec3::new(a.cos(), 0.0, a.sin()) * 1.0, a, 1.0, Default::default(), true).unwrap();
    }
    let charm_id = s.sim.spawn_thing(charm, p + Vec3::new(9.0, 0.0, 2.0), 0.0, 1.0, Default::default(), true).unwrap();
    let all = record(&mut s);
    s.run(90.0, 0.1);
    let inc = s.sim.incidents.list.first().cloned().unwrap_or_else(|| panic!("an incident: {:?}", of(&all, "cursed_rose").len()));
    assert_eq!(inc.kind, "curse");
    assert!(inc.cause.name.contains("black idol"), "{:?}", inc.cause);
    let used: Vec<SimEvent> = of(&all, "applied");
    assert!(used.iter().any(|e| e.data["with"] == charm_id), "Oda worked the charm against it: {:?}", used.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    assert!(inc.fought_by.contains("Oda"), "{}", inc.line());
    assert!(!inc.saved.is_empty(), "lifted from some: {}", inc.line());
    let _ = hero;
    eprintln!("{}", inc.line());
    sound(&s);
}

/// A ball thrown at a hoop from a few metres drops through it.
#[test]
fn a_ball_thrown_at_a_hoop_goes_through() {
    let w = world("swish", 44);
    let hoop = add_type(&w, &fixture("sims/hoop.js"));
    let ball = add_type(&w, &fixture("sims/ball.js"));
    let s = session(&w, 14, None);
    let me = s.sim.player.pos;
    let court = ground(&w, me.x, me.z + 5.0);
    drop(s);
    let hoop_id = place(&w, hoop, court, 0.0);
    let mut s = session(&w, 14, None);
    let id = s.sim.spawn_thing(ball, s.sim.player.pos + Vec3::new(0.3, 0.0, 0.3), 0.0, 1.0, Default::default(), true).unwrap();
    let all = record(&mut s);
    act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Thing(id) }, 0.2);
    let r = act_once(&mut s, ActorId::Player, Action::Throw { at: Some(Target::Instance(hoop_id)), dir: None, force: None }, 5.0);
    assert_eq!(r["ok"], true, "{r}");
    let trace: Vec<String> = all.lock().iter().map(|e| e.text.clone()).collect();
    assert!(!of(&all, "through").is_empty(), "swish: {trace:?}");
}


/// Render a scene with fire, lamps and posed people to a PNG (look at it).
#[test]
#[ignore]
fn render_live_scene_png() {
    let out = std::env::var("POCKET_PNG_OUT").unwrap_or_else(|_| std::env::temp_dir().join("pocket-live.png").display().to_string());
    let w = world_with("png", 44, marsh());
    let hut = add_type(&w, &fixture("sims/hut.js"));
    let lantern = add_type(&w, &fixture("sims/lantern.js"));
    let ball = add_type(&w, &fixture("sims/ball.js"));
    let me = w.spawn;
    let a = ground(&w, me.x + 2.0, me.z + 9.0);
    let hut_id = place(&w, hut, ground(&w, me.x - 9.0, me.z + 16.0), 0.4);
    let c1 = add_char(&w, "Ilse", "warm", &["Ola: daughter"], ground(&w, me.x - 1.5, me.z + 6.0));
    let c2 = add_char(&w, "Ola", "lively", &["Ilse: mother"], ground(&w, me.x - 1.0, me.z + 6.45));
    let c3 = add_char(&w, "Bo", "playful", &[], ground(&w, me.x + 3.5, me.z + 7.0));
    let mut s = session(&w, 15, None);
    for n in s.sim.cast.npcs.iter_mut() {
        n.think_at = f64::MAX;
    }
    // Hug, a raised arm, a lamp, a fire, a ball in the air.
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.8;
    let (i, o) = (ActorId::Npc(c1), ActorId::Npc(c2));
    s.sim.cast.get_mut(c1).unwrap().a.yaw = (s.sim.cast.get(c2).unwrap().a.pos - s.sim.cast.get(c1).unwrap().a.pos).x.atan2((s.sim.cast.get(c2).unwrap().a.pos - s.sim.cast.get(c1).unwrap().a.pos).z);
    s.sim.cast.get_mut(c2).unwrap().a.yaw = s.sim.cast.get(c1).unwrap().a.yaw + std::f32::consts::PI;
    let _ = s.sim.act(i, Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(o)) });
    let _ = s.sim.act(ActorId::Npc(c3), Action::Gesture { kind: "cheer".into(), to: None });
    let l = s.sim.spawn_thing(lantern, ground(&w, me.x + 0.8, me.z + 4.0), 0.0, 1.0, Default::default(), true).unwrap();
    let _ = l;
    let snap = s.sim.snap.clone();
    let mut lit = 0;
    for it in s.sim.cache.items_near(&snap, a, 5.0) {
        if lit < 6 {
            if let Some(id) = s.sim.promote_cell(it.cell) {
                s.sim.things.get_mut(id).unwrap().props[P_FIRE] = 0.8;
                lit += 1;
            }
        }
    }
    let h = s.sim.promote_instance(hut_id).unwrap();
    s.sim.things.get_mut(h).unwrap().props[P_CHAR] = 0.7;
    let b = s.sim.spawn_thing(ball, ground(&w, me.x + 1.5, me.z + 5.0) + Vec3::Y * 1.8, 0.0, 1.0, Default::default(), false).unwrap();
    let _ = b;
    for _ in 0..12 {
        s.sim.step(0.1);
    }
    s.sim.things.get_mut(b).unwrap().pos = ground(&w, me.x + 1.5, me.z + 5.0) + Vec3::Y * 1.8;
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    s.sim.flip(snap.clone());
    let cam = crate::render::Camera { pos: me + Vec3::Y * 1.65, yaw: 0.0, pitch: -0.08, fov_y: 1.05 };
    let (pw, ph) = (320u32, 180u32);
    let drawn = s.sim.draw(cam.pos, crate::render::VIEW_DIST);
    let culled = crate::world::cull::cull(&snap, &mut s.sim.cache, &cam, pw as f32 / ph as f32, &drawn.insts, &Default::default());
    let light = crate::render::sky::lighting(s.sim.t, &snap.look.palette);
    let sp = crate::render::SceneParams { terrain: &snap.terrain, palette: &snap.look.palette, camera: cam, width: pw, height: ph, pixel_aspect: 1.0, light, time: 1.0, frame: 0, shadows: true, lights: &drawn.lights };
    let globals = crate::render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
    let req = crate::render::FrameRequest { id: 1, width: pw, height: ph, globals, instances: culled.insts, grid: culled.grid, scene: snap.scene.clone(), terrain: snap.terrain.clone(), look: snap.look.clone() };
    let mut handle = match &gpu {
        Some(g) => crate::render::gpu::spawn(g.clone()),
        None => crate::render::cpu::spawn(),
    };
    handle.tx.send(crate::render::RenderMsg::Frame(Box::new(req))).unwrap();
    let f = handle.rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
    handle.shutdown();
    let sc = 3u32;
    let mut rgb = Vec::new();
    for y in 0..f.height * sc {
        for x in 0..f.width * sc {
            rgb.extend_from_slice(&crate::render::unpack(f.pixels[((y / sc) * f.width + x / sc) as usize]));
        }
    }
    std::fs::write(&out, crate::png::encode(f.width * sc, f.height * sc, &rgb)).unwrap();
    eprintln!("wrote {out}: {} lights, {} instances", drawn.lights.len(), drawn.insts.len());
}

/// Render people holding and carrying things by day (look at it).
#[test]
#[ignore]
fn render_day_scene_png() {
    let out = std::env::var("POCKET_PNG_OUT").unwrap_or_else(|_| std::env::temp_dir().join("pocket-day.png").display().to_string());
    let w = world_with("pngday", 44, marsh());
    let log = add_type(&w, &fixture("sims/log.js"));
    let ball = add_type(&w, &fixture("sims/ball.js"));
    let lantern = add_type(&w, &fixture("sims/lantern.js"));
    let me = w.spawn;
    let c1 = add_char(&w, "Bo", "kind", &["Rune: friend"], ground(&w, me.x - 1.0, me.z + 7.0));
    let c2 = add_char(&w, "Rune", "kind", &["Bo: friend"], ground(&w, me.x + 1.6, me.z + 7.2));
    let c3 = add_char(&w, "Ola", "lively", &[], ground(&w, me.x + 3.5, me.z + 5.0));
    let c4 = add_char(&w, "Ilse", "warm", &[], ground(&w, me.x - 3.5, me.z + 5.5));
    let mut s = session(&w, 16, None);
    for n in s.sim.cast.npcs.iter_mut() {
        n.think_at = f64::MAX;
        n.a.yaw = std::f32::consts::PI;
    }
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.45;
    let l = s.sim.spawn_thing(log, ground(&w, me.x + 0.3, me.z + 7.6), 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.things.get_mut(l).unwrap().holder = Some(ActorId::Npc(c1));
    s.sim.things.get_mut(l).unwrap().co_holder = Some(ActorId::Npc(c2));
    s.sim.cast.get_mut(c1).unwrap().a.held = Some(l);
    s.sim.cast.get_mut(c2).unwrap().a.held = Some(l);
    let b = s.sim.spawn_thing(ball, ground(&w, me.x + 3.5, me.z + 5.0), 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.hand_to(ActorId::Npc(c3), b);
    // And one in our own hand.
    let mine = s.sim.spawn_thing(ball, me, 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.hand_to(ActorId::Player, mine);
    let lt = s.sim.spawn_thing(lantern, ground(&w, me.x - 3.5, me.z + 5.5), 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.hand_to(ActorId::Npc(c4), lt);
    let _ = s.sim.act(ActorId::Npc(c4), Action::Gesture { kind: "wave".into(), to: None });
    for _ in 0..10 {
        s.sim.step(0.1);
    }
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    s.sim.flip(snap.clone());
    let cam = crate::render::Camera { pos: s.sim.player.pos + Vec3::Y * 1.65, yaw: 0.0, pitch: -0.12, fov_y: 1.05 };
    let (pw, ph) = (320u32, 180u32);
    let drawn = s.sim.draw_first_person(&cam, crate::render::VIEW_DIST);
    let culled = crate::world::cull::cull(&snap, &mut s.sim.cache, &cam, pw as f32 / ph as f32, &drawn.insts, &Default::default());
    let light = crate::render::sky::lighting(s.sim.t, &snap.look.palette);
    let sp = crate::render::SceneParams { terrain: &snap.terrain, palette: &snap.look.palette, camera: cam, width: pw, height: ph, pixel_aspect: 1.0, light, time: 1.0, frame: 0, shadows: true, lights: &drawn.lights };
    let globals = crate::render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
    let req = crate::render::FrameRequest { id: 1, width: pw, height: ph, globals, instances: culled.insts, grid: culled.grid, scene: snap.scene.clone(), terrain: snap.terrain.clone(), look: snap.look.clone() };
    let mut handle = match &gpu {
        Some(g) => crate::render::gpu::spawn(g.clone()),
        None => crate::render::cpu::spawn(),
    };
    handle.tx.send(crate::render::RenderMsg::Frame(Box::new(req))).unwrap();
    let f = handle.rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
    handle.shutdown();
    let sc = 3u32;
    let mut rgb = Vec::new();
    for y in 0..f.height * sc {
        for x in 0..f.width * sc {
            rgb.extend_from_slice(&crate::render::unpack(f.pixels[((y / sc) * f.width + x / sc) as usize]));
        }
    }
    std::fs::write(&out, crate::png::encode(f.width * sc, f.height * sc, &rgb)).unwrap();
}

/// Through the brain with a scripted LLM: genesis brings the universe's own
/// properties and rules, characters overhear each other, and spawn() of a
/// name nobody has written yet gets it written.
#[test]
fn brain_round_trip_rules_chat_and_new_types() {
    let w = world("brain", 45);
    let pine = fixture("mock/seapine.js");
    let honey = r#"
export const meta = { name: "honeycomb", bounds: [0.1, 0.05, 0.1], tags: ["item", "food"], props: { edible: 0.5, mass: 0.2 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.08, 0.03, 0.08, 0.01); }
export function color(x, y, z, k) { return rgb(230, 170, 40); }
"#
    .to_string();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if user.contains("Design the base layer") {
            return format!(
                "```json\n{{\"name\": \"Hexmoor\", \"biomes\": [], \"properties\": [{{\"name\": \"cursed\", \"default\": 0, \"meaning\": \"how cursed it is\"}}], \"rules\": [{{\"name\": \"curses spread by touch\", \"near\": 1.5, \"when\": \"self.cursed > 0.5 && other.cursed < self.cursed\", \"do\": [\"other.cursed += 0.05 * dt\"]}}, {{\"name\": \"plague\", \"near\": 10, \"when\": \"other.cursed < 1\", \"do\": [\"other.cursed = 1\"]}}]}}\n```\n```js\n{pine}\n```"
            );
        }
        if sys.contains("meet briefly") || sys.contains("talk briefly") {
            return r#"{"lines": [{"who": "Ada", "text": "Did you hear the bees?"}, {"who": "Ben", "text": "All morning, Ada."}]}"#.into();
        }
        if user.contains("Object type to write: \"honeycomb\"") {
            return format!("```js\n{honey}\n```");
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let a = dry_spot(&w, 6.0, 0.2);
    let ca = add_char(&w, "Ada", "kind", &["Ben: brother"], a);
    let cb = add_char(&w, "Ben", "kind", &["Ada: sister"], a + Vec3::new(2.0, 0.0, 0.0));
    let hive = add_type(&w, r#"
export const meta = { name: "beehive", bounds: [0.4, 0.6, 0.4], tags: ["item"], spawns: ["honeycomb"] };
export function sdf(x, y, z, k) { return roundBox(x, y - 0.3, z, 0.35, 0.3, 0.35, 0.05); }
export function color(x, y, z, k) { return rgb(200, 160, 80); }
export function tick(s, w, k) { s.s0 = s.s0 + w.dt; if (s.s0 > 3) { s.s0 = 0; spawn(0); } }
"#);
    let mut s = session(&w, 17, Some(llm));
    s.brain.send(crate::brain::Cmd::Genesis);
    for _ in 0..600 {
        s.step(0.05);
        if s.sim.vocab.id("cursed").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(s.sim.vocab.id("cursed").is_some(), "the universe's own property arrived");
    assert!(s.sim.rules.iter().any(|r| r.spec.name == "curses spread by touch"));
    assert!(!s.sim.rules.iter().any(|r| r.spec.name == "plague"), "the explosive rule was dropped");
    // Overheard talk.
    let all = record(&mut s);
    s.sim.player.pos = a + Vec3::new(0.0, 0.0, 6.0);
    s.sim.social.want_chat(ca, cb, s.sim.t);
    s.run(10.0, 0.1);
    let said: Vec<String> = of(&all, "said").into_iter().map(|e| e.text).collect();
    assert!(said.iter().any(|t| t.contains("Did you hear the bees?")), "{said:?}");
    // spawn("honeycomb"): nobody wrote it yet, so the builder does.
    let h = s.sim.spawn_thing(hive, a + Vec3::new(1.0, 0.0, 3.0), 0.0, 1.0, Default::default(), true).unwrap();
    let _ = h;
    for _ in 0..200 {
        s.step(0.1);
        if s.sim.things.live().any(|t| s.sim.snap.type_of(t.type_id).is_some_and(|ty| ty.name() == "honeycomb")) {
            break;
        }
    }
    assert!(s.sim.things.live().any(|t| s.sim.snap.type_of(t.type_id).is_some_and(|ty| ty.name() == "honeycomb")), "the hive made honeycomb: {:?}", s.sim.log.recent.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
}

/// Plans from the LLM are lenient about shape.
#[test]
fn plan_steps_accept_common_shapes() {
    use super::npc::parse_step;
    use serde_json::json;
    let cases = [
        (json!({"do": "pick up", "target": "ball"}), Action::Hold { target: Target::Name("ball".into()) }),
        (json!({"action": "walk to", "target": "Ola"}), Action::Goto { target: Target::Name("Ola".into()), run: false }),
        (json!({"do": "hug", "target": "Ola"}), Action::Gesture { kind: "hug".into(), to: Some(Target::Name("Ola".into())) }),
        (json!({"do": "say", "line": "Hello"}), Action::Say { text: "Hello".into(), to: None }),
        (json!({"do": "make", "what": "a kite"}), Action::Do { text: "make a kite".into(), on: None, at: None }),
        (json!({"do": "create", "text": "a wooden ball"}), Action::Do { text: "make a wooden ball".into(), on: None, at: None }),
        (json!({"do": "throw", "at": "hoop"}), Action::Throw { at: Some(Target::Name("hoop".into())), dir: None, force: None }),
        (json!({"do": "give", "to": "Ola"}), Action::Give { to: Target::Name("Ola".into()) }),
        (json!({"do": "wait", "secs": 3}), Action::Wait { secs: 3.0 }),
    ];
    for (v, want) in cases {
        assert_eq!(parse_step(&v), Some(want), "{v}");
    }
    assert_eq!(parse_step(&json!({"do": "teleport"})), None);
}

/// What is eaten stays gone (across sessions) until it grows back a day later.
#[test]
fn eaten_plants_stay_gone_then_grow_back() {
    let w = world("regrow", 46);
    let mut s = session(&w, 18, None);
    let snap = s.sim.snap.clone();
    let p = s.sim.player.pos;
    let it = s.sim.cache.items_near(&snap, p, 60.0).into_iter().find(|it| snap.type_of(it.inst.info[0]).is_some_and(|t| t.has_tag("grass"))).expect("grass nearby");
    let id = s.sim.promote_cell(it.cell).unwrap();
    s.sim.things.remove(id);
    s.sim.sync_overlay();
    assert!(!s.sim.cache.overlay.shows(it.cell));
    assert_eq!(s.sim.promote_cell(it.cell), None, "nothing there now");
    s.save();
    drop(s);
    let mut s = session(&w, 18, None);
    assert!(!s.sim.cache.overlay.shows(it.cell), "still gone after reopening");
    s.sim.t += crate::render::sky::DAY_SECONDS * 1.1;
    s.run(1.5, 0.1);
    assert!(s.sim.cache.overlay.shows(it.cell), "grown back");
}

/// A gesture nobody knew: the LLM writes its key poses once; it is kept and
/// anyone can do it after.
#[test]
fn an_unknown_gesture_is_learned_and_kept() {
    let w = world("gesture", 47);
    let calls = Arc::new(Mutex::new(0usize));
    let c2 = calls.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You animate a simple body") {
            *c2.lock() += 1;
            assert!(user.contains("salute"));
            return r#"{"duration": 2, "frames": [{"t": 0, "pose": {}}, {"t": 0.3, "pose": {"r_raise": 0.75, "r_fwd": 0.2, "nod": -0.1}}, {"t": 0.8, "pose": {"r_raise": 0.75, "r_fwd": 0.2}}, {"t": 1, "pose": {}}]}"#.into();
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let cid = add_char(&w, "Wim", "proper", &[], dry_spot(&w, 5.0, 0.5));
    let mut s = session(&w, 19, Some(llm.clone()));
    let r = act_once(&mut s, ActorId::Player, Action::Gesture { kind: "salute".into(), to: None }, 0.6);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(*calls.lock(), 1);
    assert!(s.sim.player.pose[super::actor::R_RAISE] > 0.3, "the arm comes up: {:?} {:?} {:?}", s.sim.player.pose, s.sim.player.gesture, s.sim.log.recent.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    // Someone else can do it too, without asking again.
    let r = act_once(&mut s, ActorId::Npc(cid), Action::Gesture { kind: "salute".into(), to: Some(Target::Actor(ActorId::Player)) }, 0.6);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(*calls.lock(), 1);
    drop(s);
    // Kept with the world.
    let s = session(&w, 19, Some(llm));
    let _ = s;
    assert!(super::actor::GestureKind::parse("salute").is_some());
    let n: i64 = w.db.with(|c| Ok(c.query_row("SELECT COUNT(*) FROM gestures", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 1);
}

// ------------------------------------------------------------------ editing things

/// The distance to a live thing's shape at `p`, as physics and picking see it.
fn thing_sdf(s: &Session, id: super::things::ThingId, p: Vec3) -> f32 {
    let t = s.sim.things.get(id).unwrap();
    let ty = s.sim.snap.type_of(t.type_id).unwrap();
    super::render::thing_inst(t, ty, [0.0; 4]).sdf(&ty.ct, p)
}

/// Cutting takes a piece out of the shape where it was touched: what was
/// solid is now air, for physics and picking alike; the rest stays.
#[test]
fn a_cut_takes_a_piece_out_where_it_was_touched() {
    let w = world("cut", 52);
    let hut = add_type(&w, &fixture("sims/hut.js"));
    let at = dry_spot(&w, 9.0, 0.3);
    let inst = place(&w, hut, at, 0.0);
    let mut s = session(&w, 22, None);
    let h = s.sim.liven(&Target::Instance(inst)).unwrap();
    let wall = s.sim.touch_point(h, s.sim.things.get(h).unwrap().pos + Vec3::new(6.0, 1.2, 0.0)).unwrap();
    let inside = wall - Vec3::X * 0.2;
    let elsewhere = wall + Vec3::new(-0.2, 0.0, 1.2);
    assert!(thing_sdf(&s, h, inside) < 0.0, "solid wall before");
    s.sim.cut(ActorId::Player, h, Some(wall), 0.4, false).unwrap();
    assert!(thing_sdf(&s, h, inside) > 0.0, "a hole after");
    assert!(thing_sdf(&s, h, elsewhere) < 0.0, "the rest of the wall stays");
    assert_eq!(s.sim.things.get(h).unwrap().shape.cuts.len(), 1);
    // More cuts than a thing can show merge, and nothing already cut comes back.
    for i in 0..6 {
        let p = wall + Vec3::new(0.0, 0.0, -1.0 + i as f32 * 0.4);
        s.sim.cut(ActorId::Player, h, Some(p), 0.2, i % 2 == 0).unwrap();
    }
    assert_eq!(s.sim.things.get(h).unwrap().shape.cuts.len(), crate::render::MAX_CUTS);
    assert!(thing_sdf(&s, h, inside) > 0.0, "the first hole is still there");
    sound(&s);
}

fn edit_script(calls: Arc<Mutex<Vec<String>>>, hut_src: String) -> Arc<Llm> {
    let w = world("edit-llm", 1);
    Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("physics and common sense") {
            calls.lock().push(user.to_string());
            if user.contains("add the stick") {
                return r#"{"narration": "You nail the stick to the wall.", "reshape": [{"target": "target", "name": "wooden hut with a stick on the wall", "change": "a stick nailed flat across the front wall at the touched spot", "with": "held"}]}"#.into();
            }
            if user.contains("chimney") {
                return r#"{"narration": "The hut grows a chimney.", "reshape": [{"target": "target", "name": "wooden hut with a chimney", "change": "add a stone chimney on the roof"}]}"#.into();
            }
            return r#"{"narration": "Your fist goes through the planks.", "cut": [{"target": "target", "size_m": 0.4, "shape": "round"}], "cache": true}"#.into();
        }
        if user.contains("Change this existing object") && user.contains("Work this other thing into it") {
            calls.lock().push("combine".into());
            assert!(user.contains("a stick") && user.contains("rgb(92, 66, 44)"), "the stick's own code is passed on: {user}");
            let code = hut_src.replace("return min(walls, roof);", "return min(min(walls, roof), box(x, y - 1.6, z + 2.05, 0.5, 0.08, 0.06));");
            return format!("```js\n{code}\n```");
        }
        if user.contains("Change this existing object") {
            calls.lock().push("edit".into());
            assert!(user.contains("wooden hut") && user.contains("chimney") && user.contains("local_point"), "{user}");
            assert!(user.contains("already cut out"), "the cuts are passed on: {user}");
            let code = hut_src.replace("const roof =", "const chimney = box(x - 1.0, y - 3.4, z, 0.25, 0.6, 0.25);\n  const roof =").replace("return min(walls, roof);", "return min(min(walls, roof), chimney);");
            return format!("```js\n{code}\n```");
        }
        r#"{"goal": "", "steps": []}"#.into()
    }))
}

/// "Punch a hole here": the interpreter answers with a cut, placed where the
/// player pointed; the same deed on another hut is answered from the cache.
/// "Add a chimney" rewrites this hut's code (its holes baked in); "add the
/// stick to this wall" works the held stick's code into the other hut's.
#[test]
fn the_interpreter_cuts_and_reshapes_at_the_spot_touched() {
    let w = world("edit", 54);
    let src = fixture("sims/hut.js");
    let hut = add_type(&w, &src);
    let a = dry_spot(&w, 9.0, 0.3);
    let b = dry_spot(&w, 9.0, 3.3);
    let ia = place(&w, hut, a, 0.0);
    let ib = place(&w, hut, b, 0.0);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut s = session(&w, 24, Some(edit_script(calls.clone(), src)));
    for (inst, at) in [(ia, a), (ib, b)] {
        let h = s.sim.liven(&Target::Instance(inst)).unwrap();
        let hp = s.sim.things.get(h).unwrap().pos;
        s.sim.player.pos = Vec3::new(hp.x + 3.6, w.terrain.height(hp.x + 3.6, hp.z), hp.z);
        let wall = s.sim.touch_point(h, hp + Vec3::new(6.0, 1.2, 0.0)).unwrap();
        let r = act_once(&mut s, ActorId::Player, Action::Do { text: "punch a hole in it".into(), on: Some(Target::Thing(h)), at: Some(wall.to_array()) }, 0.5);
        assert_eq!(r["ok"], true, "{r}");
        assert!(thing_sdf(&s, h, wall - Vec3::X * 0.2) > 0.0, "a hole where the fist went ({at:?})");
    }
    assert_eq!(calls.lock().len(), 1, "the second hut was answered from the cache");
    assert!(calls.lock()[0].contains("touched_at"), "the interpreter is told where");
    let h = s.sim.things.by_instance[&ia];
    let hp = s.sim.things.get(h).unwrap().pos;
    s.sim.player.pos = Vec3::new(hp.x + 3.6, w.terrain.height(hp.x + 3.6, hp.z), hp.z);
    let r = act_once(&mut s, ActorId::Player, Action::Do { text: "add a chimney".into(), on: Some(Target::Thing(h)), at: None }, 0.5);
    assert_eq!(r["ok"], true, "{r}");
    s.pump(std::time::Duration::from_secs(60));
    s.run(0.5, 0.05);
    // The deed's story is told once, when the new shape is there.
    let mut heard: Vec<String> = r["heard"].as_array().unwrap().iter().filter_map(|v| v.as_str().map(String::from)).collect();
    heard.extend(s.sim.drain_notes().into_iter().filter(|n| !matches!(n, super::Note::Far { .. } | super::Note::Incident { near: false, .. })).map(|n| n.text()));
    assert_eq!(heard.iter().filter(|t| t.contains("grows a chimney")).count(), 1, "{heard:?}");
    assert!(!heard.iter().any(|t| t.contains("becomes")), "and not told twice: {heard:?}");
    let t = s.sim.things.get(h).unwrap();
    assert_eq!(s.sim.thing_name(h), "wooden hut with a chimney", "calls: {:?}", calls.lock());
    assert!(t.shape.cuts.is_empty(), "the hole is baked into the new code");
    assert!(t.shape.edits.iter().any(|e| e.what.contains("reshaped")));
    assert!((t.pos - hp).length() < 0.5, "it stays where it was");
    let other = s.sim.things.by_instance[&ib];
    assert_eq!(s.sim.thing_name(other), "wooden hut", "only this hut changed");
    // "Add the stick to this wall", stick in hand: the stick's code is worked
    // into the hut's, and the stick is used up once the new hut is there.
    let stick = s.sim.spawn_thing(builtin_id(&w, "stick"), s.sim.player.pos + Vec3::new(0.0, 0.0, 0.4), 0.0, 1.0, Default::default(), true).unwrap();
    let r = act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Thing(stick) }, 0.2);
    assert_eq!(r["ok"], true, "{r}");
    let ob = s.sim.things.get(other).unwrap().pos;
    s.sim.player.pos = Vec3::new(ob.x + 3.6, w.terrain.height(ob.x + 3.6, ob.z), ob.z);
    let wall = s.sim.touch_point(other, ob + Vec3::new(6.0, 1.6, 0.0)).unwrap();
    let r = act_once(&mut s, ActorId::Player, Action::Do { text: "add the stick to this wall".into(), on: Some(Target::Thing(other)), at: Some(wall.to_array()) }, 0.5);
    assert_eq!(r["ok"], true, "{r}");
    s.pump(std::time::Duration::from_secs(60));
    s.run(0.5, 0.05);
    assert_eq!(s.sim.thing_name(other), "wooden hut with a stick on the wall", "calls: {:?}", calls.lock());
    assert!(s.sim.things.get(stick).is_none(), "the stick became part of the hut");
    assert_eq!(s.sim.player.held, None);
    sound(&s);
}

/// Render a hut with a round hole and a square one cut in its wall (look at it).
#[test]
#[ignore]
fn render_edited_hut_png() {
    let out = std::env::var("POCKET_PNG_OUT").unwrap_or_else(|_| std::env::temp_dir().join("pocket-edited.png").display().to_string());
    let w = world("pngedit", 44);
    let hut = add_type(&w, &fixture("sims/hut.js"));
    let me = w.spawn;
    let hut_at = ground(&w, me.x, me.z + 9.0);
    let hut_id = place(&w, hut, hut_at, 0.0);
    let mut s = session(&w, 15, None);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.45;
    let h = s.sim.promote_instance(hut_id).unwrap();
    let hp = s.sim.things.get(h).unwrap().pos;
    // Straight at the front wall, as a player looking at it would.
    let front = |s: &Session, dx: f32, y: f32| {
        let mut p = hp + Vec3::new(dx, y, -8.0);
        for _ in 0..200 {
            let d = thing_sdf(s, h, p);
            if d < 0.005 {
                break;
            }
            p.z += d.max(0.005);
        }
        p
    };
    let (hole, low) = (front(&s, -0.5, 1.3), front(&s, 0.9, 0.5));
    s.sim.cut(ActorId::Player, h, Some(hole), 0.7, false).unwrap();
    s.sim.cut(ActorId::Player, h, Some(low), 0.35, true).unwrap();
    s.sim.step(0.1);
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    s.sim.flip(snap.clone());
    let cam = crate::render::Camera { pos: Vec3::new(hp.x, w.terrain.height(hp.x, hp.z - 6.5) + 1.7, hp.z - 6.5), yaw: 0.0, pitch: -0.05, fov_y: 1.05 };
    let (pw, ph) = (320u32, 180u32);
    let drawn = s.sim.draw(cam.pos, crate::render::VIEW_DIST);
    let culled = crate::world::cull::cull(&snap, &mut s.sim.cache, &cam, pw as f32 / ph as f32, &drawn.insts, &Default::default());
    let light = crate::render::sky::lighting(s.sim.t, &snap.look.palette);
    let sp = crate::render::SceneParams { terrain: &snap.terrain, palette: &snap.look.palette, camera: cam, width: pw, height: ph, pixel_aspect: 1.0, light, time: 1.0, frame: 0, shadows: true, lights: &drawn.lights };
    let globals = crate::render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
    let req = crate::render::FrameRequest { id: 1, width: pw, height: ph, globals, instances: culled.insts, grid: culled.grid, scene: snap.scene.clone(), terrain: snap.terrain.clone(), look: snap.look.clone() };
    let mut handle = match &gpu {
        Some(g) => crate::render::gpu::spawn(g.clone()),
        None => crate::render::cpu::spawn(),
    };
    handle.tx.send(crate::render::RenderMsg::Frame(Box::new(req))).unwrap();
    let f = handle.rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
    handle.shutdown();
    let sc = 3u32;
    let mut rgb = Vec::new();
    for y in 0..f.height * sc {
        for x in 0..f.width * sc {
            rgb.extend_from_slice(&crate::render::unpack(f.pixels[((y / sc) * f.width + x / sc) as usize]));
        }
    }
    std::fs::write(&out, crate::png::encode(f.width * sc, f.height * sc, &rgb)).unwrap();
    eprintln!("wrote {out}");
}

/// A piece "taken off" a thing whose shape stays the same shows nothing, so
/// such an answer is not cached: the next time, the interpreter is asked again.
#[test]
fn pieces_that_leave_the_shape_unchanged_are_not_cached() {
    let w = world("nocache", 55);
    let hut = add_type(&w, &fixture("sims/hut.js"));
    let at = dry_spot(&w, 9.0, 0.3);
    let inst = place(&w, hut, at, 0.0);
    let calls = Arc::new(Mutex::new(0usize));
    let c2 = calls.clone();
    let llm = Llm::scripted(world("nocache-llm", 1).db.clone(), Arc::new(move |sys: &str, _msgs: &[Msg]| {
        if sys.contains("physics and common sense") {
            *c2.lock() += 1;
            return r#"{"narration": "You pull the hide off the doorway.", "create": [{"name": "stone", "description": "a hide"}], "cache": true}"#.into();
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let mut s = session(&w, 25, Some(llm));
    let h = s.sim.liven(&Target::Instance(inst)).unwrap();
    let hp = s.sim.things.get(h).unwrap().pos;
    s.sim.player.pos = Vec3::new(hp.x + 3.6, w.terrain.height(hp.x + 3.6, hp.z), hp.z);
    for _ in 0..2 {
        let r = act_once(&mut s, ActorId::Player, Action::Do { text: "remove the cover from the doorway".into(), on: Some(Target::Thing(h)), at: None }, 0.3);
        assert_eq!(r["ok"], true, "{r}");
    }
    assert_eq!(*calls.lock(), 2, "asked again, not answered from the cache");
}

/// A reshape named like the thing already is still changes it (its old
/// name would only find its old shape again).
#[test]
fn a_reshape_keeping_the_old_name_still_changes_the_shape() {
    let w = world("samename", 56);
    let src = fixture("sims/hut.js");
    let hut = add_type(&w, &src);
    let at = dry_spot(&w, 9.0, 0.3);
    let inst = place(&w, hut, at, 0.0);
    let src2 = src.clone();
    let llm = Llm::scripted(world("samename-llm", 1).db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("physics and common sense") {
            return r#"{"narration": "The door swings open.", "reshape": [{"target": "target", "name": "wooden hut", "change": "the door stands open"}]}"#.into();
        }
        if user.contains("Change this existing object") {
            let name = user.split("Set meta.name to exactly \"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
            return format!("```js\n{}\n```", src2.replace("name: \"wooden hut\"", &format!("name: \"{name}\"")).replace("return min(walls, roof);", "return min(subtract(walls, box(x, y - 0.9, z - 2.0, 0.4, 0.9, 0.3)), roof);"));
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let mut s = session(&w, 26, Some(llm));
    let h = s.sim.liven(&Target::Instance(inst)).unwrap();
    let hp = s.sim.things.get(h).unwrap().pos;
    s.sim.player.pos = Vec3::new(hp.x + 3.6, w.terrain.height(hp.x + 3.6, hp.z), hp.z);
    let r = act_once(&mut s, ActorId::Player, Action::Do { text: "open this door".into(), on: Some(Target::Thing(h)), at: None }, 0.3);
    assert_eq!(r["ok"], true, "{r}");
    s.pump(std::time::Duration::from_secs(60));
    s.run(0.5, 0.05);
    let t = s.sim.things.get(h).unwrap();
    assert_ne!(t.type_id, hut, "a new shape, not the old one again");
    assert!(s.sim.thing_name(h).starts_with("wooden hut"), "{}", s.sim.thing_name(h));
}

/// Earlier versions of a reshaped thing that nothing uses any more stay out
/// of the shader; reshaped types still in use, and ordinary types even
/// unused, stay in.
#[test]
fn unused_reshape_versions_are_left_out_of_the_scene() {
    let w = world("prune", 57);
    let src = fixture("sims/hut.js");
    let plain = add_type(&w, &src);
    let reshape_type = |summary: &str| {
        let ct = compile(&src).unwrap();
        let rep = probe(&ct).unwrap();
        let meta = serde_json::json!({ "name": ct.meta.name, "bounds": ct.meta.bounds, "tags": ct.meta.tags, "bottom": rep.bottom, "top": rep.top }).to_string();
        w.db.with(|c| {
            let v = db::add_version(c, Some(w.version), "interp", summary, None)?;
            db::add_type(c, Some(v), &ct.meta.name, &src, &meta, "ok", "")
        })
        .unwrap() as u32
    };
    let old = reshape_type("reshaped: wooden hut with a door");
    let kept = reshape_type("reshaped: wooden hut with a chimney");
    let made = reshape_type("new kind of thing: wooden hut");
    place(&w, kept, dry_spot(&w, 9.0, 0.3), 0.0);
    let s = session(&w, 31, None);
    let has = |id: u32| s.sim.snap.type_of(id).is_some();
    assert!(!has(old), "an unused reshape version is dropped");
    assert!(has(kept), "a reshape version still in use stays");
    assert!(has(plain) && has(made), "other types stay even unused");
}

// ------------------------------------------------------------------ species

/// A being of some species (a dog, a horse…) living near `home`.
fn add_being(w: &W, name: &str, species: &str, relationships: &[&str], home: Vec3) -> i64 {
    let persona = crate::world::Persona { name: name.into(), species: species.into(), relationships: relationships.iter().map(|s| s.to_string()).collect(), ..Default::default() };
    w.db.with(|c| db::add_character(c, crate::world::region_of(home.x, home.z), &serde_json::to_string(&persona)?, home.x, home.z, w.version)).unwrap()
}

fn calm(s: &mut Session) {
    for n in s.sim.cast.npcs.iter_mut() {
        n.needs = crate::world::characters::Needs { hunger: 0.0, fatigue: 0.0, social: 0.0, fun: 0.0, curiosity: 0.0 };
        n.think_at = f64::MAX;
    }
}

/// Phase 1: a dog lives in its own body but plays the same gestures as a
/// person (wave raises a paw, sit lies it down, a hug is a paw-and-lean),
/// holds things in its mouth, and is drawn at its own size.
#[test]
fn a_dog_plays_the_same_gestures_with_its_own_body() {
    use super::actor::{CROUCH, R_RAISE, SPREAD};
    let w = world("dogbody", 41);
    let p = dry_spot(&w, 8.0, 0.4);
    let ola = add_char(&w, "Ola", "kind and patient", &["Rex: her dog"], p);
    let rex = add_being(&w, "Rex", "dog", &["Ola: owner"], p + Vec3::new(1.2, 0.0, 0.0));
    let mut s = session(&w, 11, None);
    calm(&mut s);
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -12.0);
    let (o, r) = (ActorId::Npc(ola), ActorId::Npc(rex));
    {
        let dog = s.sim.cast.get(rex).unwrap();
        assert_eq!(dog.species.name, "dog");
        assert_ne!(Some(dog.body_ty), s.sim.snap.figure_type, "a dog has its own body");
        assert!(dog.a.dims.height > 0.4 && dog.a.dims.height < 0.8 && !dog.a.dims.arms, "{:?}", dog.a.dims);
        let person = s.sim.cast.get(ola).unwrap();
        assert!(person.species.is_human() && person.a.dims.arms && (person.a.dims.eye - 1.65 * person.a.dims.height / 1.75).abs() < 0.01);
    }
    assert_eq!(s.sim.social.rel(r, o).and_then(|x| x.owner), Some(ola), "Rex is Ola's dog");
    // Wave: the dog raises a paw.
    s.sim.act(r, Action::Gesture { kind: "wave".into(), to: None }).unwrap();
    s.run(0.8, 0.05);
    assert!(s.sim.cast.get(rex).unwrap().a.pose[R_RAISE] > 0.3, "paw up: {:?}", s.sim.cast.get(rex).unwrap().a.pose);
    s.run(2.0, 0.1);
    // Wag: only a body with a tail shows it.
    s.sim.act(r, Action::Gesture { kind: "wag".into(), to: None }).unwrap();
    let mut swing: f32 = 0.0;
    for _ in 0..12 {
        s.run(0.1, 0.05);
        swing = swing.max(s.sim.cast.get(rex).unwrap().a.pose[SPREAD].abs());
    }
    assert!(swing > 0.3, "the tail wags ({swing})");
    s.sim.act(o, Action::Gesture { kind: "wag".into(), to: None }).unwrap();
    s.run(0.5, 0.05);
    assert_eq!(s.sim.cast.get(ola).unwrap().a.pose[SPREAD], 0.0, "people have no tail");
    s.run(2.0, 0.1);
    // Sit: it lies down.
    s.sim.act(r, Action::Gesture { kind: "sit".into(), to: None }).unwrap();
    s.run(5.0, 0.1);
    assert!(s.sim.cast.get(rex).unwrap().a.pose[CROUCH] > 0.5);
    s.run(17.0, 0.2);
    // A hug with its person, through the same consent and distance rules.
    let res = s.sim.act(r, Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(o)) });
    assert!(res.is_ok(), "{res:?}");
    s.run(15.0, 0.1);
    assert!(!events(&s.sim, "hug").is_empty(), "Rex and Ola hugged: {:?}", s.sim.log.recent.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    // It holds things in its mouth: low and in front, not in a hand.
    let dog = s.sim.cast.get(rex).unwrap();
    let h = dog.a.hand(false) - dog.a.pos;
    assert!(h.y < 0.5 && h.y > 0.15 && h.dot(dog.a.forward()) > 0.25, "mouth at {h:?}");
    // Drawn with its own body, at its own size.
    let body = s.sim.snap.type_of(dog.body_ty).unwrap();
    assert_eq!(body.name(), "quadruped");
    let g = dog.gpu(body);
    assert!((g.pos_scale[3] - 0.62).abs() < 1e-3);
    sound(&s);
}

/// Render what the session shows from `cam` to a PNG (for looking at).
fn render_png(s: &mut Session, w: &W, cam: crate::render::Camera, out: &str) {
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    s.sim.flip(snap.clone());
    let (pw, ph) = (320u32, 180u32);
    let drawn = s.sim.draw(cam.pos, crate::render::VIEW_DIST);
    let culled = crate::world::cull::cull(&snap, &mut s.sim.cache, &cam, pw as f32 / ph as f32, &drawn.insts, &Default::default());
    let light = crate::render::sky::lighting(s.sim.t, &snap.look.palette);
    let sp = crate::render::SceneParams { terrain: &snap.terrain, palette: &snap.look.palette, camera: cam, width: pw, height: ph, pixel_aspect: 1.0, light, time: 1.0, frame: 0, shadows: true, lights: &drawn.lights };
    let globals = crate::render::build_globals(&sp, culled.insts.len(), culled.grid.as_ref());
    let req = crate::render::FrameRequest { id: 1, width: pw, height: ph, globals, instances: culled.insts, grid: culled.grid, scene: snap.scene.clone(), terrain: snap.terrain.clone(), look: snap.look.clone() };
    let mut handle = match &gpu {
        Some(g) => crate::render::gpu::spawn(g.clone()),
        None => crate::render::cpu::spawn(),
    };
    handle.tx.send(crate::render::RenderMsg::Frame(Box::new(req))).unwrap();
    let f = handle.rx.recv_timeout(std::time::Duration::from_secs(60)).unwrap();
    handle.shutdown();
    let sc = 3u32;
    let mut rgb = Vec::new();
    for y in 0..f.height * sc {
        for x in 0..f.width * sc {
            rgb.extend_from_slice(&crate::render::unpack(f.pixels[((y / sc) * f.width + x / sc) as usize]));
        }
    }
    std::fs::write(out, crate::png::encode(f.width * sc, f.height * sc, &rgb)).unwrap();
    eprintln!("wrote {out}");
}

/// Render the built-in species side by side (look at it).
#[test]
#[ignore]
fn species_lineup_picture() {
    let out = std::env::var("POCKET_PNG").unwrap_or_else(|_| std::env::temp_dir().join("pocket-species.png").to_string_lossy().into_owned());
    let w = world("lineup", 42);
    let me = w.spawn;
    let names = [("Ola", "human"), ("Rex", "dog"), ("Tib", "cat"), ("Bram", "horse"), ("Grey", "wolf"), ("Nan", "goat"), ("Doe", "deer")];
    let mut ids = Vec::new();
    for (i, (n, sp)) in names.iter().enumerate() {
        let x = (i as f32 - 3.0) * 1.9;
        ids.push(if *sp == "human" { add_char(&w, n, "kind", &[], ground(&w, me.x + x, me.z + 7.0)) } else { add_being(&w, n, sp, &[], ground(&w, me.x + x, me.z + 7.0)) });
    }
    let mut s = session(&w, 16, None);
    calm(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.45;
    for id in &ids {
        let n = s.sim.cast.get_mut(*id).unwrap();
        n.a.yaw = -1.3;
    }
    if let Ok(g) = std::env::var("POCKET_GESTURE") {
        for id in &ids {
            let _ = s.sim.act(ActorId::Npc(*id), Action::Gesture { kind: g.clone(), to: None });
        }
        s.run(1.0, 0.05);
    }
    let cam = crate::render::Camera { pos: me + Vec3::Y * 1.4 + Vec3::new(0.0, 0.0, -1.5), yaw: 0.0, pitch: -0.1, fov_y: 1.0 };
    render_png(&mut s, &w, cam, &out);
}

/// Render a cat three ways, side on: on the generic quadruped (as before
/// cats had their own body), on its own body, and with a poofy tail.
#[test]
#[ignore]
fn cat_bodies_picture() {
    let out = std::env::var("POCKET_PNG").unwrap_or_else(|_| std::env::temp_dir().join("pocket-cats.png").to_string_lossy().into_owned());
    let w = world("cats", 44);
    let cat = fixture("species/cat.js");
    add_type(&w, &cat);
    add_type(&w, &cat.replace("name: \"cat\"", "name: \"fluffy cat\"").replace("const tr = 0.028 + clamp(k.b, 0, 1) * 0.05;", "const tr = 0.07 + clamp(k.b, 0, 1) * 0.04;"));
    let row = |body: &str, size: f32| format!(r#"{{"name":"{body} kind","body":"{body}","size":{size},"mind":"simple","look":{{"hue":[0.07,0.07],"shade":[0.55,0.55]}}}}"#);
    w.db.with(|c| db::put_species(c, "quadruped kind", &row("quadruped", 0.34))).unwrap();
    w.db.with(|c| db::put_species(c, "cat kind", &row("cat", 1.06))).unwrap();
    w.db.with(|c| db::put_species(c, "fluffy cat kind", &row("fluffy cat", 1.06))).unwrap();
    let me = w.spawn;
    let mut ids = Vec::new();
    for (i, sp) in ["quadruped kind", "cat kind", "fluffy cat kind"].iter().enumerate() {
        let x = (i as f32 - 1.0) * 0.75;
        ids.push(add_being(&w, &format!("C{i}"), sp, &[], ground(&w, me.x + x, me.z + 2.0)));
    }
    let mut s = session(&w, 17, None);
    calm(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.45;
    for id in &ids {
        let n = s.sim.cast.get_mut(*id).unwrap();
        n.a.yaw = -std::f32::consts::FRAC_PI_2;
        n.a.pos.y = s.sim.snap.terrain.height(n.a.pos.x, n.a.pos.z);
    }
    if let Ok(g) = std::env::var("POCKET_GESTURE") {
        for id in &ids {
            let _ = s.sim.act(ActorId::Npc(*id), Action::Gesture { kind: g.clone(), to: None });
        }
        s.run(0.6, 0.05);
    }
    let y = s.sim.snap.terrain.height(me.x, me.z + 2.0);
    let cam = crate::render::Camera { pos: Vec3::new(me.x, y + 0.45, me.z + 0.6), yaw: 0.0, pitch: -0.12, fov_y: 1.0 };
    render_png(&mut s, &w, cam, &out);
}

/// Phase 2: a dog follows its person, brings back what they throw and gives
/// it to them, and wags when they meet after a while apart. A cat that
/// doesn't know you won't be hugged. Talking to a dog gets a noise and a
/// look, never words.
#[test]
fn a_dog_follows_and_fetches_and_a_wary_cat_keeps_its_distance() {
    let w = world("fetch", 43);
    let p = dry_spot(&w, 10.0, 0.9);
    let ola = add_char(&w, "Ola", "kind and patient", &["Rex: her dog"], p);
    let rex = add_being(&w, "Rex", "dog", &["Ola: owner"], p + Vec3::new(2.0, 0.0, 0.0));
    let tib = add_being(&w, "Tib", "cat", &[], p + Vec3::new(-6.0, 0.0, 3.0));
    let mut s = session(&w, 12, None);
    calm(&mut s);
    s.sim.player.pos = p + Vec3::new(-5.0, 0.0, 0.0);
    let (o, r, c) = (ActorId::Npc(ola), ActorId::Npc(rex), ActorId::Npc(tib));
    s.sim.cast.get_mut(rex).unwrap().think_at = 0.0;
    s.sim.cast.get_mut(rex).unwrap().needs.social = 0.5;
    // Ola walks off; Rex goes with her.
    let far = dry_spot(&w, 34.0, 0.9);
    s.sim.act(o, Action::Goto { target: Target::Point(far.to_array()), run: false }).unwrap();
    s.run(40.0, 0.1);
    let (op, rp) = (s.sim.actor(o).unwrap().pos, s.sim.actor(r).unwrap().pos);
    assert!((op - far).length() < 3.0, "Ola got there");
    assert!((op - rp).length() < 6.0, "Rex stayed with Ola ({:.1} m): {:?}", (op - rp).length(), s.sim.cast.get(rex).unwrap().decisions);
    assert!(events(&s.sim, "gesture").iter().any(|e| e.actor == Some(r) && e.data["kind"] == "wag"), "Rex wagged on meeting her");
    // Ola throws a stick; Rex brings it back to her.
    let stick = s.sim.type_by_name("stick").unwrap().id;
    let id = s.sim.spawn_thing(stick, op + s.sim.actor(o).unwrap().forward() * 0.6, 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.act(o, Action::Hold { target: Target::Thing(id) }).unwrap();
    s.run(0.5, 0.05);
    let a = s.sim.actor(o).unwrap().yaw;
    s.sim.act(o, Action::Throw { at: None, dir: Some([a.sin(), 0.5, a.cos()]), force: Some(9.0) }).unwrap();
    let thrown_to = { s.run(3.0, 0.05); s.sim.things.get(id).unwrap().pos };
    assert!((thrown_to - op).length() > 3.0, "the stick flew");
    s.run(30.0, 0.1);
    assert!(!events(&s.sim, "fetch").is_empty(), "Rex went after it: {:?}", s.sim.cast.get(rex).unwrap().decisions);
    let gave: Vec<_> = events(&s.sim, "gave").into_iter().filter(|e| e.actor == Some(r)).collect();
    assert!(!gave.is_empty(), "and gave it back: {:?} {:?}", s.sim.log.recent.iter().rev().take(20).map(|e| e.text.clone()).collect::<Vec<_>>(), s.sim.cast.get(rex).unwrap().decisions);
    assert_eq!(s.sim.things.get(id).unwrap().holder, Some(o), "Ola has her stick again");
    // A cat that doesn't know you.
    let res = s.sim.act(ActorId::Player, Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(c)) });
    s.run(3.0, 0.1);
    assert!(res.is_err() || events(&s.sim, "hug").is_empty(), "no hug for a stranger: {res:?}");
    // Talking to a dog: a noise and a look, never words, and no LLM call.
    s.sim.player.pos = s.sim.actor(r).unwrap().pos + Vec3::new(1.5, 0.0, 0.0);
    let before = events(&s.sim, "sound").len();
    s.sim.player_talks(rex, "Good boy! Who's a good boy?");
    let reqs = s.sim.drain_requests();
    assert!(!reqs.iter().any(|q| matches!(q, super::Request::Talk { .. })), "no dialogue for a dog");
    assert!(events(&s.sim, "sound").len() > before, "Rex answers with a noise");
    assert!(!events(&s.sim, "said").iter().any(|e| e.actor == Some(r)), "Rex never says words");
    sound(&s);
}

/// Two peoples for a fantasy valley: both use the human body, with their
/// own looks, and start out distrusting each other.
fn add_elves_and_orcs(w: &W) {
    let elf = serde_json::json!({ "name": "elf", "plural": "elves", "body": "figure", "mind": "sapient", "speech": "words", "social": "village",
        "temper": { "bold": 0.4, "wary": 0.6, "playful": 0.5, "tame": 0.5 }, "look": { "height": [1.85, 2.0], "build": [0.7, 0.85], "skin": [0.0, 0.2] } });
    let orc = serde_json::json!({ "name": "orc", "plural": "orcs", "body": "figure", "mind": "sapient", "speech": "words", "social": "village",
        "temper": { "bold": 0.8, "wary": 0.3, "playful": 0.6, "tame": 0.4 }, "look": { "height": [1.7, 1.9], "build": [1.2, 1.4], "skin": [0.55, 0.8] }, "mass": 95 });
    w.db.with(|c| {
        db::put_species(c, "elf", &elf.to_string())?;
        db::put_species(c, "orc", &orc.to_string())
    })
    .unwrap();
    w.db.kv_set("species.world", &serde_json::json!({ "attitudes": [{ "a": "elf", "b": "orc", "affection": -0.4, "trust": -0.3, "rivalry": 0.3 }] }).to_string()).unwrap();
}

/// Phase 3: the taxonomy at work. Hungry wolves go after goats (smaller,
/// and not their kin) but never after a grown person; the goats bolt as a
/// herd and keep together; in a world that doesn't hunt, nobody dies. Elves
/// and orcs start out cold; the pair who play together warm up.
#[test]
fn wolves_chase_a_herd_and_peoples_warm_to_each_other() {
    let w = world("taxonomy", 44);
    add_elves_and_orcs(&w);
    let farm = dry_spot(&w, 12.0, 2.0);
    let goats: Vec<i64> = (0..4).map(|i| add_being(&w, &format!("Goat {i}"), "goat", &[], farm + Vec3::new(i as f32 * 1.5, 0.0, (i % 2) as f32 * 1.5))).collect();
    let den = farm + Vec3::new(30.0, 0.0, 6.0);
    let wolves: Vec<i64> = (0..2).map(|i| add_being(&w, &format!("Wolf {i}"), "wolf", &[], den + Vec3::new(i as f32 * 1.2, 0.0, 0.0))).collect();
    let ola = add_char(&w, "Ola", "a kind herder", &[], farm + Vec3::new(-3.0, 0.0, -3.0));
    let green = dry_spot(&w, 40.0, 4.0);
    let mk = |name: &str, sp: &str, at: Vec3| {
        let persona = crate::world::Persona { name: name.into(), species: sp.into(), personality: "playful and lively".into(), ..Default::default() };
        w.db.with(|c| db::add_character(c, crate::world::region_of(at.x, at.z), &serde_json::to_string(&persona)?, at.x, at.z, w.version)).unwrap()
    };
    let (e1, o1) = (mk("Ael", "elf", green), mk("Grub", "orc", green + Vec3::new(4.0, 0.0, 0.0)));
    let (e2, o2) = (mk("Siv", "elf", green + Vec3::new(0.0, 0.0, 30.0)), mk("Mog", "orc", green + Vec3::new(4.0, 0.0, 30.0)));
    let ball = add_type(&w, &fixture("sims/ball.js"));
    let ball_id = place(&w, ball, green + Vec3::new(2.0, 0.0, 1.0), 0.0);
    let mut s = session(&w, 13, None);
    s.sim.cfg.hunting = false;
    calm(&mut s);
    let all = record(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * (16.0 / 24.0);
    s.sim.player.pos = farm + Vec3::new(-8.0, 0.0, 0.0);
    for g in goats.iter().chain(&wolves) {
        s.sim.cast.get_mut(*g).unwrap().think_at = 0.0;
    }
    for wf in &wolves {
        s.sim.cast.get_mut(*wf).unwrap().needs.hunger = 0.95;
    }
    let (ae, ao) = (ActorId::Npc(e1), ActorId::Npc(o1));
    let start = s.sim.social.affection(ae, ao);
    assert!(start < -0.1, "elves and orcs start cold ({start})");
    assert!(s.sim.cast.get(e1).unwrap().a.dims.height > s.sim.cast.get(o1).unwrap().a.dims.height - 0.2, "elves are tall");
    let b = s.sim.liven(&Target::Instance(ball_id)).unwrap();
    s.sim.cast.get_mut(o1).unwrap().needs.fun = 0.9;
    s.sim.cast.get_mut(e1).unwrap().needs.fun = 0.9;
    let _ = s.sim.propose(ae, ao, "catch", Some(b));
    for _ in 0..10 {
        s.run(30.0, 0.1);
        if s.sim.social.joints.is_empty() {
            s.sim.cast.get_mut(o1).unwrap().needs.fun = 0.9;
            let _ = s.sim.propose(ae, ao, "catch", Some(b));
        }
    }
    let chases = of(&all, "chase");
    assert!(!chases.is_empty(), "the wolves hunted: {:?}", s.sim.cast.get(wolves[0]).unwrap().decisions);
    for c in &chases {
        let target = c.subject.clone().unwrap_or_default();
        assert!(goats.iter().any(|g| ActorId::Npc(*g).key() == target), "wolves only go after goats, not {target}");
    }
    let fled = of(&all, "fled");
    assert!(fled.iter().any(|e| e.data["herd"] == true), "the goats bolted as a herd: {:?}", fled.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    assert!(of(&all, "died").is_empty(), "nobody dies in a world that doesn't hunt");
    assert!(!of(&all, "chase").iter().any(|e| e.subject.as_deref() == Some(&ActorId::Npc(ola).key())), "Ola is never hunted");
    let gp: Vec<Vec3> = goats.iter().map(|g| s.sim.actor(ActorId::Npc(*g)).unwrap().pos).collect();
    let centre = gp.iter().fold(Vec3::ZERO, |a, p| a + *p) / gp.len() as f32;
    let spread = gp.iter().map(|p| (*p - centre).length()).sum::<f32>() / gp.len() as f32;
    assert!(spread < 15.0, "the herd kept together ({spread:.1} m from its middle on average)");
    // The pair who played warmed up; the pair who didn't stayed cold.
    let played = s.sim.social.affection(ae, ao);
    let other = s.sim.social.affection(ActorId::Npc(e2), ActorId::Npc(o2));
    assert!(played > start + 0.15 && played > other + 0.1, "play warms them: {start:.2} → {played:.2} (others {other:.2})");
    // In a world that hunts, a caught goat dies.
    s.sim.cfg.hunting = true;
    let (wf, g) = (wolves[0], goats[0]);
    let gpos = s.sim.actor(ActorId::Npc(g)).unwrap().pos;
    s.sim.cast.get_mut(wf).unwrap().a.pos = gpos + Vec3::new(1.0, 0.0, 0.0);
    s.sim.cast.get_mut(wf).unwrap().doing = "chasing Goat 0".into();
    s.sim.cast.get_mut(wf).unwrap().plan.clear();
    s.sim.cast.get_mut(wf).unwrap().a.task = None;
    s.sim.cast.get_mut(wf).unwrap().think_at = 0.0;
    s.sim.cast.get_mut(g).unwrap().think_at = f64::MAX;
    s.sim.cast.get_mut(g).unwrap().plan.clear();
    s.sim.cast.get_mut(g).unwrap().a.task = None;
    s.run(1.0, 0.1);
    assert!(s.sim.cast.get(g).unwrap().dead, "the goat was killed");
    assert!(!s.sim.actor_ids().contains(&ActorId::Npc(g)));
    sound(&s);
}

/// Phase 4, through the brain with a scripted LLM: genesis brings a dragon
/// and a naga, whose bodies the builder writes; a region plan brings a
/// dragon who loves its rider, a naga and a small herd with its person. The
/// dragon's hug is a wing-wrap through the shared `reach` role. A naga has
/// no arms to raise, so its own cheer is written once and kept.
#[test]
fn llm_written_species_live_hug_and_learn_their_own_gestures() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let w = world("species-llm", 47);
    let dragon = fixture("species/dragon.js");
    let naga = fixture("species/naga.js");
    let pine = fixture("mock/seapine.js");
    let db2 = w.db.clone();
    let cheers = Arc::new(AtomicUsize::new(0));
    let cheers2 = cheers.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if user.contains("Design the base layer") {
            return format!(
                "```json\n{}\n```\n```js\n{pine}\n```",
                serde_json::json!({ "name": "Emberreach", "biomes": [],
                    "species": [
                        { "name": "dragon", "body": "dragon", "body_description": "a winged dragon, four legs, long neck and tail", "mind": "simple", "speech": "sounds", "social": "solitary",
                          "diet": { "meat": 0.9, "plants": 0.1 }, "temper": { "bold": 0.9, "wary": 0.3, "playful": 0.3, "tame": 0.3 }, "move": { "walk": 2.0, "run": 8.0, "fly": 14.0 },
                          "mass": 3000, "sounds": ["a low rumble", "a puff of smoke", "a roar"], "description": "a great winged dragon" },
                        { "name": "naga", "body": "naga", "body_description": "a person's chest on a serpent's tail", "mind": "sapient", "speech": "words", "social": "village", "description": "serpent folk" }
                    ],
                    "attitudes": [{ "a": "naga", "b": "human", "affection": -0.2 }] })
            );
        }
        if user.contains("Body type to write: \"dragon\"") {
            // Shown the quadruped only as an example: a `from` it adds anyway goes.
            assert!(user.contains("Leave `from` out"));
            return format!("```js\n{}\n```", dragon.replacen("body: {", "body: { from: \"quadruped\",", 1));
        }
        if user.contains("Body type to write: \"naga\"") {
            return format!("```js\n{naga}\n```");
        }
        if user.contains("Plan the story layer") {
            let spawn: [f32; 3] = serde_json::from_str(&db2.kv_get("spawn").unwrap()).unwrap();
            let caps: Vec<i32> = user.split("Region (").nth(1).unwrap().split(')').next().unwrap().split(", ").map(|v| v.parse().unwrap()).collect();
            let (lx, lz) = (spawn[0] - caps[0] as f32 * 256.0, spawn[2] - caps[1] as f32 * 256.0);
            let cl = |v: f32| v.clamp(10.0, 246.0);
            return format!("```json\n{}\n```", serde_json::json!({ "name": "Ash Hollow", "mood": "smoky", "facts": [],
                "characters": [
                    { "name": "Ilsa", "look": { "height": 1.7 }, "personality": "brave and warm", "home_x": cl(lx + 12.0), "home_z": cl(lz + 14.0), "relationships": ["Vyrm: her dragon"] },
                    { "name": "Vyrm", "species": "dragon", "home_x": cl(lx + 19.0), "home_z": cl(lz + 14.0), "relationships": ["Ilsa: rider"] },
                    { "name": "Sess", "species": "naga", "personality": "cheerful", "home_x": cl(lx + 12.0), "home_z": cl(lz + 24.0), "relationships": [] }
                ],
                "creatures": [{ "species": "goat", "count": 3, "x": cl(lx + 6.0), "z": cl(lz + 4.0), "owner": "Ilsa" }] }));
        }
        if sys.contains("You animate a simple body") {
            if user.contains("as a naga does it") {
                cheers2.fetch_add(1, Ordering::SeqCst);
                return r#"{"duration": 1.6, "frames": [{"t": 0, "pose": {}}, {"t": 0.4, "pose": {"reach": 0.9, "head": -0.4, "spread": 0.8}}, {"t": 1, "pose": {}}]}"#.into();
            }
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let mut s = session(&w, 19, Some(llm));
    s.brain.send(crate::brain::Cmd::Genesis);
    for _ in 0..600 {
        s.step(0.05);
        if s.sim.snap.species.get("dragon").is_some() && s.sim.snap.body_type("dragon").is_some_and(|b| b.name() == "dragon") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let sp = s.sim.snap.species.clone();
    assert!(sp.get("dragon").is_some() && sp.get("naga").is_some(), "both species arrived");
    assert_eq!(s.sim.snap.body_type("dragon").map(|b| b.name().to_string()).as_deref(), Some("dragon"), "the dragon's body was written");
    assert_eq!(s.sim.snap.body_line("dragon"), vec!["dragon".to_string()], "a dragon is of no quadruped's line");
    assert!(sp.attitude("naga", "human").is_some());
    let r = crate::world::region_of(s.sim.snap.spawn.x, s.sim.snap.spawn.z);
    s.brain.send(crate::brain::Cmd::Region(r));
    for _ in 0..600 {
        s.step(0.05);
        if s.sim.cast.npcs.len() >= 6 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let find = |s: &Session, n: &str| s.sim.cast.npcs.iter().find(|x| x.name() == n).map(|x| x.def.id);
    let (ilsa, vyrm, sess) = (find(&s, "Ilsa").expect("Ilsa"), find(&s, "Vyrm").expect("Vyrm"), find(&s, "Sess").expect("Sess"));
    let goats: Vec<i64> = s.sim.cast.npcs.iter().filter(|n| n.species.name == "goat").map(|n| n.def.id).collect();
    assert_eq!(goats.len(), 3, "a herd of three");
    calm(&mut s);
    {
        let d = s.sim.cast.get(vyrm).unwrap();
        assert_eq!(d.species.name, "dragon");
        assert_eq!(s.sim.snap.type_of(d.body_ty).unwrap().name(), "dragon");
        assert!(d.a.dims.height > 3.0 && d.a.dims.flies, "{:?}", d.a.dims);
    }
    assert_eq!(s.sim.owner_of(goats[0]), Some(ActorId::Npc(ilsa)), "the goats are Ilsa's");
    assert_eq!(s.sim.owner_of(vyrm), Some(ActorId::Npc(ilsa)), "Ilsa is Vyrm's rider");
    // The dragon wraps its wings around Ilsa.
    let ip = s.sim.actor(ActorId::Npc(ilsa)).unwrap().pos;
    s.sim.cast.get_mut(vyrm).unwrap().a.pos = ip + Vec3::new(6.0, 0.0, 0.0);
    s.sim.player.pos = ip + Vec3::new(0.0, 0.0, -14.0);
    let res = s.sim.act(ActorId::Npc(vyrm), Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(ActorId::Npc(ilsa))) });
    assert!(res.is_ok(), "{res:?}");
    let mut wrap: f32 = 0.0;
    for _ in 0..200 {
        s.step(0.1);
        wrap = wrap.max(s.sim.cast.get(vyrm).unwrap().a.pose[super::actor::L_FWD]);
    }
    assert!(!events(&s.sim, "hug").is_empty(), "Vyrm and Ilsa hugged: {:?}", s.sim.log.recent.iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    assert!(wrap > 0.5, "with its wings ({wrap})");
    let gap = (s.sim.actor(ActorId::Npc(vyrm)).unwrap().pos - s.sim.actor(ActorId::Npc(ilsa)).unwrap().pos).length();
    assert!(gap > 1.5, "a dragon hugs from its side, not from inside ({gap:.1} m)");
    // A naga cheers: no arms to raise, so its own version is written once.
    s.sim.act(ActorId::Npc(sess), Action::Gesture { kind: "cheer".into(), to: None }).unwrap();
    for _ in 0..300 {
        s.step(0.05);
        if super::actor::GestureKind::parse("cheer@naga").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(super::actor::GestureKind::parse("cheer@naga").is_some(), "the naga's cheer was written");
    s.run(3.0, 0.1);
    s.sim.act(ActorId::Npc(sess), Action::Gesture { kind: "cheer".into(), to: None }).unwrap();
    let mut reach: f32 = 0.0;
    for _ in 0..12 {
        s.step(0.05);
        reach = reach.max(s.sim.cast.get(sess).unwrap().a.pose[super::actor::R_FWD]);
    }
    assert!(reach > 0.5, "and used: it reaches up instead ({reach})");
    assert_eq!(cheers.load(Ordering::SeqCst), 1, "written once, then kept");
    sound(&s);
}

/// Render LLM-style species (a dragon hugging its rider, a naga, elves and
/// orcs) to look at.
#[test]
#[ignore]
fn fantasy_lineup_picture() {
    let out = std::env::var("POCKET_PNG").unwrap_or_else(|_| std::env::temp_dir().join("pocket-fantasy.png").to_string_lossy().into_owned());
    let w = world("fantasy", 48);
    add_elves_and_orcs(&w);
    add_type(&w, &fixture("species/dragon.js"));
    add_type(&w, &fixture("species/naga.js"));
    w.db.with(|c| {
        db::put_species(c, "dragon", r#"{"name":"dragon","body":"dragon","mind":"simple","speech":"sounds","mass":3000}"#)?;
        db::put_species(c, "naga", r#"{"name":"naga","body":"naga"}"#)
    })
    .unwrap();
    let me = w.spawn;
    let at = |x: f32, z: f32| ground(&w, me.x + x, me.z + z);
    let ilsa = add_char(&w, "Ilsa", "brave", &["Vyrm: her dragon"], at(-1.2, 12.0));
    let vyrm = add_being(&w, "Vyrm", "dragon", &["Ilsa: rider"], at(1.5, 13.0));
    let _ = add_being(&w, "Sess", "naga", &[], at(-4.5, 9.0));
    let mk = |name: &str, sp: &str, p: Vec3| {
        let persona = crate::world::Persona { name: name.into(), species: sp.into(), ..Default::default() };
        w.db.with(|c| db::add_character(c, crate::world::region_of(p.x, p.z), &serde_json::to_string(&persona)?, p.x, p.z, w.version)).unwrap()
    };
    mk("Ael", "elf", at(4.5, 8.0));
    mk("Grub", "orc", at(6.0, 8.5));
    let mut s = session(&w, 20, None);
    calm(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.45;
    for n in s.sim.cast.npcs.iter_mut() {
        n.a.yaw = std::f32::consts::PI;
    }
    if std::env::var("POCKET_GESTURE").is_ok() {
        let _ = s.sim.act(ActorId::Npc(vyrm), Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(ActorId::Npc(ilsa))) });
        s.run(8.0, 0.1);
    }
    let cam = crate::render::Camera { pos: me + Vec3::Y * 2.2, yaw: 0.0, pitch: -0.08, fov_y: 1.05 };
    render_png(&mut s, &w, cam, &out);
}

/// Phase 5: a warrior village. The region plan gives its people a variety
/// (broad, armoured) and two layers the builder writes for the human body.
/// They wear them from the start, drawn in their own pose; armour slows them,
/// the wool cloak catches fire and the iron doesn't; friends let each other
/// dress them, strangers don't; what they wear is kept across a restart. In
/// a world of giants, the small traveler is small.
#[test]
fn a_warrior_village_wears_its_armour_and_giants_dwarf_the_traveler() {
    use super::actor::LEAN;
    let w = world("warriors", 49);
    let plate = fixture("layers/breastplate.js");
    let cloak = fixture("layers/cloak.js");
    let pine = fixture("mock/seapine.js");
    let ball = fixture("sims/ball.js");
    let db2 = w.db.clone();
    let saw_body = Arc::new(Mutex::new(false));
    let saw2 = saw_body.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |_sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if user.contains("Design the base layer") {
            return format!("```json\n{}\n```\n```js\n{pine}\n```", serde_json::json!({ "name": "Giantsholm", "biomes": [], "sizes": { "human": 1.8 }, "traveler_height": 0.9 }));
        }
        if user.contains("Plan the story layer") {
            let spawn: [f32; 3] = serde_json::from_str(&db2.kv_get("spawn").unwrap()).unwrap();
            let caps: Vec<i32> = user.split("Region (").nth(1).unwrap().split(')').next().unwrap().split(", ").map(|v| v.parse().unwrap()).collect();
            let (lx, lz) = (spawn[0] - caps[0] as f32 * 256.0, spawn[2] - caps[1] as f32 * 256.0);
            let cl = |v: f32| v.clamp(10.0, 246.0);
            return format!("```json\n{}\n```", serde_json::json!({ "name": "Ironhold", "mood": "proud", "facts": [],
                "new_types": [
                    { "name": "iron breastplate", "description": "plate armour", "tags": ["layer"], "fits": "figure", "props": { "mass": 12 } },
                    { "name": "red cloak", "description": "a wool cloak", "tags": ["layer"], "fits": "figure", "props": { "burns": 0.8 } },
                    { "name": "leather ball", "description": "a ball", "tags": ["ball"], "props": { "toy": 1 } }
                ],
                "things": [{ "type": "leather ball", "near": "Brann" }, { "type": "leather ball", "near": "" }, { "type": "no such thing", "near": "Tova" }],
                "varieties": [{ "species": "human", "name": "warrior", "look": { "build": [1.15, 1.35] }, "layers": ["iron breastplate", "red cloak"] }],
                "settlement": { "name": "Ironhold", "x": cl(lx + 40.0), "z": cl(lz + 40.0), "variety": "warrior", "buildings": [] },
                "characters": [
                    { "name": "Brann", "personality": "proud", "home_x": cl(lx + 12.0), "home_z": cl(lz + 14.0), "relationships": ["Tova: friend"] },
                    { "name": "Tova", "personality": "stern", "home_x": cl(lx + 14.0), "home_z": cl(lz + 14.0), "relationships": ["Brann: friend"], "layers": [] },
                    { "name": "Pell", "variety": "farmer", "personality": "gentle", "home_x": cl(lx + 16.0), "home_z": cl(lz + 20.0) }
                ] }));
        }
        if user.contains("Object type to write: \"iron breastplate\"") {
            *saw2.lock() = user.contains("This is a layer") && user.contains("name: \"figure\"");
            return format!("```js\n{plate}\n```");
        }
        if user.contains("Object type to write: \"red cloak\"") {
            return format!("```js\n{cloak}\n```");
        }
        if user.contains("Object type to write: \"leather ball\"") {
            return format!("```js\n{ball}\n```");
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let mut s = session(&w, 21, Some(llm));
    s.brain.send(crate::brain::Cmd::Genesis);
    for _ in 0..600 {
        s.step(0.05);
        if s.sim.snap.species.traveler_height.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // A world of giants, and a small traveler.
    let me = s.sim.player.dims;
    assert!((me.eye - 0.85).abs() < 0.05, "the traveler's eyes are low ({:.2} m)", me.eye);
    let r = crate::world::region_of(s.sim.snap.spawn.x, s.sim.snap.spawn.z);
    s.brain.send(crate::brain::Cmd::Region(r));
    for _ in 0..800 {
        s.step(0.05);
        if s.sim.cast.npcs.len() >= 3 && s.sim.things.live().filter(|t| t.worn.is_some()).count() >= 4 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(*saw_body.lock(), "the layer was written against the body's code");
    calm(&mut s);
    let find = |s: &Session, n: &str| s.sim.cast.npcs.iter().find(|x| x.name() == n).map(|x| x.def.id).unwrap();
    let (brann, tova, pell) = (find(&s, "Brann"), find(&s, "Tova"), find(&s, "Pell"));
    let (b, t, p) = (ActorId::Npc(brann), ActorId::Npc(tova), ActorId::Npc(pell));
    let giant = s.sim.cast.get(brann).unwrap().a.dims;
    assert!(giant.height > 2.8, "the people here are giants ({:.1} m)", giant.height);
    assert!(s.sim.strength(ActorId::Player) < 10.0 && s.sim.strength(b) > 50.0);
    assert_eq!(s.sim.cast.get(brann).unwrap().def.persona.variety, "warrior", "the settlement's people are warriors");
    // The plan's things lie about: one by Brann's home, one in the middle.
    let balls: Vec<Vec3> = s.sim.snap.instances.iter().filter(|pl| s.sim.snap.type_of(pl.type_id).is_some_and(|t| t.name() == "leather ball")).map(|pl| pl.pos).collect();
    assert_eq!(balls.len(), 2, "two balls placed");
    let home = s.sim.cast.get(brann).unwrap().def.home;
    assert!(balls.iter().any(|b| Vec3::new(b.x - home.x, 0.0, b.z - home.z).length() < 3.0), "one by Brann's home");
    assert_eq!(s.sim.cast.get(pell).unwrap().def.persona.variety, "farmer", "unless they are something else");
    let build = s.sim.cast.get(brann).unwrap().sliders[1];
    assert!((1.15..=1.35).contains(&build), "broad warriors ({build})");
    assert_eq!(s.sim.worn_by(b).len(), 2, "Brann wears his armour and cloak");
    assert!(s.sim.worn_by(p).is_empty(), "Pell, a farmer, wears neither");
    // Armour slows a body.
    assert!(s.sim.speeds(b).0 < s.sim.speeds(p).0 * 0.99, "armour is heavy");
    // Drawn in his pose: the plate leans with him.
    s.sim.cast.get_mut(brann).unwrap().a.pose[LEAN] = 0.4;
    let plate_ty = s.sim.type_by_name("iron breastplate").unwrap().id;
    let drawn = s.sim.draw(s.sim.actor(b).unwrap().pos, 50.0);
    let li = drawn.insts.iter().find(|g| g.info[0] == plate_ty).expect("the plate is drawn");
    assert!((li.s1[0] - 0.4).abs() < 1e-4 && (li.pos() - s.sim.actor(b).unwrap().pos).length() < 1e-3, "in Brann's place and pose");
    // A fire beside him: the wool burns, the iron doesn't.
    let stick = s.sim.type_by_name("stick").unwrap().id;
    let bp = s.sim.actor(b).unwrap().pos;
    for i in 0..3 {
        let f = s.sim.spawn_thing(stick, bp + Vec3::new(0.3 + i as f32 * 0.2, 0.0, 0.3), 0.0, 1.0, Default::default(), true).unwrap();
        s.sim.things.get_mut(f).unwrap().props[P_FIRE] = 1.0;
    }
    let cloak_id = s.sim.worn_by(b).into_iter().find(|id| s.sim.thing_name(*id) == "red cloak").unwrap();
    let plate_id = s.sim.worn_by(b).into_iter().find(|id| s.sim.thing_name(*id) == "iron breastplate").unwrap();
    s.run(15.0, 0.1);
    let (cl, pl) = (s.sim.things.get(cloak_id).map(|t| t.props.clone()), s.sim.things.get(plate_id).unwrap().props.clone());
    assert!(cl.is_none_or(|c| c[P_FIRE] > 0.0 || c[P_CHAR] > 0.0), "the cloak caught fire");
    assert!(pl[P_FIRE] == 0.0, "the plate didn't");
    // Friends dress each other; strangers don't get to.
    s.sim.cast.get_mut(tova).unwrap().a.pos = s.sim.actor(b).unwrap().pos + Vec3::new(1.0, 0.0, 0.0);
    let r = s.sim.act(b, Action::TakeOff { target: Some(Target::Thing(plate_id)), from: None });
    assert!(r.is_ok() && s.sim.actor(b).unwrap().held == Some(plate_id), "{r:?}");
    let r = s.sim.act(b, Action::Wear { target: None, on: Some(Target::Actor(t)) });
    assert!(r.is_ok(), "Tova lets her friend put it on her: {r:?}");
    assert!(s.sim.worn_by(t).contains(&plate_id));
    s.sim.player.pos = s.sim.actor(t).unwrap().pos + Vec3::new(0.0, 0.0, 1.0);
    let r = s.sim.act(ActorId::Player, Action::TakeOff { target: Some(Target::Thing(plate_id)), from: Some(Target::Actor(t)) });
    assert!(r.is_err(), "a stranger can't undress her");
    // Kept across a restart.
    s.save();
    drop(s);
    let s = session(&w, 22, None);
    assert!(s.sim.worn_by(ActorId::Npc(tova)).contains(&plate_id), "still wearing it after a restart");
    sound(&s);
}

/// Render warriors in plate and cloaks next to a farmer (look at it).
#[test]
#[ignore]
fn warriors_picture() {
    let out = std::env::var("POCKET_PNG").unwrap_or_else(|_| std::env::temp_dir().join("pocket-warriors.png").to_string_lossy().into_owned());
    let w = world("warpic", 50);
    add_type(&w, &fixture("layers/breastplate.js"));
    add_type(&w, &fixture("layers/cloak.js"));
    let mut human: serde_json::Value = serde_json::from_str(crate::world::species::BUILTIN_SPECIES).unwrap();
    let mut h = human.as_array_mut().unwrap().remove(0);
    h["varieties"] = serde_json::json!([{ "name": "warrior", "look": { "build": [1.15, 1.35] }, "layers": ["iron breastplate", "red cloak"] }]);
    w.db.with(|c| db::put_species(c, "human", &h.to_string())).unwrap();
    let me = w.spawn;
    let mut ids = Vec::new();
    for (i, v) in ["warrior", "warrior", "", "warrior"].iter().enumerate() {
        let p = ground(&w, me.x + (i as f32 - 1.5) * 1.6, me.z + 5.0);
        let persona = crate::world::Persona { name: format!("P{i}"), variety: v.to_string(), ..Default::default() };
        ids.push(w.db.with(|c| db::add_character(c, crate::world::region_of(p.x, p.z), &serde_json::to_string(&persona)?, p.x, p.z, w.version)).unwrap());
    }
    let mut s = session(&w, 23, None);
    calm(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.45;
    for (i, id) in ids.iter().enumerate() {
        let n = s.sim.cast.get_mut(*id).unwrap();
        n.a.yaw = if i == 1 { 0.0 } else { std::f32::consts::PI + 0.4 * (i as f32 - 1.5) };
    }
    let _ = s.sim.act(ActorId::Npc(ids[3]), Action::Gesture { kind: "bow".into(), to: None });
    s.run(0.8, 0.05);
    let cam = crate::render::Camera { pos: me + Vec3::Y * 1.5 + Vec3::new(0.0, 0.0, 0.5), yaw: 0.0, pitch: -0.12, fov_y: 0.9 };
    render_png(&mut s, &w, cam, &out);
}

/// Phase 6: deeds in words on beings. A guard who doesn't know you won't be
/// dressed by you; once he trusts you, the cloak (written for his body) goes
/// on. Feeding a horse makes it fonder of you, a dog learns a trick and
/// greets with it, a curse turns a man into a toad who still knows you
/// (only in a world with forces of its own), and a hound can be conjured
/// only where beings can be made.
#[test]
fn deeds_dress_feed_teach_curse_and_conjure_beings() {
    let w = world("deeds", 51);
    crate::sim::persist::set_universe_rules(&w.db, &[("cursed".into(), 0.0, "how cursed it is".into())], &[]).unwrap();
    w.db.with(|c| db::put_species(c, "toad", r#"{"name":"toad","body":"quadruped","size":0.32,"mind":"instinct","speech":"sounds","sounds":["a croak"],"mass":3,"look":{"legs":[0,0.1],"length":[0,0.2],"ears":[0,0.05],"hue":[0.25,0.3],"shade":[0.3,0.5]}}"#)).unwrap();
    let cloak = fixture("layers/cloak.js");
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |_sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        let fx = |v: serde_json::Value| v.to_string();
        if user.contains("\"give the guard a red cloak\"") {
            return fx(serde_json::json!({ "narration": "The traveler drapes a red cloak over the guard.", "being": { "wear": [{ "name": "red cloak", "description": "a red wool cloak", "props": { "burns": 0.8 } }] } }));
        }
        if user.contains("\"feed the horse an apple\"") {
            return fx(serde_json::json!({ "narration": "The horse crunches the apple.", "being": { "needs": { "hunger": -0.5 }, "feel": { "affection": 0.25, "trust": 0.2 } } }));
        }
        if user.contains("\"teach Rex to sit\"") {
            return fx(serde_json::json!({ "narration": "Rex sits.", "being": { "learn": "sit", "feel": { "affection": 0.05 } } }));
        }
        if user.contains("\"curse Brann into a toad\"") {
            return fx(serde_json::json!({ "narration": "Green smoke, and a toad blinks up.", "being": { "become": "toad" }, "cache": false }));
        }
        if user.contains("\"conjure a hound\"") {
            return fx(serde_json::json!({ "narration": "A hound steps out of the mist.", "beings": [{ "species": "dog", "name": "Mist", "look": { "hue": 1.2, "shade": 0.5 }, "size": 1.5 }] }));
        }
        if user.contains("\"make mist much larger\"") {
            assert!(user.contains("\"person\":\"Mist\""), "Mist is the target, named in the words");
            return fx(serde_json::json!({ "narration": "Mist grows.", "being": { "grow": 2.0 } }));
        }
        if user.contains("Object type to write: \"red cloak\"") {
            assert!(user.contains("This is a layer"), "written as a layer");
            return format!("```js\n{cloak}\n```");
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let p = dry_spot(&w, 9.0, 1.4);
    let brann = add_char(&w, "Brann", "a stern guard", &[], p);
    let bram = add_being(&w, "Bram", "horse", &[], p + Vec3::new(4.0, 0.0, 0.0));
    let rex = add_being(&w, "Rex", "dog", &[], p + Vec3::new(-3.0, 0.0, 1.0));
    let mut s = session(&w, 24, Some(llm));
    calm(&mut s);
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -1.5);
    let (g, h, d) = (ActorId::Npc(brann), ActorId::Npc(bram), ActorId::Npc(rex));
    let me = ActorId::Player;
    let deed = |s: &mut Session, text: &str, on: ActorId| {
        let r = act_once(s, me, Action::Do { text: text.into(), on: Some(Target::Actor(on)), at: None }, 0.3);
        for _ in 0..200 {
            s.step(0.05);
            if s.sim.interp.pending.is_empty() && s.sim.interp.building.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        r
    };
    // A stranger can't dress the guard; a friend can.
    deed(&mut s, "give the guard a red cloak", g);
    assert!(s.sim.worn_by(g).is_empty(), "he wouldn't let a stranger");
    assert!(events(&s.sim, "refused").iter().any(|e| e.actor == Some(g)));
    s.sim.social.bond(g, me, 0.5, s.sim.t);
    deed(&mut s, "give the guard a red cloak", g);
    for _ in 0..100 {
        s.step(0.05);
        if !s.sim.worn_by(g).is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.sim.worn_by(g).iter().map(|id| s.sim.thing_name(*id)).collect::<Vec<_>>(), vec!["red cloak".to_string()], "the cloak went on");
    // Feeding a horse.
    s.sim.player.pos = s.sim.actor(h).unwrap().pos + Vec3::new(0.0, 0.0, -1.5);
    s.sim.cast.get_mut(bram).unwrap().needs.hunger = 0.8;
    let before = s.sim.social.affection(h, me);
    deed(&mut s, "feed the horse an apple", h);
    assert!(s.sim.cast.get(bram).unwrap().needs.hunger < 0.4 && s.sim.social.affection(h, me) > before + 0.2, "the horse is fed and fonder");
    // Teaching a dog.
    s.sim.player.pos = s.sim.actor(d).unwrap().pos + Vec3::new(0.0, 0.0, -1.5);
    deed(&mut s, "teach Rex to sit", d);
    assert_eq!(s.sim.cast.get(rex).unwrap().tricks, vec!["sit".to_string()]);
    assert!(!events(&s.sim, "learned").is_empty());
    // A curse: still Brann, still knows the traveler, now a toad; the cloak falls off.
    let fond = s.sim.social.affection(g, me);
    s.sim.player.pos = s.sim.actor(g).unwrap().pos + Vec3::new(0.0, 0.0, -1.5);
    deed(&mut s, "curse Brann into a toad", g);
    let b = s.sim.cast.get(brann).unwrap();
    assert_eq!(b.species.name, "toad");
    assert!(b.a.dims.height < 0.5 && !b.a.dims.arms, "{:?}", b.a.dims);
    assert_eq!(b.name(), "Brann");
    assert!((s.sim.social.affection(g, me) - fond).abs() < 1e-4, "he still knows the traveler");
    assert!(s.sim.worn_by(g).is_empty(), "the cloak fell off");
    assert!(!events(&s.sim, "transformed").is_empty());
    // Conjuring: not in a world that forbids it, until it allows it.
    s.sim.cfg.create_beings = false;
    let n0 = s.sim.cast.npcs.len();
    deed(&mut s, "conjure a hound", me);
    assert_eq!(s.sim.cast.npcs.len(), n0, "beings can't be made here");
    s.sim.cfg.create_beings = true;
    deed(&mut s, "conjure a hound", me);
    let mist = s.sim.cast.npcs.iter().find(|n| n.name() == "Mist").map(|n| n.def.id).expect("a hound appeared");
    assert_eq!(s.sim.owner_of(mist), Some(me), "it is the traveler's");
    let m = s.sim.cast.get(mist).unwrap();
    assert_eq!(m.def.persona.look.get("hue"), Some(&1.2), "made grey, as asked");
    assert_eq!(m.def.persona.size, Some(1.5));
    assert_eq!(m.growth, 1.0, "made beings arrive grown, not as newborns");
    let h0 = m.a.dims.height;
    // Named, not pointed at: "make mist much larger" is done to Mist.
    s.sim.player.pos = s.sim.actor(ActorId::Npc(mist)).unwrap().pos + Vec3::new(0.0, 0.0, -3.0);
    act_once(&mut s, me, Action::Do { text: "make mist much larger".into(), on: None, at: None }, 0.3);
    for _ in 0..200 {
        s.step(0.05);
        if s.sim.interp.pending.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let h1 = s.sim.cast.get(mist).unwrap().a.dims.height;
    assert!((h1 / h0 - 2.0).abs() < 0.05, "twice as big: {h0} -> {h1}");
    s.save();
    drop(s);
    let s = session(&w, 25, None);
    assert_eq!(s.sim.cast.get(brann).unwrap().species.name, "toad", "still a toad after a restart");
    assert!(s.sim.cast.get(mist).is_some(), "and the hound is still here");
    sound(&s);
    // Without forces of its own, a world has no curses.
    let w2 = world("nocurse", 52);
    let c = add_char(&w2, "Ola", "kind", &[], dry_spot(&w2, 6.0, 0.3));
    let mut s2 = session(&w2, 26, None);
    assert!(s2.sim.transform_being(c, "dog", ActorId::Player).is_err());
}

/// Phase 7: a griffin flies over a house to its person and lands; the
/// traveler rides a horse that knows them and steers it with the walk keys
/// until a wolf spooks it and it throws them; a stranger's horse won't be
/// ridden; a character rides her griffin to a far place and gets down there.
#[test]
fn griffins_fly_horses_carry_and_bolt() {
    let w = world("riding", 53);
    add_type(&w, &fixture("species/dragon.js"));
    w.db.with(|c| db::put_species(c, "griffin", r#"{"name":"griffin","body":"dragon","size":0.55,"mind":"simple","speech":"sounds","sounds":["a shrill cry"],"social":"pair",
        "diet":{"meat":0.7,"plants":0.3},"temper":{"bold":0.7,"wary":0.3,"playful":0.4,"tame":0.9},"move":{"walk":1.8,"run":6,"fly":12},"mass":400}"#)).unwrap();
    let hut = add_type(&w, &fixture("sims/hut.js"));
    let a = dry_spot(&w, 10.0, 0.6);
    let far = dry_spot(&w, 70.0, 0.6);
    let ilsa = add_char(&w, "Ilsa", "brave and warm", &["Gale: her griffin"], far);
    let gale = add_being(&w, "Gale", "griffin", &["Ilsa: rider"], a);
    let mid = (a + far) * 0.5;
    let hut_id = place(&w, hut, ground(&w, mid.x, mid.z), 0.0);
    let _ = hut_id;
    let bram = add_being(&w, "Bram", "horse", &[], a + Vec3::new(-12.0, 0.0, 0.0));
    let nell = add_being(&w, "Nell", "horse", &[], a + Vec3::new(-12.0, 0.0, 8.0));
    let wolf = add_being(&w, "Grey", "wolf", &[], a + Vec3::new(-60.0, 0.0, -40.0));
    let mut s = session(&w, 27, None);
    calm(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * (16.0 / 24.0);
    let (i, g, b, n, wf) = (ActorId::Npc(ilsa), ActorId::Npc(gale), ActorId::Npc(bram), ActorId::Npc(nell), ActorId::Npc(wolf));
    s.sim.player.pos = a + Vec3::new(-10.0, 0.0, -2.0);
    assert!(s.sim.can_fly(g) && s.sim.actor(g).unwrap().dims.seat.is_some());
    // The griffin flies to Ilsa, over the house.
    let solids: Vec<crate::world::Solid> = s.sim.solids_near(mid, 20.0);
    assert!(!solids.is_empty(), "the house stands between them");
    s.sim.act(g, Action::Goto { target: Target::Actor(i), run: false }).unwrap();
    let mut top: f32 = 0.0;
    for _ in 0..400 {
        s.step(0.05);
        let x = s.sim.actor(g).unwrap();
        top = top.max(x.alt);
        for sd in &solids {
            assert!(sd.inst.sdf(&sd.ty.ct, x.pos + Vec3::Y * x.dims.height * 0.5) > 0.0, "never through the house");
        }
        if x.task.is_none() {
            break;
        }
    }
    let x = s.sim.actor(g).unwrap();
    assert!(top > 3.0, "it flew ({top:.1} m up)");
    assert!((x.pos - s.sim.actor(i).unwrap().pos).length() < 6.0 && x.alt < 0.2, "and landed by Ilsa ({:.1} m, {:.1} up)", (x.pos - s.sim.actor(i).unwrap().pos).length(), x.alt);
    // The traveler rides a horse that knows them.
    s.sim.social.bond(b, ActorId::Player, 0.6, s.sim.t);
    s.sim.player.pos = s.sim.actor(b).unwrap().pos + Vec3::new(1.2, 0.0, 0.0);
    let r = s.sim.act(ActorId::Player, Action::Ride { target: Target::Actor(b) });
    assert!(r.is_ok(), "{r:?}");
    let start = s.sim.actor(b).unwrap().pos;
    let dir = Vec3::new(0.0, 0.0, 1.0);
    for _ in 0..30 {
        s.sim.drive(dir, 0.0, 0.1);
        s.step(0.1);
    }
    let hp = s.sim.actor(b).unwrap().pos;
    assert!((hp - start).length() > 5.0, "the horse carried them ({:.1} m)", (hp - start).length());
    let me = s.sim.player.pos;
    assert!(me.y > s.sim.snap.terrain.height(me.x, me.z) + 0.5 && Vec3::new(me.x - hp.x, 0.0, me.z - hp.z).length() < 1.0, "sitting on its back");
    // A wolf: the horse bolts and throws them.
    s.sim.cast.get_mut(wolf).unwrap().a.pos = hp + Vec3::new(5.0, 0.0, 2.0);
    s.sim.cast.get_mut(bram).unwrap().think_at = 0.0;
    s.run(1.0, 0.1);
    assert!(s.sim.player.riding.is_none(), "thrown off");
    assert!(!events(&s.sim, "thrown").is_empty() && !events(&s.sim, "fled").is_empty(), "the horse bolted");
    let _ = wf;
    // A horse that doesn't know them won't have them.
    s.sim.player.pos = s.sim.actor(n).unwrap().pos + Vec3::new(1.2, 0.0, 0.0);
    assert!(s.sim.act(ActorId::Player, Action::Ride { target: Target::Actor(n) }).is_err(), "a stranger's horse shies away");
    // Ilsa rides Gale to a far place, and gets down there.
    s.sim.cast.get_mut(wolf).unwrap().a.pos = a + Vec3::new(-200.0, 0.0, -200.0);
    let dest = dry_spot(&w, 170.0, 0.6);
    s.sim.act(i, Action::Goto { target: Target::Point(dest.to_array()), run: false }).unwrap();
    let mut high: f32 = 0.0;
    for _ in 0..600 {
        s.step(0.1);
        high = high.max(s.sim.actor(g).unwrap().alt);
        if s.sim.actor(i).unwrap().task.is_none() {
            break;
        }
    }
    assert!(high > 3.0, "they flew ({high:.1} m up)");
    assert!(!events(&s.sim, "mounted").iter().all(|e| e.actor != Some(i)), "Ilsa climbed onto Gale");
    let ip = s.sim.actor(i).unwrap().pos;
    let gp = s.sim.actor(g).unwrap().pos;
    assert!(Vec3::new(ip.x - dest.x, 0.0, ip.z - dest.z).length() < 8.0 && (gp - ip).length() < 6.0, "both got there ({:.1} m, {:.1} m apart)", Vec3::new(ip.x - dest.x, 0.0, ip.z - dest.z).length(), (gp - ip).length());
    assert!(s.sim.actor(i).unwrap().riding.is_none(), "and she got down");
    sound(&s);
}

/// Render riders: one on a horse, one on a griffin in the air (look at it).
#[test]
#[ignore]
fn riders_picture() {
    let out = std::env::var("POCKET_PNG").unwrap_or_else(|_| std::env::temp_dir().join("pocket-riders.png").to_string_lossy().into_owned());
    let w = world("ridepic", 54);
    add_type(&w, &fixture("species/dragon.js"));
    w.db.with(|c| db::put_species(c, "griffin", r#"{"name":"griffin","body":"dragon","size":0.55,"mind":"simple","speech":"sounds","temper":{"tame":0.9},"move":{"fly":12},"mass":400}"#)).unwrap();
    let me = w.spawn;
    let at = |x: f32, z: f32| ground(&w, me.x + x, me.z + z);
    let ola = add_char(&w, "Ola", "kind", &["Bram: her horse"], at(-2.5, 9.0));
    let bram = add_being(&w, "Bram", "horse", &["Ola: rider"], at(-2.5, 9.0));
    let ilsa = add_char(&w, "Ilsa", "brave", &["Gale: her griffin"], at(3.0, 12.0));
    let gale = add_being(&w, "Gale", "griffin", &["Ilsa: rider"], at(3.0, 12.0));
    let mut s = session(&w, 28, None);
    calm(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.45;
    s.sim.cast.get_mut(bram).unwrap().a.yaw = -1.4;
    s.sim.cast.get_mut(gale).unwrap().a.yaw = 1.2;
    s.sim.ride(ActorId::Npc(ola), ActorId::Npc(bram)).unwrap();
    s.sim.ride(ActorId::Npc(ilsa), ActorId::Npc(gale)).unwrap();
    {
        let g = s.sim.cast.get_mut(gale).unwrap();
        g.a.pos.y += 3.5;
        g.a.alt = 3.5;
    }
    s.sim.update_riders();
    for _ in 0..8 {
        for id in [ola, bram, ilsa, gale] {
            let t = s.sim.t;
            let n = s.sim.cast.get_mut(id).unwrap();
            n.a.update_pose(t, 0.1, None);
        }
        s.sim.t += 0.1;
    }
    let cam = crate::render::Camera { pos: me + Vec3::Y * 2.0, yaw: 0.0, pitch: 0.02, fov_y: 1.05 };
    render_png(&mut s, &w, cam, &out);
}

/// One run of a wolf valley: two pairs whose villagers feed them; their young
/// grow up together and pair across the two lines. Returns who was born, in
/// order, and the session.
fn wolf_generations(seed: u64) -> (Vec<String>, Session, Arc<Mutex<Vec<SimEvent>>>, i64) {
    let w = world("wolfline", 55);
    let den = dry_spot(&w, 14.0, 2.6);
    let ola = add_char(&w, "Ola", "a kind villager who feeds the wolves", &[], den + Vec3::new(-4.0, 0.0, 0.0));
    let founders: Vec<i64> = (0..4).map(|i| add_being(&w, &format!("Wolf {i}"), "wolf", &[], den + Vec3::new((i % 2) as f32 * 1.5, 0.0, (i / 2) as f32 * 6.0))).collect();
    let mut s = session(&w, seed, None);
    s.sim.cfg.life_speed = 8.0;
    s.sim.cfg.max_creatures = 30;
    let all = record(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * (15.0 / 24.0);
    s.sim.player.pos = den + Vec3::new(-15.0, 0.0, -15.0);
    for n in s.sim.cast.npcs.iter_mut() {
        n.think_at = 0.0;
    }
    let o = ActorId::Npc(ola);
    let (a, b) = (ActorId::Npc(founders[0]), ActorId::Npc(founders[1]));
    let (c, d) = (ActorId::Npc(founders[2]), ActorId::Npc(founders[3]));
    s.sim.social.bond(a, b, 0.8, 0.0);
    s.sim.social.bond(c, d, 0.8, 0.0);
    for _ in 0..140 {
        // The villager feeds the wolves; the young who grow up together pair up.
        let t = s.sim.t;
        let wolves: Vec<i64> = s.sim.cast.npcs.iter().filter(|n| n.species.name.contains("wolf") && !n.dead).map(|n| n.def.id).collect();
        for wf in &wolves {
            if let Some(n) = s.sim.cast.get_mut(*wf) {
                n.needs.hunger = 0.0;
            }
            s.sim.social.bond(ActorId::Npc(*wf), o, 0.05, t);
        }
        let adults: Vec<(i64, i64)> = s.sim.cast.npcs.iter().filter(|n| n.born > 0.0 && n.growth >= 1.0 && !n.dead).map(|n| (n.def.id, n.lineage)).collect();
        for (x, lx) in &adults {
            for (y, ly) in &adults {
                if x < y && lx != ly && s.sim.social.affection(ActorId::Npc(*x), ActorId::Npc(*y)) < 0.7 {
                    s.sim.social.bond(ActorId::Npc(*x), ActorId::Npc(*y), 0.8, t);
                }
            }
        }
        s.run(60.0, 0.25);
    }
    let born: Vec<String> = of(&all, "born").into_iter().map(|e| e.text).collect();
    (born, s, all, ola)
}

/// Phase 8: wolves have young, who start small, keep to a parent and grow
/// up; the valley fills only so far; a line that villagers keep feeding gets
/// tamer with each generation until people call it by its own name. The
/// same seed gives the same family history.
#[test]
fn families_grow_and_a_fed_wolf_line_turns_tame() {
    let (born, s, all, _) = wolf_generations(29);
    assert!(born.len() >= 3, "young were born: {born:?}");
    let young: Vec<&super::npc::Npc> = s.sim.cast.npcs.iter().filter(|n| n.born > 0.0).collect();
    assert!(young.iter().any(|n| n.growth >= 1.0), "and grew up");
    let wolves = s.sim.cast.npcs.iter().filter(|n| n.species.name.contains("wolf") && !n.dead).count();
    assert!(wolves <= s.sim.cfg.max_creatures, "the valley keeps to its limit ({wolves})");
    // Newborns are small and keep close to a parent.
    let first = of(&all, "born").into_iter().next().unwrap();
    let kid = young.iter().find(|n| first.actor == Some(ActorId::Npc(n.def.id))).unwrap();
    assert_eq!(kid.parents.len(), 2);
    assert!(s.sim.social.rel(ActorId::Npc(kid.def.id), ActorId::Npc(kid.parents[0])).is_some_and(|r| r.family));
    // A fed line drifts tame, generation by generation, and gets a name.
    let tames: Vec<f32> = of(&all, "born").iter().map(|e| e.data["tame"].as_f64().unwrap_or(0.0) as f32).collect();
    let base = s.sim.snap.species.get("wolf").unwrap().temper.tame;
    let last = tames.iter().rev().take(2).sum::<f32>() / 2.0;
    assert!(last > base + 0.08, "later young are tamer ({base:.2} → {last:.2}): {tames:?}");
    let named = of(&all, "new_variety");
    assert!(!named.is_empty() || !of(&all, "new_species").is_empty(), "the line got its own name: {:?}", tames);
    if let Some(v) = named.first() {
        assert!(v.data["variety"].as_str().unwrap_or("").contains("tame"), "{v:?}");
    }
    sound(&s);
    // Same seed, same history.
    let (again, _, _, _) = wolf_generations(29);
    assert_eq!(born, again, "the same seed gives the same lineage history");
}

// ------------------------------------------------------------------ work in progress

/// Slow work shows while it goes: the deed while the world decides, then the
/// new thing while it is written. The deed counts ("interpreted", which
/// achievements watch) only once that thing exists, or as having come to
/// nothing when it never does.
#[test]
fn a_deed_shows_as_work_and_counts_once_its_making_is_done() {
    use super::Request;
    use super::interp::WorkKind;
    let w = world("work", 41);
    let stick = builtin_id(&w, "stick");
    let mut s = session(&w, 8, None);
    s.sim.has_llm = true;
    let ask = |s: &mut Session, text: &str| -> u64 {
        s.sim.act(ActorId::Player, Action::Do { text: text.into(), on: None, at: None }).unwrap();
        s.sim.drain_requests().into_iter().find_map(|r| if let Request::Interpret { id, .. } = r { Some(id) } else { None }).unwrap()
    };
    let built = |s: &mut Session| -> u64 { s.sim.drain_requests().into_iter().find_map(|r| if let Request::BuildType { id, .. } = r { Some(id) } else { None }).unwrap() };
    let answer = |name: &str| Ok(serde_json::json!({ "narration": format!("A {name} takes shape."), "create": [{ "name": name, "description": "new" }] }));

    let id = ask(&mut s, "whittle a flute");
    let work = s.sim.work();
    assert_eq!(work.len(), 1);
    assert_eq!((work[0].kind, work[0].who, work[0].what.as_str()), (WorkKind::Doing, Some(ActorId::Player), "whittle a flute"));
    s.sim.on_interpreted(id, answer("reed flute"));
    let b = built(&mut s);
    let work = s.sim.work();
    assert_eq!(work.len(), 1, "the deed became a making");
    assert_eq!((work[0].kind, work[0].who, work[0].what.as_str()), (WorkKind::Making, Some(ActorId::Player), "reed flute"));
    assert!(events(&s.sim, "interpreted").is_empty(), "not counted before it exists");
    s.sim.on_type_built(b, Some(stick));
    assert!(s.sim.work().is_empty());
    let done = events(&s.sim, "interpreted");
    assert_eq!(done.len(), 1);
    assert!(done[0].data.get("came_to_nothing").is_none() && done[0].data.get("effect").is_some(), "{:?}", done[0]);
    assert!(!events(&s.sim, "made").is_empty());

    // A making that fails: the deed still counts, as having come to nothing.
    let id = ask(&mut s, "carve a whistle");
    s.sim.on_interpreted(id, answer("bone whistle"));
    let b = built(&mut s);
    s.sim.on_type_built(b, None);
    assert!(s.sim.work().is_empty());
    let done = events(&s.sim, "interpreted");
    assert_eq!(done.len(), 2);
    assert_eq!(done[1].data["came_to_nothing"], true);
}

/// What a deed changed that the eye may miss gets a short line under its
/// story; a deed that changes nothing hidden gets none.
#[test]
fn a_deed_tells_its_hidden_effects() {
    use super::{Note, Request};
    let w = world("effects", 42);
    let stick = builtin_id(&w, "stick");
    let mut s = session(&w, 9, None);
    s.sim.has_llm = true;
    let p = s.sim.player.pos;
    let id = s.sim.spawn_thing(stick, p + Vec3::new(0.5, 0.0, 0.5), 0.0, 1.0, Default::default(), true).unwrap();
    let deed = |s: &mut Session, text: &str, answer: serde_json::Value| -> Vec<Note> {
        s.sim.drain_notes();
        s.sim.act(ActorId::Player, Action::Do { text: text.into(), on: Some(Target::Thing(id)), at: None }).unwrap();
        let rid = s.sim.drain_requests().into_iter().find_map(|r| if let Request::Interpret { id, .. } = r { Some(id) } else { None }).unwrap();
        s.sim.on_interpreted(rid, Ok(answer));
        s.sim.drain_notes()
    };
    let notes = deed(&mut s, "warm the stick", serde_json::json!({ "narration": "It warms.", "changes": [{ "target": "target", "props": { "heat": 80 } }] }));
    let effect: Vec<&String> = notes.iter().filter_map(|n| if let Note::Effect(e) = n { Some(e) } else { None }).collect();
    assert_eq!(effect, vec!["the stick: heat ↑"], "{notes:?}");
    let notes = deed(&mut s, "look at the stick", serde_json::json!({ "narration": "It is a stick.", "cache": false }));
    assert!(!notes.iter().any(|n| matches!(n, Note::Effect(_))), "{notes:?}");
}

#[test]
fn big_things_are_in_reach_from_beside_their_wall() {
    let w = world("reach", 41);
    let barn = add_type(&w, r#"
export const meta = { name: "barn", bounds: [4.0, 3.0, 3.0], tags: ["building"] };
export function sdf(x, y, z, k) { return roundBox(x, y - 1.5, z, 3.9, 1.5, 2.9, 0.05); }
export function color(x, y, z, k) { return rgb(150, 60, 40); }
"#);
    let at = dry_spot(&w, 20.0, 0.4);
    let inst = place(&w, barn, at, 0.7);
    let stone = builtin_id(&w, "stone");
    let mut s = session(&w, 5, None);
    let ty = s.sim.snap.type_of(barn).unwrap().clone();
    let gi = s.sim.snap.instances.iter().find(|p| p.id == inst).unwrap().gpu(&ty, 1.0);
    // Local -z is a long wall, 3 m from the middle; stand 1.5 m off it, then 4 m off it.
    let stand = |s: &mut Session, off: f32| {
        let p = gi.from_local(Vec3::new(0.5, 0.0, -3.0 - off));
        s.sim.player.pos = ground(&w, p.x, p.z);
    };
    stand(&mut s, 1.5);
    let p = s.sim.player.pos;
    assert!((p - gi.pos()).length() > 4.0, "the middle is out of reach");
    let id = s.sim.spawn_thing(stone, p + Vec3::new(0.2, 0.0, 0.2), 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.act(ActorId::Player, Action::Hold { target: Target::Thing(id) }).unwrap();
    let use_on = Action::Use { target: None, on: Some(Target::Instance(inst)), at: None };
    match s.sim.act(ActorId::Player, use_on.clone()) {
        Err(super::ActErr::TooFar { dist, .. }) => panic!("beside the wall but too far ({dist:.1} m)"),
        _ => {}
    }
    stand(&mut s, 4.0);
    match s.sim.act(ActorId::Player, use_on) {
        Err(super::ActErr::TooFar { dist, .. }) => assert!((dist - 4.0).abs() < 0.6, "measured to the wall: {dist}"),
        r => panic!("4 m off the wall is out of reach: {r:?}"),
    }
}

#[test]
fn a_house_out_of_nowhere_frightens_the_timid_and_the_next_one_less() {
    use super::surprise::{Arrival, News, Sight};
    let w = world("surprise", 7);
    let home = dry_spot(&w, 12.0, 0.4);
    add_char(&w, "Pell", "timid and nervous", &[], home);
    let mut s = session(&w, 7, None);
    let cid = s.sim.cast.npcs[0].def.id;
    let pos = s.sim.cast.npcs[0].a.pos;
    s.sim.cast.get_mut(cid).unwrap().traits.brave = 0.2;
    let house = Sight { how: Arrival::FromNowhere, extent: 8.0, strange: 0.0 };
    let at = pos + Vec3::new(6.0, 0.0, 0.0);
    let news = |memory: &'static str| News { at, sight: house, memory, importance: 0.5, event: "new_building", tell: "A house just appeared next to you.", what: "the house" };
    s.sim.out.clear();
    s.sim.startle_one(cid, &news("A house appeared east of me."));
    let felt = |s: &Session| -> Vec<(String, f32)> { s.sim.out.iter().filter_map(|r| if let super::Request::Witness { text, importance, .. } = r { Some((text.clone(), *importance)) } else { None }).collect() };
    let first = felt(&s);
    assert_eq!(first.len(), 1);
    assert!(first[0].0.contains("frightened"), "{first:?}");
    assert!(first[0].1 > 0.95);
    assert!(s.sim.cast.get(cid).unwrap().doing.starts_with("fleeing"), "{}", s.sim.cast.get(cid).unwrap().doing);
    // The same news again is not news; another house is, but less of a shock.
    s.sim.out.clear();
    s.sim.startle_one(cid, &news("A house appeared east of me."));
    assert!(felt(&s).is_empty());
    s.sim.startle_one(cid, &news("Another house appeared east of me."));
    let second = felt(&s);
    assert!(second[0].1 < first[0].1, "{second:?}");
}

/// A camp dog scares the horses at first, but scares that come to nothing
/// wear off: they get used to it and stop bolting. One telling per herd.
#[test]
fn herds_get_used_to_a_dog_that_never_hunts_them() {
    let w = world("camp-dog", 61);
    let a = dry_spot(&w, 10.0, 0.6);
    let bram = add_being(&w, "Bram", "horse", &[], a);
    let nell = add_being(&w, "Nell", "horse", &[], a + Vec3::new(2.0, 0.0, 0.0));
    let dog = add_being(&w, "Bankhar", "dog", &[], a + Vec3::new(0.0, 0.0, 4.0));
    let mut s = session(&w, 5, None);
    calm(&mut s);
    let (b, d) = (ActorId::Npc(bram), ActorId::Npc(dog));
    for id in [bram, nell] {
        let n = s.sim.cast.get_mut(id).unwrap();
        n.temper.wary = 0.7;
    }
    assert!(!s.sim.hunts(d, b), "the dog doesn't hunt horses");
    assert_eq!(s.sim.threat(b).map(|t| t.0), Some(d), "at first the dog scares the horse");
    s.sim.player.pos = a + Vec3::new(5.0, 0.0, 0.0);
    s.sim.drain_notes();
    let at = s.sim.actor(d).unwrap().pos;
    s.sim.flee(bram, d, at);
    s.sim.flee(nell, d, at);
    let told = s.sim.drain_notes().iter().filter(|n| n.text().contains("bolt") || n.text().contains("runs")).count();
    assert_eq!(told, 1, "one line for the whole herd");
    for _ in 0..3 {
        s.sim.flee(bram, d, at);
    }
    // Put them back next to the dog: they no longer care.
    s.sim.cast.get_mut(dog).unwrap().a.pos = s.sim.actor(b).unwrap().pos + Vec3::new(0.0, 0.0, 4.0);
    assert!(s.sim.threat(b).is_none(), "used to the dog now");
    assert!(s.sim.threat(ActorId::Npc(nell)).is_none(), "the whole herd is");
}

/// What a character makes by a deed is theirs: the "made" event names them,
/// the thing knows its maker, and the world's record of creations lists it
/// under them. Made again, it counts up instead of adding a line.
#[test]
fn a_characters_creation_is_credited_and_counted() {
    use super::Request;
    let w = world("credit", 43);
    let stick = builtin_id(&w, "stick");
    let at = dry_spot(&w, 8.0, 0.3);
    let nell = add_char(&w, "Nell", "handy", &[], at);
    let mut s = session(&w, 9, None);
    s.sim.has_llm = true;
    calm(&mut s);
    let me = ActorId::Npc(nell);
    // A character's deed waits its turn for the model; its id is all we need.
    let id = s.sim.act(me, Action::Do { text: "whittle a flute".into(), on: None, at: None }).unwrap().pending.expect("asked");
    s.sim.on_interpreted(id, Ok(serde_json::json!({ "narration": "A flute takes shape.", "create": [{ "name": "reed flute", "description": "new" }] })));
    let b = s.sim.drain_requests().into_iter().find_map(|r| if let Request::BuildType { id, .. } = r { Some(id) } else { None }).unwrap();
    s.sim.on_type_built(b, Some(stick));
    let made = events(&s.sim, "made");
    assert_eq!(made.last().and_then(|e| e.actor), Some(me), "the made event names its maker: {made:?}");
    assert!(s.sim.things.live().any(|t| t.type_id == stick && t.origin.made_by.as_deref() == Some("Nell")), "the thing knows who made it");
    let rows = s.sim.db.with(crate::db::creations).unwrap();
    let r = rows.iter().find(|r| r.key == format!("type:{stick}")).expect("listed");
    assert_eq!((r.kind.as_str(), r.made_by.as_str(), r.count), ("made", "Nell", 1));
    s.sim.record_creation(stick, "made", Some(ActorId::Player), "", at);
    let rows = s.sim.db.with(crate::db::creations).unwrap();
    let r = rows.iter().find(|r| r.key == format!("type:{stick}")).unwrap();
    assert_eq!((r.made_by.as_str(), r.count), ("Nell", 2), "the first maker keeps the credit; it counts up");
}

// ------------------------------------------------------------ settlement things

/// A pastime need not bounce: an orc camp's skull, marked as a toy and lying
/// untouched where the region put it, is found and played with.
#[test]
fn an_untouched_toy_that_barely_bounces_is_played_with() {
    let w = world("toy", 41);
    let p = dry_spot(&w, 8.0, 0.4);
    let a = add_char(&w, "Grub", "a playful, rowdy orc", &["Snag: friend"], p);
    let b = add_char(&w, "Snag", "a lively, playful orc", &["Grub: friend"], p + Vec3::new(5.0, 0.0, 0.0));
    let skull = add_type(&w, &fixture("sims/skull.js"));
    let skull_id = place(&w, skull, p + Vec3::new(2.0, 0.0, 2.0), 0.0);
    let mut s = session(&w, 9, None);
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -15.0);
    for cid in [a, b] {
        let n = s.sim.cast.get_mut(cid).unwrap();
        n.needs = crate::world::characters::Needs { hunger: 0.0, fatigue: 0.0, social: 0.0, fun: 0.95, curiosity: 0.0 };
    }
    let all = record(&mut s);
    for _ in 0..(120.0 / 0.1) as usize {
        s.step(0.1);
        if s.sim.night() {
            s.sim.t += crate::render::sky::DAY_SECONDS * 0.5;
        }
        if !of(&all, "caught").is_empty() || !of(&all, "threw").is_empty() {
            break;
        }
    }
    assert!(s.sim.things.by_instance.contains_key(&skull_id), "the skull was picked up");
    assert!(!of(&all, "threw").is_empty() || !of(&all, "caught").is_empty(), "someone played with it: {:?}", all.lock().iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    sound(&s);
}

/// Near the traveler something gets made every minute or so: the clock
/// turns the idlest person to their trade (at once while nothing has been
/// made), and a making resets it.
#[test]
fn the_maker_clock_sends_someone_to_their_trade_each_minute() {
    let w = world("maker", 42);
    let p = dry_spot(&w, 8.0, 1.1);
    let ball = add_type(&w, &fixture("sims/ball.js"));
    place(&w, ball, p + Vec3::new(1.5, 0.0, 0.0), 0.0);
    add_char(&w, "Ada", "a steady cobbler", &[], p);
    add_char(&w, "Ben", "a quiet weaver", &[], p + Vec3::new(6.0, 0.0, 0.0));
    let asks = Arc::new(Mutex::new(Vec::<(f64, String)>::new()));
    let a2 = asks.clone();
    let clock = Arc::new(Mutex::new(0.0f64));
    let c2 = clock.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You decide what a character") {
            if user.contains("craft or daily work") {
                a2.lock().push((*c2.lock(), user.to_string()));
                return r#"{"goal": "make a ball", "steps": [{"do": "do", "text": "stitch a leather ball"}]}"#.into();
            }
            return r#"{"goal": "", "steps": []}"#.into();
        }
        if sys.contains("physics and common sense") {
            return r#"{"narration": "A ball takes shape.", "create": [{"name": "leather ball"}], "cache": false}"#.into();
        }
        r#"{"lines": []}"#.into()
    }));
    let mut s = session(&w, 10, Some(llm));
    s.sim.cfg.work_secs = 1e6;
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -12.0);
    for n in s.sim.cast.npcs.iter_mut() {
        n.needs = crate::world::characters::Needs { hunger: 0.0, fatigue: 0.0, social: 0.0, fun: 0.0, curiosity: 0.0 };
    }
    let start = s.sim.t;
    for _ in 0..(200.0 / 0.1) as usize {
        *clock.lock() = s.sim.t - start;
        s.step(0.1);
        if s.sim.night() {
            s.sim.t += crate::render::sky::DAY_SECONDS * 0.5;
        }
    }
    let asks = asks.lock().clone();
    let times: Vec<f64> = asks.iter().map(|a| a.0.round()).collect();
    assert!(asks.len() >= 2 && asks.len() <= 5, "about one turn to work a minute: {times:?}");
    assert!(times[0] < 5.0, "nothing made yet: the first comes at once: {times:?}");
    assert!(times.windows(2).all(|w| w[1] - w[0] >= 50.0), "and never sooner after a making: {times:?}");
    assert!(s.sim.cast.last_made > start, "something was made");
    sound(&s);
}

// ------------------------------------------------------------------ the dark

/// Game time just before dusk on day `d` (by `secs` seconds).
fn before_dusk(d: f64, secs: f64) -> f64 {
    crate::render::sky::DAY_SECONDS * (d + super::night::DARK_FROM as f64) - secs
}

fn before_dawn(d: f64, secs: f64) -> f64 {
    crate::render::sky::DAY_SECONDS * (d + 1.21) - secs
}

/// Run `secs` of game time (the sim takes at most a quarter second a step).
fn run_for(s: &mut Session, secs: f32) {
    for _ in 0..(secs / 0.25).ceil() as usize {
        s.step(0.25);
    }
}

fn the_dark(s: &Session) -> Vec<i64> {
    s.sim.cast.npcs.iter().filter(|n| !n.dead && n.species.touch.harms()).map(|n| n.def.id).collect()
}

/// On normal, something comes out of the dark at dusk, creeps up from
/// behind, touches the traveler (a charge and some corruption), and is gone
/// at dawn, when the charges are topped up with a bonus for the night.
#[test]
fn the_dark_comes_at_dusk_touches_you_and_leaves_at_dawn() {
    let w = world("dark", 7);
    let mut s = session(&w, 7, None);
    s.sim.cfg.difficulty = 2;
    s.sim.player.pos = w.spawn;
    s.sim.player.yaw = 0.0;
    s.sim.t = before_dusk(1.0, 2.0);
    let all = record(&mut s);
    for _ in 0..40 {
        s.step(0.1);
    }
    assert!(s.sim.dark(), "night has fallen");
    assert_eq!(s.sim.charges(), Some(24), "normal starts with 24");
    let dark = the_dark(&s);
    assert_eq!(dark.len(), 1, "one comes on normal: {:?}", of(&all, "dark_came"));
    let h = s.sim.cast.get(dark[0]).unwrap();
    assert!(h.here() && h.species.about(true) && !h.species.about(false));
    assert!(h.corruption() > 0.9, "the dark is corrupt");
    let d0 = (h.a.pos - s.sim.player.pos).length();
    assert!(d0 > 35.0 && !s.sim.player_sees(h.a.pos), "it comes out of sight, behind: {d0:.0} m");
    // The traveler stands still, looking away: it reaches them.
    let mut touched = false;
    for _ in 0..(120.0 / 0.1) as usize {
        s.step(0.1);
        if !of(&all, "touched").is_empty() {
            touched = true;
            break;
        }
    }
    assert!(touched, "it reached the traveler; it is {:?}", s.sim.cast.get(dark[0]).map(|n| (n.doing.clone(), (n.a.pos - s.sim.player.pos).length())));
    assert_eq!(s.sim.charges(), Some(23), "a touch takes a charge on normal");
    assert!(s.sim.night.corruption > 0.2, "and lets something in");
    // It draws back after.
    for _ in 0..30 {
        s.step(0.1);
    }
    assert_eq!(of(&all, "touched").len(), 1, "one touch, then it draws back");
    // Dawn: it is gone (out of sight), the night is got through.
    s.sim.player.yaw = std::f32::consts::PI;
    s.sim.t = before_dawn(1.0, 1.0);
    for _ in 0..(40.0 / 0.1) as usize {
        s.step(0.1);
    }
    assert!(!s.sim.dark());
    assert!(s.sim.cast.get(dark[0]).is_some_and(|n| n.away), "gone with the light");
    let got = of(&all, "survived_night");
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].data["untouched"], false);
    assert_eq!(s.sim.charges(), Some(26), "topped up to 24, and 2 for the night");
    // The next dusk it comes back (the same one), near the traveler again.
    s.sim.t = before_dusk(2.0, 1.0);
    for _ in 0..30 {
        s.step(0.1);
    }
    assert_eq!(the_dark(&s), dark, "the same one comes back");
    assert!(s.sim.cast.get(dark[0]).is_some_and(|n| n.here() && (n.a.pos - s.sim.player.pos).length() < 70.0));
    sound(&s);
}

/// Watched, the dark's being stands still however near it is; at the very
/// edge of the eye it still creeps closer.
#[test]
fn the_dark_freezes_when_watched_even_up_close() {
    let w = world("dark close", 9);
    let mut s = session(&w, 9, None);
    s.sim.cfg.difficulty = 2;
    s.sim.player.pos = w.spawn;
    s.sim.player.yaw = 0.0;
    s.sim.t = before_dusk(1.0, 2.0);
    for _ in 0..40 {
        s.step(0.1);
    }
    let dark = the_dark(&s);
    assert_eq!(dark.len(), 1);
    let put = |s: &mut Session, p: Vec3| {
        let p = Vec3::new(p.x, s.sim.snap.terrain.height(p.x, p.z), p.z);
        let n = s.sim.cast.get_mut(dark[0]).unwrap();
        n.a.pos = p;
        n.think_at = 0.0;
    };
    let me = s.sim.player.pos;
    let f = s.sim.player.forward();
    let all = record(&mut s);
    put(&mut s, me + f * 2.5);
    let at = s.sim.cast.get(dark[0]).unwrap().a.pos;
    for _ in 0..30 {
        s.step(0.1);
    }
    let moved = (s.sim.cast.get(dark[0]).unwrap().a.pos - at).length();
    assert!(moved < 0.05, "looked at from 2.5 m it stays put: moved {moved:.2} m");
    assert!(of(&all, "touched").is_empty(), "out of arm's reach: no touch");
    // Within arm's reach looking doesn't save you.
    put(&mut s, me + f * 1.2);
    s.step(0.1);
    assert_eq!(of(&all, "touched").len(), 1, "within arm's reach it touches you, watched or not");
    if let Some(n) = s.sim.cast.get_mut(dark[0]) {
        n.touched_at = -1e9;
    }
    // At the edge of the eye (95% of the way out), it comes on.
    let r = s.sim.player.right();
    let slope = 1.0;
    put(&mut s, me + f * 10.0 + r * 10.0 * slope * 0.95);
    let from = (s.sim.cast.get(dark[0]).unwrap().a.pos - me).length();
    for _ in 0..30 {
        s.step(0.1);
    }
    let to = (s.sim.cast.get(dark[0]).unwrap().a.pos - me).length();
    assert!(to < from - 0.3, "at the edge of the eye it creeps closer: {from:.1} → {to:.1} m");
}

/// Every night horror is a "night walker" at the one pace; signs never say
/// where, and what the game says of where is true.
#[test]
fn night_walkers_are_named_paced_and_placed_truly() {
    let h = super::night::fallback_horror();
    assert_eq!(h.name, "night walker");
    assert!(h.signs.iter().all(|l| !l.to_lowercase().contains("behind")), "stock signs don't say where");
    let mut odd = h.clone();
    odd.moves.walk = 0.4;
    odd.moves.run = 9.0;
    odd.moves.fly = 6.0;
    super::night::fit_horror(&mut odd);
    assert_eq!((odd.moves.walk, odd.moves.run, odd.moves.fly), (h.moves.walk, h.moves.run, 0.0), "one pace for all");
    let w = world("walkers", 21);
    let mut s = session(&w, 21, None);
    s.sim.cfg.difficulty = 2;
    s.sim.player.pos = w.spawn;
    s.sim.player.yaw = 0.0;
    let me = s.sim.player.pos;
    let (f, r) = (s.sim.player.forward(), s.sim.player.right());
    assert_eq!(s.sim.where_is(me - f * 30.0), "behind you");
    assert_eq!(s.sim.where_is(me - f * 5.0), "close behind you");
    assert_eq!(s.sim.where_is(me + r * 30.0), "off to your right");
    assert_eq!(s.sim.where_is(me - r * 30.0), "off to your left");
    assert_eq!(s.sim.where_is(me + f * 30.0), "ahead of you, in the dark");
    // It comes at dusk as the night walker.
    s.sim.t = before_dusk(1.0, 2.0);
    for _ in 0..40 {
        s.step(0.1);
    }
    let dark = the_dark(&s);
    assert_eq!(dark.len(), 1);
    let n = s.sim.cast.get(dark[0]).unwrap();
    assert_eq!(n.species.name, "night walker");
    assert_eq!(s.sim.actor_name(ActorId::Npc(dark[0])), "the night walker");
}

/// Gone with the light stays gone: reopened by day, the dark's being isn't
/// back, and nothing says it left again.
#[test]
fn the_dark_stays_away_by_day_after_reopening() {
    let w = world("dark away", 13);
    let h = {
        let mut s = session(&w, 13, None);
        s.sim.cfg.difficulty = 3;
        s.sim.player.pos = w.spawn;
        s.sim.t = before_dusk(1.0, 1.0);
        run_for(&mut s, 3.0);
        let h = the_dark(&s)[0];
        s.sim.t = before_dawn(1.0, 1.0);
        run_for(&mut s, 3.0);
        assert!(s.sim.cast.get(h).is_some_and(|n| n.away), "gone at dawn");
        s.sim.t = crate::render::sky::DAY_SECONDS * 2.45;
        s.save();
        h
    };
    let mut s = session(&w, 13, None);
    assert!(!s.sim.dark());
    s.step(0.1);
    assert!(s.sim.cast.get(h).is_some_and(|n| n.away && !n.here()), "still away after reopening");
    let told: Vec<String> = s.sim.drain_notes().iter().map(|n| n.text()).filter(|t| t.contains("gone with the light")).collect();
    assert!(told.is_empty(), "nothing tells of it leaving again: {told:?}");
}

/// Peaceful: nothing comes, charges are unlimited, nothing twists.
#[test]
fn peaceful_nights_are_quiet_and_creation_is_free() {
    let w = world("peace", 8);
    let mut s = session(&w, 8, None);
    assert_eq!(s.sim.cfg.difficulty, 0, "peaceful by default");
    s.sim.t = before_dusk(1.0, 1.0);
    for _ in 0..(60.0 / 0.1) as usize {
        s.step(0.1);
    }
    assert!(s.sim.dark());
    assert!(the_dark(&s).is_empty() && events(&s.sim, "dark_came").is_empty());
    assert_eq!(s.sim.charges(), None);
    assert!((0..100).all(|_| s.sim.spend_charge()));
    assert_eq!(s.sim.night_status(), vec!["✦ ∞".to_string()]);
    // Someone corrupted (a world made harder, then easier again) doesn't twist here.
    let id = add_char(&w, "Ola", "kind", &[], w.spawn + Vec3::new(4.0, 0.0, 0.0));
    let snap = s.sim.snap.clone();
    let _ = (id, snap);
    assert!(s.sim.twist_line(ActorId::Player).is_none());
}

/// The dark's kind is written slowly (or never comes back): the night
/// doesn't wait forever, the stock horror comes, and the written kind joins
/// later. A world past its first night asks for it at once.
#[test]
fn a_slow_writing_doesnt_keep_the_dark_away() {
    use super::Request;
    let w = world("slow dark", 12);
    let mut s = session(&w, 12, None);
    s.sim.has_llm = true;
    s.sim.cfg.difficulty = 3;
    s.sim.player.pos = w.spawn;
    s.sim.player.yaw = 0.0;
    s.sim.night.nights = 2;
    // Step the sim alone: nothing answers its requests.
    let run = |s: &mut Session, secs: f32| (0..(secs / 0.25).ceil() as usize).for_each(|_| s.sim.step(0.25));
    s.sim.t = crate::render::sky::DAY_SECONDS * 2.4;
    run(&mut s, 1.1);
    let asked = |s: &mut Session| s.sim.drain_requests().into_iter().filter(|r| matches!(r, Request::NewSpecies { .. })).count();
    assert_eq!(asked(&mut s), 1, "asked for by day, not at sunset");
    s.sim.t = before_dusk(2.0, 1.0);
    run(&mut s, 3.0);
    assert!(s.sim.dark() && s.sim.night.hunting);
    assert!(the_dark(&s).is_empty(), "waits a while for the writing");
    run(&mut s, 60.0);
    assert_eq!(the_dark(&s).len(), 1, "then the stock horror comes");
    assert_eq!(asked(&mut s), 0, "and the writing isn't asked for twice");
}

/// Charges run out, deeds stop, and dawn brings them back; extra is kept.
#[test]
fn charges_run_out_and_dawn_tops_them_up() {
    let w = world("charges", 9);
    let mut s = session(&w, 9, None);
    s.sim.cfg.difficulty = 3;
    s.sim.t = crate::render::sky::DAY_SECONDS * 1.5;
    run_for(&mut s, 1.1);
    assert_eq!(s.sim.charges(), Some(12));
    for _ in 0..12 {
        assert!(s.sim.spend_charge());
    }
    assert!(!s.sim.spend_charge(), "spent");
    assert_eq!(s.sim.charges(), Some(0));
    // Dawn tops up to the start.
    s.sim.t = before_dawn(1.0, 0.5);
    run_for(&mut s, 0.1);
    s.sim.night.was_night = Some(true);
    s.sim.t = before_dawn(1.0, -1.0);
    run_for(&mut s, 1.1);
    assert_eq!(s.sim.charges(), Some(12));
    // More than the start is kept at dawn.
    s.sim.gain_charges(5, "test");
    s.sim.night.was_night = Some(true);
    run_for(&mut s, 1.1);
    assert_eq!(s.sim.charges(), Some(17));
    // Harder or easier keeps the same share: from hard (12) to normal (24) adds 12.
    s.sim.cfg.difficulty = 2;
    run_for(&mut s, 1.1);
    assert_eq!(s.sim.charges(), Some(29));
}

/// It moves only while unwatched, and waits at the edge of a light.
#[test]
fn watched_it_stands_still_and_light_keeps_it_at_bay() {
    let w = world("watched", 10);
    let lantern = add_type(&w, &fixture("sims/lantern.js"));
    let mut s = session(&w, 10, None);
    s.sim.cfg.difficulty = 3;
    s.sim.player.pos = w.spawn;
    s.sim.player.yaw = 0.0;
    s.sim.t = before_dusk(1.0, 1.0);
    for _ in 0..30 {
        s.step(0.1);
    }
    let h = the_dark(&s)[0];
    // Put it in front of the traveler, 20 m away, watched.
    let front = s.sim.player.pos + s.sim.player.forward() * 20.0;
    let front = ground(&w, front.x, front.z);
    s.sim.cast.get_mut(h).unwrap().a.pos = front;
    for _ in 0..50 {
        s.step(0.1);
    }
    let moved = (s.sim.cast.get(h).unwrap().a.pos - front).length();
    assert!(moved < 0.2, "watched, it doesn't move ({moved:.2} m)");
    // A lantern at the traveler's feet: turned away, it comes only to the light's edge.
    s.sim.spawn_thing(lantern, s.sim.player.pos + Vec3::Y * 0.3, 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.player.yaw = std::f32::consts::PI;
    let mut closest = f32::MAX;
    for _ in 0..(60.0 / 0.1) as usize {
        s.step(0.1);
        closest = closest.min((s.sim.cast.get(h).unwrap().a.pos - s.sim.player.pos).length());
    }
    assert!(closest < 15.0, "unwatched, it came closer ({closest:.1} m)");
    assert!(closest > 6.0, "but not into the light ({closest:.1} m)");
    assert!(events(&s.sim, "touched").is_empty(), "the light kept it off");
    assert!(s.sim.cast.get(h).unwrap().doing.contains("edge of the light"), "{}", s.sim.cast.get(h).unwrap().doing);
}

/// Corruption passes by touch (on hard), twists minds, and kindness from the
/// traveler draws it out: rules on the body's own `corruption` and a touch's
/// `kindness`, with no code that looks at what kind of touch it was.
#[test]
fn corruption_spreads_by_touch_twists_and_kindness_draws_it_out() {
    use super::body::{EMBRACE, HUG_SECS};
    let w = world("corrupt", 11);
    let a = add_char(&w, "Ada", "generous", &[], w.spawn + Vec3::new(3.0, 0.0, 0.0));
    let b = add_char(&w, "Bo", "curious", &[], w.spawn + Vec3::new(5.0, 0.0, 0.0));
    let c = add_char(&w, "Cy", "calm", &[], w.spawn + Vec3::new(7.0, 0.0, 0.0));
    let mut s = session(&w, 11, None);
    s.sim.cfg.difficulty = 3;
    s.sim.t = crate::render::sky::DAY_SECONDS * 1.45;
    run_for(&mut s, 1.1);
    let (ada, bo, cy) = (ActorId::Npc(a), ActorId::Npc(b), ActorId::Npc(c));
    s.sim.cast.get_mut(a).unwrap().props[P_CORRUPT] = 0.9;
    assert!(s.sim.twist_line(ada).is_some_and(|l| l.contains("never say")));
    assert!(s.sim.twist_line(bo).is_none());
    assert!(s.sim.decide_context(a, "x").contains("Something dark has a hold on you"));
    // Old saves' word for it is gone: events named like a hug do nothing.
    s.sim.event("gesture", Some(ActorId::Player), Some(ada.key()), "the traveler hugs Ada", None, serde_json::json!({ "kind": "hug" }));
    run_for(&mut s, 1.1);
    assert!(s.sim.corruption_of(ada) > 0.85);
    // Ada and Bo embrace, as a contact gesture does it.
    for _ in 0..8 {
        s.sim.touch_bodies(ada, bo, 0.5, 0.5, HUG_SECS, EMBRACE);
        run_for(&mut s, 1.1);
    }
    let cb = s.sim.corruption_of(bo);
    assert!(cb >= 0.3, "touch carried it to Bo ({cb:.2})");
    // On easy darkness doesn't pass at all.
    s.sim.cfg.difficulty = 1;
    s.sim.touch_bodies(ada, cy, 0.5, 0.5, HUG_SECS, EMBRACE);
    assert_eq!(s.sim.corruption_of(cy), 0.0, "easy: it doesn't pass");
    // Peaceful again: nothing twists, though the number stays.
    s.sim.cfg.difficulty = 0;
    assert!(s.sim.twist_line(ada).is_none());
    s.sim.cfg.difficulty = 3;
    // Kindness: hugs from the traveler.
    for _ in 0..12 {
        s.sim.touch_bodies(ActorId::Player, ada, 0.5, 0.0, HUG_SECS, EMBRACE);
        run_for(&mut s, 1.1);
    }
    assert!(s.sim.corruption_of(ada) < 0.1, "{}", s.sim.corruption_of(ada));
    assert_eq!(events(&s.sim, "cleansed").len(), 1, "the dark goes out of Ada");
    assert_eq!(s.sim.night.corruption, 0.0, "the traveler is never touched by the rules");
}

#[test]
fn species_hours_and_touch_read_and_default() {
    let old: crate::world::species::Species = serde_json::from_value(serde_json::json!({ "name": "owl", "body": "quadruped" })).unwrap();
    assert!(old.about(true) && old.about(false) && !old.touch.any() && old.want.is_empty());
    let mut moth: crate::world::species::Species = serde_json::from_value(serde_json::json!({
        "name": "Lamp Moth", "body": "moth", "active": "night", "want": "the traveler",
        "touch": { "glow": 3.0, "needs": { "fatigue": 0.3, "bogus": 1.0 } }, "shuns": [" Wet "]
    }))
    .unwrap();
    moth.sanitize();
    assert!(moth.about(true) && !moth.about(false));
    assert_eq!(moth.want, "traveler");
    assert_eq!(moth.touch.glow, 1.0);
    assert!(!moth.touch.harms() && moth.touch.any());
    assert_eq!(moth.touch.needs.len(), 1);
    assert_eq!(moth.shuns, vec!["wet".to_string()]);
    let j = serde_json::to_value(&old).unwrap();
    assert!(j.get("active").is_none() && j.get("touch").is_none(), "nothing new written for plain species");
    let h = super::night::fallback_horror();
    assert!(h.touch.harms() && h.moves_unseen && h.want == "traveler" && h.shuns == vec!["light".to_string()]);
    assert!(h.signs.len() >= 4, "it gives signs of itself");
}

/// Bodies are written, not fixed: cats on the generic quadruped get their
/// own body, written from it (at the height they had), in the background.
/// A deed makes one cat's tail poofy: that cat alone gets its own body,
/// written from the cat's, with every role it had. A collar made for the
/// quadruped still fits it, it is still a mate for a plain cat, and it all
/// stays after a restart.
#[test]
fn species_get_own_bodies_and_one_being_can_be_reshaped() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let w = world("own-bodies", 61);
    let cat = fixture("species/cat.js");
    let fluffy = cat.replace("const tr = 0.028 + clamp(k.b, 0, 1) * 0.05;", "const tr = 0.07 + clamp(k.b, 0, 1) * 0.04;").replace("from: \"quadruped\", ", "");
    assert_ne!(cat, fluffy);
    let from_template = Arc::new(AtomicBool::new(false));
    let kept_roles = Arc::new(AtomicBool::new(false));
    let (ft, kr) = (from_template.clone(), kept_roles.clone());
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |_sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if user.contains("Body type to write: \"cat\"") {
            ft.store(user.contains("An existing body, \"quadruped\"") && user.contains("export function sdf"), Ordering::SeqCst);
            return format!("```js\n{cat}\n```");
        }
        if user.contains("Reshape a body") {
            kr.store(user.contains("written from the body \"cat\"") && user.contains("spread") && user.contains("a big poofy tail"), Ordering::SeqCst);
            return format!("```js\n{fluffy}\n```");
        }
        if user.contains("\"make Tom's tail poofy\"") {
            return serde_json::json!({ "narration": "Tom's tail puffs up like a bottlebrush.", "being": { "reshape": "a big poofy tail, three times as thick" } }).to_string();
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let p = dry_spot(&w, 8.0, 0.9);
    let tom = add_being(&w, "Tom", "cat", &[], p);
    let mia = add_being(&w, "Mia", "cat", &[], p + Vec3::new(1.5, 0.0, 0.0));
    let mut s = session(&w, 31, Some(llm));
    calm(&mut s);
    let body_of = |s: &Session, c: i64| s.sim.snap.type_of(s.sim.cast.get(c).unwrap().body_ty).unwrap().name().to_string();
    assert_eq!(body_of(&s, tom), "quadruped", "a cat starts on the generic body");
    let h0 = s.sim.cast.get(tom).unwrap().a.dims.height;
    let roles0 = s.sim.cast.get(tom).unwrap().a.roles;
    for _ in 0..600 {
        s.step(0.05);
        if body_of(&s, tom) == "cat" && body_of(&s, mia) == "cat" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.sim.snap.species.get("cat").unwrap().body, "cat", "the cat species has its own body");
    assert_eq!(body_of(&s, tom), "cat");
    assert!(from_template.load(Ordering::SeqCst), "written from the quadruped, shown as an example");
    assert_eq!(s.sim.snap.body_line("cat"), vec!["cat".to_string(), "quadruped".to_string()]);
    let h1 = s.sim.cast.get(tom).unwrap().a.dims.height;
    assert!((h1 / h0 - 1.0).abs() < 0.1, "as tall as before: {h0} -> {h1}");
    assert_eq!(s.sim.cast.get(tom).unwrap().a.roles, roles0);
    // One cat's tail.
    let (me, t) = (ActorId::Player, ActorId::Npc(tom));
    s.sim.social.bond(t, me, 0.6, s.sim.t);
    s.sim.player.pos = s.sim.actor(t).unwrap().pos + Vec3::new(0.0, 0.0, -1.5);
    act_once(&mut s, me, Action::Do { text: "make Tom's tail poofy".into(), on: Some(Target::Actor(t)), at: None }, 0.3);
    for _ in 0..400 {
        s.step(0.05);
        if s.sim.interp.pending.is_empty() && s.sim.interp.building.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(kept_roles.load(Ordering::SeqCst), "the reshape keeps the cat's roles");
    let own = body_of(&s, tom);
    assert!(own.starts_with("cat (Tom"), "{own}");
    assert_eq!(s.sim.cast.get(tom).unwrap().def.persona.body, own);
    assert_eq!(body_of(&s, mia), "cat", "only Tom changed");
    assert_eq!(s.sim.snap.body_line(&own), vec![own.clone(), "cat".to_string(), "quadruped".to_string()]);
    assert_eq!(s.sim.cast.get(tom).unwrap().a.roles, roles0, "his gestures still work");
    assert!(s.sim.snap.layer_fits("quadruped", &own) && s.sim.snap.layer_fits("cat", &own) && !s.sim.snap.layer_fits(&own, "cat"));
    assert_eq!(s.sim.snap.body_root(&own), s.sim.snap.body_root("cat"), "still a mate for a plain cat");
    assert!(!events(&s.sim, "reshaped_being").is_empty());
    s.save();
    drop(s);
    let s = session(&w, 32, None);
    assert_eq!(body_of(&s, tom), own, "his own body after a restart");
    assert_eq!(body_of(&s, mia), "cat");
    sound(&s);
}

/// A goat grazes a grass tuft at its feet (grass has no `edible`, but it is
/// alive and small, and goats eat plants); a person can't eat grass.
#[test]
fn plant_eaters_graze_grass_and_people_do_not() {
    let w = world("graze", 52);
    let spot = dry_spot(&w, 14.0, 1.0);
    let goat = add_being(&w, "Nan", "goat", &[], spot);
    let ola = add_char(&w, "Ola", "a kind herder", &[], spot + Vec3::new(1.0, 0.0, 0.0));
    let grass = builtin_id(&w, "grass tuft");
    let tuft = place(&w, grass, spot + Vec3::new(0.5, 0.0, 0.0), 0.0);
    let mut s = session(&w, 7, None);
    calm(&mut s);
    let t1 = s.sim.liven(&Target::Instance(tuft)).unwrap();
    let r = s.sim.act(ActorId::Npc(ola), Action::Eat { target: Some(Target::Thing(t1)) });
    assert!(r.is_err(), "a person doesn't eat grass: {r:?}");
    s.sim.cast.get_mut(goat).unwrap().needs.hunger = 0.9;
    let r = s.sim.act(ActorId::Npc(goat), Action::Eat { target: Some(Target::Thing(t1)) });
    assert!(r.is_ok(), "the goat grazes: {r:?}");
    assert!(s.sim.things.get(t1).is_none(), "the tuft is eaten");
    assert!(s.sim.cast.get(goat).unwrap().needs.hunger < 0.9, "and the goat is less hungry");
}

// ------------------------------------------------------------------ sound

/// What happens is heard where it happens, as physics says: a dog's noise
/// in the dog's own voice, from the dog; a living shrub gives way and
/// rustles, never knocks; a body meeting stone is a dull, quiet thud with
/// no ring; fire crackles where it burns; a thrown stone where it lands.
/// The game's foley turns them into voices that sound.
#[test]
fn noises_bumps_and_impacts_are_heard_where_they_happen() {
    use crate::audio::Heard;
    let w = world("sound", 51);
    let p = dry_spot(&w, 10.0, 0.4);
    let rex = add_being(&w, "Rex", "dog", &[], p + Vec3::new(4.0, 0.0, -2.0));
    let shrub = builtin_id(&w, "shrub");
    place(&w, shrub, p + Vec3::new(0.0, 0.0, 2.6), 0.0);
    let boulder = builtin_id(&w, "boulder");
    place(&w, boulder, p + Vec3::new(2.0, 0.0, -3.6), 0.0);
    let mut s = session(&w, 5, None);
    calm(&mut s);
    clear_scatter(&mut s, p, 8.0);
    s.sim.player.pos = ground(&w, p.x, p.z);
    s.sim.player.yaw = 0.0;
    s.sim.sounds.clear();

    // The dog.
    s.sim.make_noise(rex, true);
    let c = s.sim.sounds.pop().expect("the noise is heard");
    assert_eq!(c.from, Some(ActorId::Npc(rex)));
    let dog = s.sim.actor(ActorId::Npc(rex)).unwrap().pos;
    assert!((c.at - dog).length() < 1.5, "from the dog");
    let Heard::Call { call, mass, .. } = &c.what else { panic!("a call: {c:?}") };
    let sp = s.sim.cast.get(rex).unwrap().species.clone();
    assert_eq!(*mass, sp.mass);
    assert!(sp.voice.iter().any(|v| v == call), "its own written voice: {call:?}");

    // Walking into the shrub, and leaning on it: growth gives way, no knock.
    for _ in 0..120 {
        s.sim.walk(ActorId::Player, Vec3::new(0.0, 0.0, 5.0 / 60.0));
        s.sim.step(1.0 / 60.0);
    }
    assert!(!s.sim.sounds.iter().any(|c| matches!(c.what, Heard::Hit { .. })), "a living shrub is not struck: {:?}", s.sim.sounds);
    // Walking along it: it rustles, softly, as the traveler's own brushing.
    let mut foley = crate::audio::foley::Foley::default();
    let mut cmds = Vec::new();
    let mut rustle: f32 = 0.0;
    for _ in 0..60 {
        s.sim.walk(ActorId::Player, Vec3::new(3.0 / 60.0, 0.0, 0.0));
        s.sim.step(1.0 / 60.0);
        foley.frame(&mut s.sim, 1.0 / 60.0, &mut cmds);
        for c in cmds.drain(..) {
            if let crate::audio::mix::Cmd::Texture { at: crate::audio::mix::At::Listener, drive, mat, roar, .. } = c {
                assert!(mat.leafy > 0.5 && mat.hard < 0.5 && roar == 0.0, "{mat:?}");
                rustle = rustle.max(drive);
            }
        }
    }
    assert!(rustle > 0.1, "brushing past it rustles: {rustle}");

    // A boulder walked into squarely: one dull, quiet thud, no ring.
    s.sim.player.pos = ground(&w, p.x + 2.0, p.z);
    s.sim.sounds.clear();
    cmds.clear();
    for _ in 0..90 {
        s.sim.walk(ActorId::Player, Vec3::new(0.0, 0.0, -4.0 / 60.0));
        s.sim.step(1.0 / 60.0);
    }
    let hits: Vec<_> = s.sim.sounds.iter().filter(|c| matches!(c.what, Heard::Hit { .. })).cloned().collect();
    assert_eq!(hits.len(), 1, "met once: {hits:?}");
    let Heard::Hit { mat, mass, by, .. } = hits[0].what else { unreachable!() };
    assert!(mat.hard > 0.6 && mass > 1000.0 && by.is_some(), "stone, heavy, met by a body: {:?}", hits[0]);
    s.sim.sounds.retain(|c| matches!(c.what, Heard::Hit { .. }));
    foley.frame(&mut s.sim, 1.0 / 60.0, &mut cmds);
    let thud = cmds.iter().find_map(|c| if let crate::audio::mix::Cmd::Play(p) = c { Some(p.call.0[0].clone()) } else { None }).expect("a thud");
    assert!(thud.ring < 0.05 && thud.hard < 0.2 && thud.loud < 0.3, "flesh on stone is a dull, quiet thud: {thud:?}");
    cmds.clear();

    // Fire crackles where it burns.
    let stick = builtin_id(&w, "stick");
    let fire_at = s.sim.player.pos + Vec3::new(4.0, 0.0, 4.0);
    let id = s.sim.spawn_thing(stick, fire_at, 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.things.get_mut(id).unwrap().props[P_FIRE] = 1.0;
    for _ in 0..20 {
        foley.frame(&mut s.sim, 1.0 / 60.0, &mut cmds);
    }
    let fire = cmds.iter().find_map(|c| if let crate::audio::mix::Cmd::Texture { at: crate::audio::mix::At::Point(p), drive, roar, .. } = c { (*roar > 0.0).then_some((*p, *drive)) } else { None }).expect("a fire is heard");
    assert!((fire.0 - fire_at).length() < 3.0 && fire.1 > 0.2, "{fire:?}");
    cmds.clear();
    s.sim.sounds.clear();

    // The foley makes voices of it all, and they sound.
    s.sim.make_noise(rex, false);
    foley.frame(&mut s.sim, 1.0 / 60.0, &mut cmds);
    assert!(s.sim.sounds.is_empty(), "taken");
    assert!(cmds.iter().any(|c| matches!(c, crate::audio::mix::Cmd::Play(_))));
    let _ = rex;
    let mut m = crate::audio::mix::Mixer::new(48000.0);
    for c in cmds {
        m.apply(c);
    }
    let e: f32 = m.render(1.5).iter().map(|x| x * x).sum();
    assert!(e > 0.01, "heard: {e}");

    // A thrown stone.
    let stone = builtin_id(&w, "stone");
    let id = s.sim.spawn_thing(stone, s.sim.player.pos + Vec3::new(0.3, 0.0, -0.3), 0.0, 1.0, Default::default(), true).unwrap();
    let r = act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Thing(id) }, 0.2);
    assert_eq!(r["ok"], true, "{r}");
    s.sim.sounds.clear();
    let far = s.sim.player.pos + Vec3::new(-6.0, 0.0, -6.0);
    let r = act_once(&mut s, ActorId::Player, Action::Throw { at: Some(Target::Point(far.to_array())), dir: None, force: Some(9.0) }, 4.0);
    assert_eq!(r["ok"], true, "{r}");
    let landed = s.sim.things.get(id).unwrap().pos;
    let hit = s.sim.sounds.iter().find_map(|c| if let Heard::Hit { mat, mass, .. } = c.what { Some((c.at, mat, mass)) } else { None }).expect("the stone is heard landing");
    assert!(hit.1.hard > 0.6 && hit.1.leafy < 0.5, "stone: {:?}", hit.1);
    assert!((hit.0 - landed).length() < 4.0, "where it landed");
}

/// A world with sound never changes what happens: the same seed makes the
/// same history whether or not anyone listens.
#[test]
fn listening_changes_nothing() {
    let run = |listen: bool| {
        let w = world("listen", 52);
        let p = dry_spot(&w, 10.0, 0.4);
        add_being(&w, "Rex", "dog", &[], p + Vec3::new(4.0, 0.0, -2.0));
        add_being(&w, "Tib", "cat", &[], p + Vec3::new(-4.0, 0.0, 2.0));
        let mut s = session(&w, 9, None);
        let all = record(&mut s);
        let mut foley = crate::audio::foley::Foley::default();
        let mut cmds = Vec::new();
        for _ in 0..600 {
            s.step(0.1);
            if listen {
                foley.frame(&mut s.sim, 0.1, &mut cmds);
                cmds.clear();
            }
        }
        let v: Vec<String> = all.lock().iter().map(|e| format!("{} {} {}", e.t, e.kind, e.text)).collect();
        v
    };
    assert_eq!(run(false), run(true));
}

// ------------------------------------------------------- vocabulary hygiene

/// A deed answer naming a property the world calls something else ("curse"
/// in a world of "cursed") is mapped by one ruling, kept with the world and
/// takes effect; the next deed reuses the alias without asking. A name
/// nothing maps to is sent back to be fixed, not dropped. No type, deed or
/// universe rule can set `force`, which only an action has.
#[test]
fn near_miss_property_names_are_mapped_once_and_act_properties_stay_apart() {
    let w = world("alias", 39);
    let (props, rules) = crate::brain::check_universe_rules(&[("cursed".into(), 0.0, "how cursed it is, 0..1".into())], &[]);
    super::persist::set_universe_rules(&w.db, &props, &rules).unwrap();
    let stick = builtin_id(&w, "stick");
    let rulings = Arc::new(Mutex::new(Vec::<String>::new()));
    let r2 = rulings.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("property names the world doesn't have") {
            r2.lock().push(user.to_string());
            return r#"{"curse": "cursed", "glimmer": null}"#.into();
        }
        if sys.contains("physics and common sense") {
            assert!(sys.contains("for scale:"), "values are written against anchors");
            assert!(!sys.contains("- force:"), "act properties are not offered");
            if user.contains("That failed validation") {
                assert!(user.contains("glimmer"), "{user}");
                return r#"{"narration": "It glows faintly.", "changes": [{"target": "target", "props": {"light": 0.5}}]}"#.into();
            }
            if user.contains("glimmer") {
                return r#"{"narration": "It glimmers.", "changes": [{"target": "target", "props": {"glimmer": 1}}]}"#.into();
            }
            return r#"{"narration": "A chill settles on it.", "changes": [{"target": "target", "props": {"curse": 1, "force": 1}}]}"#.into();
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let mut s = session(&w, 9, Some(llm));
    let p = s.sim.player.pos;
    let sticks: Vec<_> = (0..3).map(|k| s.sim.spawn_thing(stick, p + Vec3::new(0.5 + k as f32 * 0.3, 0.0, 0.6), 0.0, 1.0, Default::default(), true).unwrap()).collect();
    let ci = s.sim.vocab.id("cursed").unwrap();
    let r = act_once(&mut s, ActorId::Player, Action::Do { text: "hex the stick".into(), on: Some(Target::Thing(sticks[0])), at: None }, 1.0);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(s.sim.things.get(sticks[0]).unwrap().props[ci], 1.0, "the curse took effect");
    assert_eq!(s.sim.things.get(sticks[0]).unwrap().props[P_FORCE], 0.0, "a deed can't set force");
    assert_eq!(rulings.lock().len(), 1);
    assert_eq!(super::persist::prop_aliases(&w.db).get("curse").map(String::as_str), Some("cursed"), "kept with the world");
    act_once(&mut s, ActorId::Player, Action::Do { text: "lay a curse on the stick".into(), on: Some(Target::Thing(sticks[1])), at: None }, 1.0);
    assert_eq!(s.sim.things.get(sticks[1]).unwrap().props[ci], 1.0);
    assert_eq!(rulings.lock().len(), 1, "the alias is reused without asking again");
    act_once(&mut s, ActorId::Player, Action::Do { text: "make the stick glimmer".into(), on: Some(Target::Thing(sticks[2])), at: None }, 1.0);
    assert_eq!(rulings.lock().len(), 2);
    assert!(rulings.lock()[1].contains("glimmer") && !rulings.lock()[1].contains("curse"), "only new names are ruled on: {:?}", rulings.lock());
    assert_eq!(s.sim.things.get(sticks[2]).unwrap().props[P_LIGHT], 0.5, "sent back and fixed");
    assert_eq!(super::persist::prop_aliases(&w.db).get("glimmer").map(String::as_str), Some(""), "a name with no match is remembered too");
    // A reloaded world knows the alias.
    s.sim.load_universe_rules();
    assert_eq!(s.sim.vocab.lookup("Curse"), Some(ci));
    // Types can't give themselves force; nor can a universe's rules.
    let forced = compile(
        r#"
export const meta = { name: "ram", bounds: [0.2, 0.2, 0.6], tags: ["item"], props: { force: 1 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.15, 0.15, 0.5, 0.03); }
export function color(x, y, z, k) { return rgb(90, 60, 30); }
"#,
    )
    .unwrap();
    let e = crate::brain::check_type_props(&forced, &s.sim.vocab).unwrap_err();
    assert!(e.contains("force"), "{e}");
    let push = super::rules::RuleSpec { name: "curses push".into(), near: None, when: "self.cursed > 0".into(), effects: vec!["self.force = 1".into()] };
    assert!(crate::brain::check_universe_rules(&props, &[push]).1.is_empty());
    sound(&s);
}

// ------------------------------------------------------- bodies have properties

/// A world with a curse that spreads by touch: `cursed` with words for its
/// crossings and the given harm.
fn cursed_world(tag: &str, seed: u32, hazard: f32) -> W {
    use super::props::{Crossing, PropMeta};
    use super::rules::RuleSpec;
    let w = world(tag, seed);
    let rules = vec![RuleSpec { name: "curses spread by touch".into(), near: Some(1.5), when: "self.cursed > 0.5 && other.cursed < self.cursed".into(), effects: vec!["other.cursed += 0.1 * dt".into()] }];
    let (props, rules) = crate::brain::check_universe_rules(&[("cursed".into(), 0.0, "how cursed it is, 0..1".into())], &rules);
    super::persist::set_universe_rules(&w.db, &props, &rules).unwrap();
    let meta = PropMeta { rises: vec![Crossing { kind: "cursed_rose".into(), text: "fell under the curse".into(), when: None, saved: false }], hazard, keep: true, range: Some([0.0, 1.0]), ..Default::default() };
    super::persist::set_universe_meta(&w.db, &[("cursed".into(), meta)]).unwrap();
    w
}

/// A cursed idol passed hand to hand curses whoever holds it: the world's
/// own rule reaches a body through what it holds, and the crossing is told
/// by the holder's name. Nobody far off is touched.
#[test]
fn a_cursed_idol_passed_hand_to_hand_curses_whoever_holds_it() {
    let w = cursed_world("idol-hands", 51, 0.0);
    let idol = add_type(&w, r#"
export const meta = { name: "black idol", bounds: [0.1, 0.15, 0.1], tags: ["item"], props: { cursed: 1, mass: 1 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.08, 0.13, 0.08, 0.02); }
export function color(x, y, z, k) { return rgb(30, 20, 40); }
"#);
    let p = dry_spot(&w, 8.0, 1.3);
    let a = add_char(&w, "Ada", "kind and steady", &["Bo: brother"], p);
    let b = add_char(&w, "Bo", "kind and steady", &["Ada: sister"], p + Vec3::new(6.0, 0.0, 0.0));
    let c = add_char(&w, "Cy", "quiet", &[], p + Vec3::new(-30.0, 0.0, 0.0));
    let mut s = session(&w, 51, None);
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -12.0);
    clear_scatter(&mut s, p, 20.0);
    let all = record(&mut s);
    let ci = s.sim.vocab.id("cursed").unwrap();
    let curse = |s: &Session, id: i64| s.sim.cast.get(id).unwrap().props[ci];
    let id = s.sim.spawn_thing(idol, p + Vec3::new(0.3, 0.0, 0.0), 0.0, 1.0, Default::default(), true).unwrap();
    for n in s.sim.cast.npcs.iter_mut() {
        n.think_at = f64::MAX;
    }
    s.sim.hand_to(ActorId::Npc(a), id);
    s.run(12.0, 0.1);
    assert!(curse(&s, a) > 0.5, "holding it cursed Ada ({:.2})", curse(&s, a));
    let told: Vec<String> = of(&all, "cursed_rose").into_iter().filter(|e| e.subject.as_deref() == Some(ActorId::Npc(a).key().as_str())).map(|e| e.text).collect();
    assert!(told.iter().any(|t| t.starts_with("Ada fell under the curse")), "told by her name: {told:?}");
    let r = act_once(&mut s, ActorId::Npc(a), Action::Give { to: Target::Actor(ActorId::Npc(b)) }, 20.0);
    assert_eq!(s.sim.actor(ActorId::Npc(b)).unwrap().held, Some(id), "Bo took it: {r}");
    s.run(12.0, 0.1);
    assert!(curse(&s, b) > 0.5, "and now Bo is cursed ({:.2})", curse(&s, b));
    assert!(curse(&s, c) < 0.01, "Cy, far off, is not");
    // Kept across a restart, as the difference from a plain body.
    s.save();
    drop(s);
    let s = session(&w, 51, None);
    assert!(s.sim.cast.get(b).unwrap().props[ci] > 0.5, "still cursed after a restart");
    sound(&s);
}

/// A villager takes off a cursed cloak: what is worn touches the body, the
/// curse's harm (its metadata, no code for curses) makes it hurt, and they
/// get it off. Their planner hears what is on their body.
#[test]
fn a_villager_takes_off_a_cursed_cloak_because_it_harms() {
    let w = cursed_world("cloak", 52, 0.5);
    let cloak = add_type(&w, r#"
export const meta = { name: "grey cloak", bounds: [0.4, 0.6, 0.2], tags: ["layer"], props: { cursed: 1, mass: 2 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.35, 0.5, 0.15, 0.05); }
export function color(x, y, z, k) { return rgb(120, 120, 130); }
"#);
    let p = dry_spot(&w, 8.0, 2.1);
    let a = add_char(&w, "Oda", "practical", &[], p);
    let mut s = session(&w, 52, None);
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -12.0);
    let all = record(&mut s);
    let id = s.sim.spawn_thing(cloak, p, 0.0, 1.0, Default::default(), true).unwrap();
    let me = ActorId::Npc(a);
    s.sim.wear(me, me, id).unwrap();
    // Busy for a few seconds before she notices.
    s.sim.cast.get_mut(a).unwrap().think_at = s.sim.t + 4.0;
    let ci = s.sim.vocab.id("cursed").unwrap();
    let mut line = None;
    for _ in 0..200 {
        s.step(0.1);
        if line.is_none() && s.sim.cast.get(a).unwrap().pain > 0.05 {
            line = Some(s.sim.decide_context(a, "x"));
        }
        if !s.sim.worn_by(me).contains(&id) {
            break;
        }
    }
    assert!(!s.sim.worn_by(me).contains(&id), "Oda took the cloak off: {:?}", of(&all, "took_off").iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    assert!(!of(&all, "took_off").is_empty());
    assert!(s.sim.cast.get(a).unwrap().props[ci] > 0.0, "it touched her while she wore it");
    let line = line.expect("it hurt");
    assert!(line.contains("On your own body: cursed") && line.contains("it hurts"), "{line}");
    sound(&s);
}

/// A real hug on hard carries darkness from one body to the other; the
/// traveler's gift draws some out and earns a charge. Nothing reads event names.
#[test]
fn a_hug_carries_darkness_on_hard_and_a_gift_eases_it() {
    let w = world("hug-dark", 53);
    let p = dry_spot(&w, 8.0, 0.4);
    let a = add_char(&w, "Ada", "warm and affectionate", &["Bo: husband"], p);
    let b = add_char(&w, "Bo", "warm and affectionate", &["Ada: wife"], p + Vec3::new(1.5, 0.0, 0.0));
    let stick = builtin_id(&w, "stick");
    let mut s = session(&w, 53, None);
    s.sim.cfg.difficulty = 3;
    s.sim.t = crate::render::sky::DAY_SECONDS * 1.45;
    run_for(&mut s, 1.1);
    let (ada, bo) = (ActorId::Npc(a), ActorId::Npc(b));
    for (x, y) in [(ada, bo), (bo, ada)] {
        let r = s.sim.social.rel_mut(x, y);
        r.affection = 0.9;
        r.partner = true;
    }
    s.sim.cast.get_mut(a).unwrap().props[P_CORRUPT] = 0.8;
    let all = record(&mut s);
    s.sim.act(ada, Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(bo)) }).unwrap();
    for _ in 0..300 {
        s.step(0.1);
        if !of(&all, "hug").is_empty() {
            break;
        }
    }
    assert!(!of(&all, "hug").is_empty(), "they hugged");
    let cb = s.sim.corruption_of(bo);
    assert!(cb > 0.05, "the hug carried darkness to Bo ({cb:.3})");
    // The traveler hands Ada a stick: kindness eases her and earns a charge.
    s.sim.player.pos = s.sim.actor(ada).unwrap().pos + Vec3::new(0.0, 0.0, -1.0);
    let st = s.sim.spawn_thing(stick, s.sim.player.pos, 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.hand_to(ActorId::Player, st);
    s.sim.cast.get_mut(a).unwrap().a.held = None;
    let (ca, charges) = (s.sim.corruption_of(ada), s.sim.night.charges);
    s.sim.act(ActorId::Player, Action::Give { to: Target::Actor(ada) }).unwrap();
    assert!(s.sim.corruption_of(ada) < ca - 0.05, "the gift eased her ({ca:.2} → {:.2})", s.sim.corruption_of(ada));
    assert_eq!(s.sim.night.charges, charges + 1, "kindness earns a charge");
    sound(&s);
}

/// A kiln warms the people by it and kills nobody: heat reaches bodies by
/// the same rule as it reaches grass, and the plants' "killed by heat"
/// leaves bodies alone. Wading wets them.
#[test]
fn a_kiln_warms_the_people_by_it_and_kills_nobody() {
    let w = world("kiln", 54);
    let kiln = add_type(&w, r#"
export const meta = { name: "kiln", bounds: [0.8, 1.0, 0.8], tags: ["item"], props: { heat: 900, mass: 3000, burns: 0 } };
export function sdf(x, y, z, k) { return roundBox(x, y, z, 0.7, 0.9, 0.7, 0.1); }
export function color(x, y, z, k) { return rgb(150, 80, 50); }
"#);
    let p = dry_spot(&w, 10.0, 0.9);
    let ids: Vec<i64> = (0..3).map(|k| add_char(&w, &format!("Potter{k}"), "patient", &[], p + Vec3::new(1.4 * (k as f32 * 2.1).cos(), 0.0, 1.4 * (k as f32 * 2.1).sin()))).collect();
    let mut s = session(&w, 54, None);
    s.sim.player.pos = p + Vec3::new(0.0, 0.0, -14.0);
    clear_scatter(&mut s, p, 10.0);
    s.sim.spawn_thing(kiln, p, 0.0, 1.0, Default::default(), true).unwrap();
    for n in s.sim.cast.npcs.iter_mut() {
        n.think_at = f64::MAX;
    }
    let mut warmest: f32 = 0.0;
    for _ in 0..200 {
        s.step(0.1);
        for id in &ids {
            warmest = warmest.max(s.sim.cast.get(*id).unwrap().props[P_TEMP]);
        }
    }
    assert!(warmest > 45.0, "the kiln warmed someone ({warmest:.0}°)");
    for id in &ids {
        let n = s.sim.cast.get(*id).unwrap();
        assert!(!n.dead && n.props[P_ALIVE] > 0.5, "{} is alive", n.name());
    }
    // A body keeps itself warm away from the kiln, and gets wet in water.
    let o = s.sim.cast.get(ids[0]).unwrap().props.clone();
    assert!((o[P_HEAT] - 36.0).abs() < 1e-3 && o[P_BODY] == 1.0);
    let lake = (0..400).map(|k| w.spawn + Vec3::new((k as f32 * 0.7).cos(), 0.0, (k as f32 * 0.7).sin()) * (5.0 + k as f32 * 0.5)).find(|q| w.terrain.height(q.x, q.z) < WATER_LEVEL - 0.4);
    if let Some(q) = lake {
        s.sim.cast.get_mut(ids[1]).unwrap().a.pos = Vec3::new(q.x, w.terrain.height(q.x, q.z), q.z);
        s.sim.player.pos = q + Vec3::new(0.0, 0.0, -10.0);
        s.run(2.0, 0.1);
        assert!(s.sim.cast.get(ids[1]).unwrap().props[P_WET] > 0.9, "wading wets");
    }
    sound(&s);
}

// ------------------------------------------------------------ goals as data

/// Asked for a ball, a character promises (a goal with the traveler as
/// promisee, in their planner's context), makes it over time and hands it
/// over; the sim sees the condition met and closes the goal by itself.
#[test]
fn a_promised_ball_is_made_handed_over_and_the_goal_closes_itself() {
    let w = world("promise", 55);
    let spot = dry_spot(&w, 10.0, 0.4);
    let a = add_char(&w, "Tam", "a kind toymaker", &[], spot);
    let ball = fixture("sims/ball.js");
    let contexts = Arc::new(Mutex::new(Vec::<String>::new()));
    let c2 = contexts.clone();
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You decide what a character") {
            c2.lock().push(user.to_string());
            if user.contains("The traveler just said") {
                return r#"{"goal": "make the traveler a ball", "say": null, "promise": {"text": "make the traveler a ball", "to": "the traveler", "what": "a ball", "within_hours": 6}, "steps": [{"do": "create", "text": "a leather ball"}, {"do": "give", "to": "the traveler"}]}"#.into();
            }
            return r#"{"goal": "", "steps": []}"#.into();
        }
        if sys.contains("physics and common sense") && user.contains("ball") {
            return r#"{"narration": "", "make": [{"text": "a leather ball"}]}"#.into();
        }
        if user.contains("is making something in the world") {
            return format!("```json\n{{\"summary\": \"a leather ball\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": 1}}]}}\n```\n```js\n{ball}\n```");
        }
        r#"{"lines": []}"#.into()
    }));
    let mut s = session(&w, 55, Some(llm));
    s.sim.player.pos = spot + Vec3::new(0.0, 0.0, -3.0);
    let all = record(&mut s);
    s.sim.asked(a, "Could you make me a ball?", "Of course, I'll make you one.");
    let mut handed = false;
    for _ in 0..(150.0 / 0.1) as usize {
        s.step(0.1);
        handed |= s.sim.player.held.is_some();
        if handed && !of(&all, "goal_met").is_empty() {
            break;
        }
    }
    let log: Vec<String> = all.lock().iter().map(|e| format!("{:.0} {} {}", e.t, e.kind, e.text)).collect();
    let set = of(&all, "goal_set");
    assert!(set.iter().any(|e| e.data["crosses"] == true && e.text.contains("promised the traveler")), "a promise across two: {}", log.join("\n"));
    assert_eq!(set.len(), 1, "one promise, not two: {set:?}");
    assert!(handed, "handed over:\n{}", log.join("\n"));
    let met = of(&all, "goal_met");
    assert!(met.iter().any(|e| e.text.contains("kept their promise to the traveler")), "the goal closed itself:\n{}", log.join("\n"));
    let g = s.sim.goals.list.iter().find(|g| g.owner == a && g.source == super::goals::Source::Promise).unwrap();
    assert_eq!(g.status, super::goals::Status::Done);
    assert!(!g.steps.is_empty(), "what they did toward it is kept");
    // Kept across a restart.
    s.save();
    drop(s);
    let s = session(&w, 55, None);
    assert!(s.sim.goals.list.iter().any(|g| g.owner == a && g.status == super::goals::Status::Done), "kept with the world");
    sound(&s);
}

/// A promise not kept by its deadline is given up: told, remembered by the
/// one who made it, and the one it was made to trusts them less.
#[test]
fn a_promise_not_kept_by_its_deadline_is_dropped_remembered_and_costs_trust() {
    let w = world("broken", 56);
    let spot = dry_spot(&w, 10.0, 0.4);
    let a = add_char(&w, "Vel", "busy and forgetful", &[], spot);
    let llm = Llm::scripted(w.db.clone(), Arc::new(move |sys: &str, msgs: &[Msg]| {
        let user = msgs.last().map(|m| m.text.as_str()).unwrap_or("");
        if sys.contains("You decide what a character") && user.contains("The traveler just said") {
            return r#"{"goal": "", "say": null, "promise": {"text": "bring the traveler a lantern", "to": "the traveler", "what": "a lantern", "within_hours": 1}, "steps": []}"#.into();
        }
        r#"{"goal": "", "steps": []}"#.into()
    }));
    let mut s = session(&w, 56, Some(llm));
    s.sim.player.pos = spot + Vec3::new(0.0, 0.0, -3.0);
    let all = record(&mut s);
    let me = ActorId::Npc(a);
    let trust0 = s.sim.social.rel(ActorId::Player, me).map(|r| r.trust).unwrap_or(0.0);
    s.sim.asked(a, "Could you bring me a lantern later?", "Yes, later today.");
    s.run(2.0, 0.1);
    let ctx = s.sim.decide_context(a, "x");
    assert!(ctx.contains("Your goals: bring the traveler a lantern (promised to the traveler"), "{ctx}");
    s.run((super::headless::hour_s() * 1.3) as f32, 0.1);
    let dropped = of(&all, "goal_dropped");
    assert!(dropped.iter().any(|e| e.text.contains("broke their promise to the traveler")), "{:?}", all.lock().iter().map(|e| e.text.clone()).collect::<Vec<_>>());
    let trust1 = s.sim.social.rel(ActorId::Player, me).map(|r| r.trust).unwrap_or(0.0);
    assert!(trust1 < trust0 - 0.1, "it costs trust ({trust0:.2} → {trust1:.2})");
    s.pump(std::time::Duration::from_secs(2));
    let mems = w.db.memories(a).unwrap();
    assert!(mems.iter().any(|m| m.text.contains("didn't keep my promise")), "remembered: {:?}", mems.iter().map(|m| m.text.clone()).collect::<Vec<_>>());
    assert!(s.sim.decide_context(a, "x").contains("Lately: bring the traveler a lantern (given up)"));
    sound(&s);
}
