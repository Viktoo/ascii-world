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
        if sys.contains("You animate a simple figure") {
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
