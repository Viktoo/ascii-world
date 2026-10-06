//! End-to-end: the real brain, committer, SQLite and (if present) GPU, driven
//! by a scripted LLM. Covers genesis, a region plan, a creation with a repair,
//! a rejected floating placement, undo, dialogue, and memory across a restart.

use crate::brain::{Brain, Cmd, Event};
use crate::db::Db;
use crate::llm::{Llm, Msg};
use crate::model::{Live, WorldModel};
use crate::world::describe::{NpcView, describe};
use crate::world::scatter::ScatterCache;
use crate::world::{WorldSnapshot, region_of};
use glam::Vec3;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fixture(p: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(p)).unwrap()
}

#[derive(Default)]
struct Calls {
    log: Vec<(String, String)>,
    create_attempts: usize,
    float_attempts: usize,
}

fn script(sys: &str, msgs: &[Msg], db: &Db, calls: &Mutex<Calls>) -> String {
    let user = msgs.last().map(|m| m.text.clone()).unwrap_or_default();
    let mut c = calls.lock();
    let entry = format!("{sys}\n---\n{user}");
    macro_rules! note {
        ($k:expr) => {
            c.log.push(($k.to_string(), entry.clone()))
        };
    }
    let lighthouse = fixture("good/lighthouse.js");
    if sys.contains("You are a character in Pocket Universe") {
        note!("dialogue");
        return if sys.contains("The traveler said") {
            "Ah, you came back. Did you find the keeper?".into()
        } else {
            "You look like you fell from the sky.".into()
        };
    }
    if sys.contains("You decide what a character") {
        note!("decide");
        return r#"{"action": "approach", "line": "You look lost, traveler."}"#.into();
    }
    if sys.contains("Update this character's private memory summary") {
        note!("summary");
        return "I met a traveler who fell from the sky.".into();
    }
    if user.contains("That failed validation") {
        note!("repair");
        return format!(
            "```json\n{{\"summary\": \"a lighthouse on the hill\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": 0, \"rot\": 0, \"scale\": 1.0}}]}}\n```\n```js\n{lighthouse}\n```"
        );
    }
    if user.contains("is making something in the world") {
        if user.contains("hovering") {
            c.float_attempts += 1;
            note!("create-float");
            return "```json\n{\"summary\": \"a hovering lighthouse\", \"reuse\": \"lighthouse\", \"placements\": [{\"right\": 0, \"forward\": 0, \"lift\": 6.0}]}\n```".into();
        }
        c.create_attempts += 1;
        note!("create");
        let broken = lighthouse.replace("return smoothUnion(tower, lamp, 0.3);", "return smoothUnion(tower, lamp, 0.3;");
        return format!("```json\n{{\"summary\": \"a lighthouse on the hill\", \"reuse\": null, \"placements\": [{{\"right\": 0, \"forward\": 0}}]}}\n```\n```js\n{broken}\n```");
    }
    if user.contains("Design the base layer") {
        note!("genesis");
        let look = r#"{"name": "Greywater Coast", "land": "a grey coast of fishing villages, pine headlands and tidal flats", "start": "the lighthouse keeper has vanished", "palette": {"sky_day": [120, 140, 160], "horizon_day": [190, 200, 205], "sky_dusk": [70, 70, 110], "horizon_dusk": [220, 150, 120], "sky_night": [8, 10, 20], "horizon_night": [25, 30, 45], "sun": [240, 235, 220], "water": [40, 70, 85], "rock": [110, 110, 105], "snow": [235, 238, 240], "sand": [190, 180, 150], "fog": 1.8},
          "biomes": [{"name": "dune grass", "base": 3, "amp": 6, "rough": 0.1, "ground": [140, 150, 100], "ground2": [180, 170, 130], "scatter": {"rock": 0.3, "grass": 2.5}},
                     {"name": "coastal pines", "base": 8, "amp": 16, "rough": 0.3, "ground": [70, 95, 60], "ground2": [90, 100, 70], "scatter": {"tree": 1.0, "rock": 0.2}},
                     {"name": "tidal flats", "base": -4, "amp": 5, "rough": 0.05, "ground": [120, 120, 100], "ground2": [150, 140, 110], "scatter": {"rock": 0.4}}]}"#;
        return format!("```json\n{look}\n```\n```js\n{}\n```\n```js\n{}\n```", fixture("mock/seapine.js"), fixture("mock/tiderock.js"));
    }
    if user.contains("Plan the story layer") {
        note!("region");
        let spawn: [f32; 3] = serde_json::from_str(&db.kv_get("spawn").unwrap()).unwrap();
        let caps: Vec<i32> = user.split("Region (").nth(1).unwrap().split(')').next().unwrap().split(", ").map(|v| v.parse().unwrap()).collect();
        let (lx, lz) = (spawn[0] - caps[0] as f32 * 256.0, spawn[2] - caps[1] as f32 * 256.0);
        let cl = |v: f32| v.clamp(10.0, 246.0);
        return format!(
            r#"```json
{{"name": "Pinewood Vale", "mood": "rain-soaked and watchful", "facts": ["The lighthouse keeper vanished three nights ago."],
 "new_types": [{{"name": "keeper's cottage", "description": "whitewashed cottage", "size_m": [6, 6, 5], "tags": ["building"]}}],
 "landmarks": [],
 "settlement": {{"name": "Greywater", "x": {sx}, "z": {sz}, "buildings": [{{"type": "keeper's cottage", "dx": 0, "dz": 0, "rot": 180}}, {{"type": "keeper's cottage", "dx": 14, "dz": 4, "rot": 200}}]}},
 "characters": [{{"name": "Mara", "age": 52, "appearance": "net-mender in a yellow oilskin", "look": {{"height": 1.68, "build": 1.1, "skin": 0.35, "shirt_hue": 0.14, "trousers_hue": 0.6}},
   "personality": "gruff but kind", "goals": "find the keeper", "voice": "short sentences", "home": "by the shore", "home_x": {hx}, "home_z": {hz}, "relationships": []}}]}}
```"#,
            sx = cl(lx + 30.0),
            sz = cl(lz + 30.0),
            hx = cl(lx + 2.0),
            hz = cl(lz + 6.0)
        );
    }
    if user.contains("Object type to write") {
        note!("type");
        return format!("```js\n{}\n```", fixture("mock/cottage.js"));
    }
    note!("unknown");
    "?".into()
}

fn wait_for(rx: &crossbeam_channel::Receiver<Event>, secs: u64, mut f: impl FnMut(&Event) -> bool, snap: &mut Arc<WorldSnapshot>, logs: &mut Vec<String>) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        let Ok(ev) = rx.recv_timeout(Duration::from_millis(100)) else { continue };
        match &ev {
            Event::Flip { snap: s, .. } => {
                s.check_consistent().expect("snapshot consistent");
                *snap = s.clone();
            }
            Event::Log(l) => logs.push(l.clone()),
            _ => {}
        }
        if f(&ev) {
            return true;
        }
    }
    false
}

struct Session {
    brain: Brain,
    rx: crossbeam_channel::Receiver<Event>,
    snap: Arc<WorldSnapshot>,
    db: Arc<Db>,
}

fn open(path: &std::path::Path, calls: &Arc<Mutex<Calls>>) -> Session {
    let db = Db::open(path).unwrap();
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(Live::default()));
    let mut model = WorldModel::load(db.clone(), gpu, live.clone()).unwrap();
    let snap = model.snapshot().unwrap();
    // Stand a little away from the spawn so nothing is built on top of us.
    live.lock().player = snap.spawn + Vec3::new(-30.0, 0.0, -30.0);
    let (db2, calls2) = (db.clone(), calls.clone());
    let llm = Llm::scripted(db.clone(), Arc::new(move |s: &str, m: &[Msg]| script(s, m, &db2, &calls2)));
    let (tx, rx) = crossbeam_channel::unbounded();
    let brain = Brain::start(model, Some(llm), tx, false);
    Session { brain, rx, snap, db }
}

#[test]
fn story_pipeline_end_to_end() {
    let dir = std::env::temp_dir().join(format!("pocket-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("e2e.pocket");
    Db::create(&path, 777, "a rainy coastal valley where the lighthouse keeper vanished", &serde_json::to_string(&crate::world::Look::default()).unwrap()).unwrap();
    let calls = Arc::new(Mutex::new(Calls::default()));
    let s = open(&path, &calls);
    let mut logs = Vec::new();
    let mut snap = s.snap.clone();

    // Genesis: palette, biomes, base types.
    s.brain.send(Cmd::Genesis);
    assert!(wait_for(&s.rx, 60, |e| matches!(e, Event::GenesisDone), &mut snap, &mut logs), "genesis timed out");
    assert_eq!(snap.look.name, "Greywater Coast");
    assert_eq!(snap.terrain.biomes.len(), 3);
    let names: Vec<String> = snap.scene.types.values().map(|t| t.name().to_string()).collect();
    assert!(names.contains(&"sea pine".to_string()) && names.contains(&"tide rock".to_string()), "{names:?}");
    let tree_ids = &snap.scatter["tree"];
    assert!(tree_ids.iter().all(|id| snap.type_of(*id).unwrap().name() == "sea pine"), "universe trees replace the built-ins");
    assert!(logs.iter().any(|l| l.contains("true shape")));
    assert_eq!(snap.look.start, "the lighthouse keeper has vanished");

    // A region plan for where we stand.
    let r = region_of(snap.spawn.x, snap.spawn.z);
    s.brain.send(Cmd::Region(r));
    let mut ok = false;
    assert!(wait_for(&s.rx, 60, |e| matches!(e, Event::RegionFinished(rr, o) if { ok = *o; *rr == r }), &mut snap, &mut logs));
    assert!(ok, "region failed: {logs:?}");
    assert_eq!(snap.region_name(r), Some("Pinewood Vale"));
    let cottages = snap.instances.iter().filter(|p| snap.type_of(p.type_id).unwrap().name() == "keeper's cottage").count();
    assert_eq!(cottages, 2, "both cottages placed");
    assert_eq!(snap.characters.len(), 1);
    assert_eq!(snap.characters[0].persona.name, "Mara");
    let mara = snap.characters[0].id;
    {
        let c = calls.lock();
        let plan = &c.log.iter().find(|(k, _)| k == "region").unwrap().1;
        assert!(plan.contains("The traveler begins in this region: the lighthouse keeper has vanished"), "the starting region gets the start");
        assert!(plan.contains("fishing villages") && !plan.contains("rainy coastal valley"), "regions are planned from the land, not the prompt");
    }

    // /a lighthouse: first attempt has a syntax error and is repaired.
    let mut cache = ScatterCache::default();
    let cam = crate::render::Camera { pos: snap.spawn + Vec3::Y * 1.7, yaw: 0.0, pitch: -0.06, fov_y: 1.05, roll: 0.0 };
    let view = describe(&snap, &mut cache, &Vec::<NpcView>::new(), &cam, 1.6, 400.0);
    let target = {
        let p = snap.spawn + Vec3::new(0.0, 0.0, -45.0);
        Vec3::new(p.x, snap.terrain.height(p.x, p.z), p.z)
    };
    let before = snap.instances.len();
    s.brain.send(Cmd::Create { id: None, by: None, text: "a lighthouse on that hill".into(), view: view.clone(), target, yaw: std::f32::consts::PI });
    assert!(wait_for(&s.rx, 60, |e| matches!(e, Event::Log(l) if l.starts_with("Built") || l.starts_with("Couldn't")), &mut snap, &mut logs), "create timed out");
    assert!(logs.iter().any(|l| l.contains("Built the lighthouse")), "{logs:?}");
    assert_eq!(snap.instances.len(), before + 1);
    {
        let c = calls.lock();
        let repair = c.log.iter().find(|(k, _)| k == "repair").expect("a repair call");
        assert!(repair.1.contains("syntax error"), "repair got the parse error: {}", repair.1);
    }
    let with_lighthouse = snap.version;

    // A floating placement is rejected at the placement step and repaired.
    s.brain.send(Cmd::Create { id: None, by: None, text: "a hovering lighthouse".into(), view: view.clone(), target: target + Vec3::new(40.0, 0.0, 0.0), yaw: std::f32::consts::PI });
    assert!(wait_for(&s.rx, 60, |e| matches!(e, Event::Log(l) if l.starts_with("Built") || l.starts_with("Couldn't")), &mut snap, &mut logs));
    {
        let c = calls.lock();
        let repairs: Vec<&(String, String)> = c.log.iter().filter(|(k, _)| k == "repair").collect();
        assert!(repairs.last().unwrap().1.contains("floats"), "placement error fed back: {}", repairs.last().unwrap().1);
    }
    assert_eq!(snap.instances.len(), before + 2);

    // Undo is instant (cached pipeline) and exact.
    let t0 = Instant::now();
    s.brain.send(Cmd::Undo);
    assert!(wait_for(&s.rx, 10, |e| matches!(e, Event::Flip { .. }), &mut snap, &mut logs));
    let undo_ms = t0.elapsed().as_millis();
    assert_eq!(snap.instances.len(), before + 1);
    assert!(undo_ms < 500, "undo took {undo_ms} ms");
    let _ = with_lighthouse;

    // Dialogue streams and is remembered.
    s.brain.send(Cmd::Talk { cid: mara, text: "Hello! What happened to the lighthouse keeper?".into(), context: "It is morning.".into(), history: vec![] });
    let mut reply = String::new();
    assert!(wait_for(&s.rx, 30, |e| {
        if let Event::Token { text, .. } = e {
            reply.push_str(text);
        }
        matches!(e, Event::ReplyDone { ok: true, .. })
    }, &mut snap, &mut logs));
    assert_eq!(reply, "You look like you fell from the sky.");
    let mems = s.db.memories(mara).unwrap();
    assert!(mems.iter().any(|m| m.text.contains("lighthouse keeper")));

    // Restart: a fresh process view of the same file. Mara remembers.
    drop(s);
    std::thread::sleep(Duration::from_millis(300));
    let s2 = open(&path, &calls);
    assert_eq!(s2.snap.instances.len(), before + 1, "reopened world matches");
    s2.brain.send(Cmd::Talk { cid: mara, text: "I'm back.".into(), context: "It is noon.".into(), history: vec![] });
    let mut reply = String::new();
    let mut snap2 = s2.snap.clone();
    assert!(wait_for(&s2.rx, 30, |e| {
        if let Event::Token { text, .. } = e {
            reply.push_str(text);
        }
        matches!(e, Event::ReplyDone { ok: true, .. })
    }, &mut snap2, &mut logs));
    assert!(reply.contains("you came back"), "Mara should remember: {reply}");
    let c = calls.lock();
    let last = c.log.iter().rev().find(|(k, _)| k == "dialogue").unwrap();
    assert!(last.1.contains("What happened to the lighthouse keeper"), "memory in prompt");
    assert!(last.1.contains("fishing villages") && !last.1.contains("rainy coastal valley"), "a reopened world still speaks from the land");
    drop(c);

    // Any other region is its own place in the land, without the start.
    let next = (r.0 + 1, r.1);
    s2.brain.send(Cmd::Region(next));
    assert!(wait_for(&s2.rx, 60, |e| matches!(e, Event::RegionFinished(rr, _) if *rr == next), &mut snap2, &mut logs));
    let c = calls.lock();
    let plan = &c.log.iter().rev().find(|(k, _)| k == "region").unwrap().1;
    assert!(plan.contains(&format!("Region ({}, {})", next.0, next.1)));
    assert!(plan.contains("did not begin here") && !plan.contains("keeper has vanished"), "only the first region gets the start");

    // Copying the file gives an identical world that diverges independently.
    drop(c);
    let copy = dir.join("copy.pocket");
    std::fs::copy(&path, &copy).unwrap();
    let a = Db::open(&path).unwrap();
    let b = Db::open(&copy).unwrap();
    assert_eq!(a.instances().unwrap().len(), b.instances().unwrap().len());
    assert_eq!(a.universe().unwrap().seed, b.universe().unwrap().seed);
    b.with(|c| Ok(crate::db::add_version(c, None, "create", "only in the copy", None)?)).unwrap();
    assert_ne!(a.versions().unwrap().len(), b.versions().unwrap().len());
}
