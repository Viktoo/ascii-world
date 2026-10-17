//! Life in the water: things scattered by how much water they grow in
//! (`meta.grows`), and beings by how much they live in it (a species'
//! `water` and `depth`): a fish keeps to its water at its own depth, and
//! stranded it suffers and works its way back.

use super::*;

const KELP: &str = r#"export const meta = { name: "kelp", bounds: [0.4, 2.0, 0.4], tags: ["base", "kelp", "nonsolid"], props: { alive: 1, edible: 0.3 }, grows: { depth: [1.5, 30] } };
export function sdf(x, y, z, k) { return capsule(x, y, z, 0, 0, 0, 0, 3.8, 0, 0.08); }
export function color(x, y, z, k) { return rgb(60, 90, 40); }
"#;

const LILY: &str = r#"export const meta = { name: "water lily", bounds: [0.5, 0.3, 0.5], tags: ["base", "lily", "nonsolid"], props: { alive: 1 }, grows: { depth: [0.3, 3], at: "surface" } };
export function sdf(x, y, z, k) { return ellipsoid(x, y, z, 0.45, 0.04, 0.45); }
export function color(x, y, z, k) { return rgb(70, 130, 60); }
"#;

/// A low coast: half sea, half land, with grass, kelp and lilies to scatter.
fn coast() -> crate::world::Look {
    let mut b = crate::terrain::default_biomes()[0].clone();
    b.name = "low coast".into();
    b.base = -4.0;
    b.amp = 12.0;
    b.rough = 0.1;
    b.cliffs = 0.0;
    b.scatter = [("grass".to_string(), 2.0), ("kelp".to_string(), 1.0), ("lily".to_string(), 0.6)].into_iter().collect();
    crate::world::Look { name: "Coast".into(), biomes: vec![b], ..Default::default() }
}

#[test]
fn meta_grows_is_checked() {
    let ct = compile(LILY).unwrap();
    let g = ct.meta.grows.clone().expect("grows");
    assert_eq!(g.depth, [0.3, 3.0]);
    assert!(g.surface && g.fits(1.0) && !g.fits(-0.1));
    // Saying nothing: dry ground only.
    let dry = crate::lang::ir::Grows::DRY;
    assert!(dry.fits(-2.0) && !dry.fits(-0.2) && !dry.fits(1.0));
    for bad in ["grows: { depth: [3, 1] }", "grows: { depth: [1, 2], at: \"sky\" }", "grows: { at: \"bed\" }", "grows: { depth: [-2, -1], at: \"surface\" }"] {
        let src = KELP.replace("grows: { depth: [1.5, 30] }", bad);
        assert!(compile(&src).is_err(), "{bad} is refused");
    }
}

/// Each kind grows only where the water suits it: grass on dry ground, kelp
/// on the sea bed, lilies floating on the shallows.
#[test]
fn plants_grow_where_the_water_suits_them() {
    let w = world_with("grows", 12, coast());
    add_type(&w, KELP);
    add_type(&w, LILY);
    let mut s = session(&w, 12, None);
    let snap = s.sim.snap.clone();
    let t = snap.terrain.clone();
    let c0 = crate::world::chunk_of(w.spawn.x, w.spawn.z);
    let mut items = Vec::new();
    for dz in -4..=4 {
        for dx in -4..=4 {
            items.extend(s.sim.cache.get(&snap, (c0.0 + dx, c0.1 + dz)).iter().cloned());
        }
    }
    let of = |tag: &str| -> Vec<Vec3> { items.iter().filter(|it| snap.type_of(it.inst.info[0]).is_some_and(|ty| ty.has_tag(tag))).map(|it| it.inst.pos()).collect() };
    let wd = |p: &Vec3| t.water(p.x, p.z) - t.height(p.x, p.z);
    let (grass, kelp, lily) = (of("grass"), of("kelp"), of("lily"));
    eprintln!("{} grass, {} kelp, {} lilies", grass.len(), kelp.len(), lily.len());
    assert!(grass.len() > 100 && kelp.len() > 50 && lily.len() > 20, "all three grow");
    assert!(grass.iter().all(|p| wd(p) <= -0.35), "grass on dry ground");
    assert!(kelp.iter().all(|p| (1.5..=30.0).contains(&wd(p))), "kelp on the bed under 1.5–30 m of water");
    assert!(kelp.iter().all(|p| p.y < t.water(p.x, p.z) - 1.0), "kelp stands under the water");
    assert!(lily.iter().all(|p| (0.3..=3.0).contains(&wd(p))), "lilies on the shallows");
    assert!(lily.iter().all(|p| (p.y - t.water(p.x, p.z)).abs() < 0.2), "lilies float at the surface");
}

/// A deep spot near the spawn, and dry land beside the water there.
fn sea_and_shore(w: &W) -> (Vec3, Vec3) {
    let t = &w.terrain;
    let wd = |x: f32, z: f32| t.water(x, z) - t.height(x, z);
    let mut deep = None;
    for k in 0..20000 {
        let a = k as f32 * 2.399;
        let r = (k as f32).sqrt() * 2.0;
        let (x, z) = (w.spawn.x + a.cos() * r, w.spawn.z + a.sin() * r);
        // Deep, with deep water all round it.
        if wd(x, z) > 4.0 && (0..8).all(|i| { let b = i as f32 * 0.785; wd(x + b.cos() * 8.0, z + b.sin() * 8.0) > 3.0 }) {
            deep = Some(Vec3::new(x, t.height(x, z), z));
            break;
        }
    }
    let deep = deep.expect("deep water");
    // The nearest dry land from it, and a little further in.
    let mut best: Option<(f32, Vec3)> = None;
    for i in 0..16 {
        let a = i as f32 / 16.0 * std::f32::consts::TAU;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let mut d = 0.0;
        while d < 300.0 && wd(deep.x + dir.x * d, deep.z + dir.z * d) > -0.3 {
            d += 0.5;
        }
        if d < 300.0 && best.is_none_or(|b| d < b.0) {
            best = Some((d, deep + dir * (d + 1.0)));
        }
    }
    let p = best.expect("land near the deep water").1;
    (deep, Vec3::new(p.x, t.height(p.x, p.z), p.z))
}

fn put_carp(w: &W) {
    let carp = serde_json::json!({ "name": "carp", "plural": "carp", "body": "quadruped", "size": 0.6, "mind": "instinct", "speech": "none", "social": "solitary",
        "diet": { "plants": 0.7, "meat": 0.3 }, "move": { "walk": 0.5, "run": 1.0, "swim": 1.2 }, "water": 1, "depth": [1.0, 2.0], "description": "a big carp" });
    w.db.with(|c| db::put_species(c, "carp", &carp.to_string())).unwrap();
}

/// A fish swims at its own depth, never out of its water, even sent ashore.
#[test]
fn a_fish_keeps_to_its_water_at_its_depth() {
    let w = world_with("fish", 12, coast());
    put_carp(&w);
    let (deep, shore) = sea_and_shore(&w);
    let id = add_being(&w, "Old Whiskers", "carp", &[], deep);
    let mut s = session(&w, 12, None);
    calm(&mut s);
    let me = ActorId::Npc(id);
    let need = s.sim.capsule(me).needs_water;
    assert!(s.sim.capsule(me).swims && need > 0.0, "it swims, and needs water");
    let t = s.sim.snap.terrain.clone();
    let wd = |p: Vec3| t.water(p.x, p.z) - t.height(p.x, p.z);
    for _ in 0..600 {
        s.sim.step(0.1);
    }
    let a = s.sim.actor(me).unwrap().clone();
    let eye = a.pos.y + a.dims.eye;
    let under = t.water(a.pos.x, a.pos.z) - eye;
    eprintln!("eyes {under:.2} m under, in {:.1} m of water", wd(a.pos));
    assert!(a.swimming, "swimming");
    assert!((0.6..2.6).contains(&under), "at its depth (1–2 m): {under}");
    // Sent ashore: it goes to the edge of its water and no further.
    s.sim.plan(me, vec![Action::Goto { target: Target::Point(shore.to_array()), run: false }], "", false);
    let mut least = f32::MAX;
    for _ in 0..600 {
        s.sim.step(0.1);
        least = least.min(wd(s.sim.actor(me).unwrap().pos));
    }
    assert!(least >= need - 0.05, "never out of its water: {least} (needs {need})");
}

/// Washed up on the shore, a fish suffers, flops, and works back to the water.
#[test]
fn a_stranded_fish_suffers_and_flops_back() {
    let w = world_with("stranded", 12, coast());
    put_carp(&w);
    let (_, shore) = sea_and_shore(&w);
    let id = add_being(&w, "Old Whiskers", "carp", &[], shore);
    let mut s = session(&w, 12, None);
    calm(&mut s);
    let me = ActorId::Npc(id);
    let t = s.sim.snap.terrain.clone();
    let wd = |p: Vec3| t.water(p.x, p.z) - t.height(p.x, p.z);
    let health = |s: &Session| s.sim.cast.get(id).map(|n| n.props[crate::sim::props::P_HEALTH]).unwrap();
    let start = health(&s);
    assert!(wd(s.sim.actor(me).unwrap().pos) < 0.0, "on dry land");
    for _ in 0..100 {
        s.sim.step(0.1);
    }
    assert!(health(&s) < start, "it suffers out of the water");
    for _ in 0..1200 {
        s.sim.step(0.1);
    }
    let p = s.sim.actor(me).unwrap().pos;
    assert!(wd(p) > 0.0, "back in the water: {} m deep", wd(p));
    let h = health(&s);
    for _ in 0..100 {
        s.sim.step(0.1);
    }
    assert!(health(&s) >= h - 1e-4, "and it suffers no more: {h} then {}", health(&s));
}

/// What doesn't swim (no swim speed) or shuns being wet keeps out of deep
/// water; anything else that swims can go in.
#[test]
fn going_in_the_water_follows_the_species() {
    let w = world_with("swimmers", 12, coast());
    let (_, shore) = sea_and_shore(&w);
    let sp = |name: &str, extra: serde_json::Value| {
        let mut v = serde_json::json!({ "name": name, "body": "quadruped", "mind": "instinct", "speech": "none", "move": { "walk": 1.0, "run": 2.0, "swim": 1.0 } });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        w.db.with(|c| db::put_species(c, name, &v.to_string())).unwrap();
    };
    sp("otter", serde_json::json!({ "water": 0.5 }));
    sp("lynx", serde_json::json!({ "shuns": ["wet"] }));
    sp("mole", serde_json::json!({ "move": { "walk": 0.5, "run": 1.0, "swim": 0 } }));
    let ids: Vec<i64> = ["otter", "lynx", "mole"].iter().map(|n| add_being(&w, n, n, &[], shore)).collect();
    let s = session(&w, 12, None);
    let caps: Vec<_> = ids.iter().map(|id| s.sim.capsule(ActorId::Npc(*id))).collect();
    assert!(caps[0].swims && caps[0].needs_water == 0.0, "an otter swims and goes ashore");
    assert!(!caps[1].swims, "a lynx shuns the wet");
    assert!(!caps[2].swims, "a mole doesn't swim");
}

/// Pictures (look at them): from a boat over the coast's shallows, and
/// under the water among the kelp with the carp swimming by.
#[test]
#[ignore]
fn water_life_png() {
    let dir = std::env::var("POCKET_PNG_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let w = world_with("waterpng", 12, coast());
    add_type(&w, KELP);
    add_type(&w, LILY);
    put_carp(&w);
    let (deep, shore) = sea_and_shore(&w);
    add_being(&w, "Old Whiskers", "carp", &[], deep);
    let mut s = session(&w, 12, None);
    calm(&mut s);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.42;
    for _ in 0..300 {
        s.sim.step(0.1);
    }
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    s.sim.flip(snap.clone());
    let fish = s.sim.cast.npcs[0].a.pos;
    let back = (Vec3::new(shore.x, 0.0, shore.z) - Vec3::new(fish.x, 0.0, fish.z)).normalize_or_zero();
    let yaw = (-back.x).atan2(-back.z);
    let at = |p: Vec3, pitch: f32| crate::render::Camera { pos: p, yaw, pitch, fov_y: 1.05, roll: 0.0 };
    png_of(&mut s, &snap, &gpu, at(Vec3::new(fish.x, 0.0, fish.z) + back * 8.0 + Vec3::Y * 3.0, -0.35), &format!("{dir}/pocket-water-over.png"));
    png_of(&mut s, &snap, &gpu, at(Vec3::new(fish.x, fish.y + 0.3, fish.z) + back * 4.0, -0.05), &format!("{dir}/pocket-water-under.png"));
}

fn put_merfolk(w: &W) {
    let mer = serde_json::json!({ "name": "merfolk", "plural": "merfolk", "body": "figure", "mind": "sapient", "speech": "words", "social": "village",
        "move": { "walk": 0.5, "run": 1.0, "swim": 2.0 }, "water": 1, "depth": [0.2, 0.6], "description": "a people of the sea" });
    w.db.with(|c| db::put_species(c, "merfolk", &mer.to_string())).unwrap();
}

/// A house on the sea bed: one of a people of the water swims down to its
/// doorway and in, opening the door, to its bed; and inside it keeps to its
/// floor rather than floating up through the roof.
#[test]
fn merfolk_swim_into_a_house_under_the_water() {
    let w = world_with("sunken", 12, coast());
    put_merfolk(&w);
    let hut = add_type(&w, &fixture("physics/hut.js").replace("tags: [\"building\"]", "tags: [\"building\", \"water\"]"));
    add_type(&w, &fixture("physics/sword.js"));
    // Open sea: deep all round, so the house and its yard are well under.
    let t = w.terrain.clone();
    let wd = |x: f32, z: f32| t.water(x, z) - t.height(x, z);
    let deep = (0..40000).map(|k| {
        let (a, r) = (k as f32 * 2.399, (k as f32).sqrt() * 2.0);
        Vec3::new(w.spawn.x + a.cos() * r, 0.0, w.spawn.z + a.sin() * r)
    }).find(|p| wd(p.x, p.z) > 7.0 && (0..12).all(|i| { let b = i as f32 * 0.524; wd(p.x + b.cos() * 14.0, p.z + b.sin() * 14.0) > 6.0 })).expect("open sea");
    let ok = commit_places(&w, vec![pl(hut, deep, 0.0)], None);
    let h = ok.snapshot.instances.iter().find(|p| p.type_id == hut).expect("the hut stands under the water").pos;
    assert!(t.water(h.x, h.z) - h.y > 3.0, "on the bed, {:.1} m down", t.water(h.x, h.z) - h.y);
    let id = add_being(&w, "Nerys", "merfolk", &[], h + Vec3::new(5.0, 0.0, 8.0));
    let mut s = session(&w, 12, None);
    calm(&mut s);
    let me = ActorId::Npc(id);
    for _ in 0..100 {
        s.sim.step(0.1);
    }
    let a = s.sim.actor(me).unwrap().clone();
    // Let be, it swims at its own depth near the top (eyes 0.2–0.6 m
    // under), well over the roof.
    let under = |a: &crate::sim::actor::Actor| t.water(a.pos.x, a.pos.z) - (a.pos.y + a.dims.eye);
    assert!(a.swimming && (0.0..1.0).contains(&under(&a)), "swimming at its depth first: eyes {:.1} m under", under(&a));
    assert!(a.pos.y > h.y + 3.0, "over the roof: {:.1} m over the floor", a.pos.y - h.y);
    let bed = s.sim.rooms[0].beds[0];
    let r = act_once(&mut s, me, Action::Goto { target: Target::Point(bed.to_array()), run: false }, 40.0);
    let at = s.sim.actor(me).unwrap().pos;
    assert!(s.sim.room_at(at + Vec3::Y * 0.3).is_some(), "Nerys swam in through the door: {r}, at {:?} from the hut", at - h);
    assert!(s.sim.log.recent.iter().any(|e| e.kind == "opened" && e.actor == Some(me)), "Nerys opened the door to go in");
    for _ in 0..100 {
        s.sim.step(0.1);
    }
    let at = s.sim.actor(me).unwrap().pos;
    assert!(s.sim.room_at(at + Vec3::Y * 0.3).is_some() && at.y < h.y + 1.0, "still inside, by the floor: {:.2} m over it", at.y - h.y);
    // And out again, back up to its depth.
    let out = h + Vec3::new(6.0, 0.0, 10.0);
    act_once(&mut s, me, Action::Goto { target: Target::Point(out.to_array()), run: false }, 40.0);
    for _ in 0..100 {
        s.sim.step(0.1);
    }
    let a = s.sim.actor(me).unwrap().clone();
    assert!(s.sim.room_at(a.pos + Vec3::Y * 0.3).is_none() && (0.0..1.0).contains(&under(&a)), "out and up to its depth again: {:?} from the hut, eyes {:.1} m under", a.pos - h, under(&a));
}

const SUB: &str = r#"export const meta = { name: "brass submarine", bounds: [0.9, 0.9, 2.2], tags: ["prop", "water"], props: { mass: 900 },
  anchors: [{ kind: "seat", at: [0, -0.2, 0], face: 0 }], drive: { speed: 6, on: "under", by: "engine" } };
export function sdf(x, y, z, k) { return max(ellipsoid(x, y, z, 0.9, 0.9, 2.2), -box(x, y - 0.7, z, 0.5, 0.4, 0.6)); }
export function color(x, y, z, k) { return rgb(180, 140, 60); }
"#;

/// A submarine made on the sea bed sits there; driven, Space takes it up
/// (never over the surface), c back down to the bed, and the traveler gets
/// out of it down there and swims.
#[test]
fn a_submarine_dives_and_lets_you_out_under_the_water() {
    let w = world_with("sub", 12, coast());
    let sub = add_type(&w, SUB);
    let (deep, _) = sea_and_shore(&w);
    let t = w.terrain.clone();
    let id0 = place(&w, sub, deep, 0.0);
    let mut s = session(&w, 12, None);
    calm(&mut s);
    let me = ActorId::Player;
    s.sim.player.pos = deep + Vec3::new(1.5, 0.5, 0.0);
    let r = s.sim.act(me, Action::Ride { target: Target::Instance(id0) });
    assert!(r.as_ref().is_ok_and(|o| o.msg.contains("Space up")), "{r:?}");
    let id = s.sim.player.aboard.unwrap();
    let feet = |s: &Session| {
        let th = s.sim.things.get(id).unwrap();
        th.pos.y + s.sim.snap.type_of(th.type_id).unwrap().bottom * th.scale
    };
    let bed = |s: &Session| {
        let p = s.sim.things.get(id).unwrap().pos;
        t.height(p.x, p.z)
    };
    assert!(feet(&s) - bed(&s) < 0.5, "made on the bed");
    let drive = |s: &mut Session, fwd: f32, climb: f32, n: usize| {
        for _ in 0..n {
            s.sim.steer(me, fwd, 0.0, climb, 0.1);
            s.step(0.1);
        }
    };
    drive(&mut s, 0.0, 1.0, 10);
    let up = feet(&s) - bed(&s);
    assert!(up > 2.0, "Space takes it up ({up:.1} m off the bed)");
    drive(&mut s, 0.0, 0.0, 10);
    assert!((feet(&s) - bed(&s) - up).abs() < 0.5, "let be, it holds its depth");
    drive(&mut s, 1.0, 1.0, 60);
    let p = s.sim.things.get(id).unwrap().pos;
    assert!(feet(&s) < t.water(p.x, p.z), "never over the surface");
    drive(&mut s, 0.0, -1.0, 80);
    assert!(feet(&s) - bed(&s) < 0.5, "c takes it down to the bed ({:.1} m off it)", feet(&s) - bed(&s));
    let r = s.sim.act(me, Action::Dismount);
    assert!(r.is_ok() && s.sim.player.aboard.is_none(), "out under the water: {r:?}");
    s.step(0.1);
    let a = &s.sim.player;
    assert!(a.swimming && a.pos.y < t.water(a.pos.x, a.pos.z) - 2.0, "swimming down there");
}

/// At night on the sea, a night walker comes for the traveler through the
/// water and dives to claw them where they swim near the bed.
#[test]
fn a_night_walker_swims_down_to_the_traveler() {
    let w = world_with("deep dark", 12, coast());
    let (deep, _) = sea_and_shore(&w);
    let t = w.terrain.clone();
    let mut s = session(&w, 12, None);
    calm(&mut s);
    s.sim.cfg.difficulty = 1;
    s.sim.player.pos = deep + Vec3::Y * 0.5;
    s.sim.player.yaw = 0.0;
    s.sim.t = before_dusk(1.0, 2.0);
    let all = record(&mut s);
    let mut dark = Vec::new();
    for _ in 0..40 {
        s.sim.player.swim = -1.0;
        s.step(0.1);
        dark = the_dark(&s);
    }
    for id in s.sim.cast.npcs.iter_mut().filter(|n| n.species.hostile) {
        id.a.pos += Vec3::new(400.0, 0.0, 400.0);
        id.think_at = f64::MAX;
    }
    assert_eq!(dark.len(), 1, "a walker came, though there's only sea about");
    // Up at the surface, behind the traveler.
    let p = deep - s.sim.player.forward() * 8.0;
    let n = s.sim.cast.get_mut(dark[0]).unwrap();
    n.a.pos = Vec3::new(p.x, t.water(p.x, p.z) - 1.5, p.z);
    n.think_at = 0.0;
    for _ in 0..300 {
        s.sim.player.swim = -1.0;
        s.step(0.1);
        if !of(&all, "touched").is_empty() {
            break;
        }
    }
    let a = s.sim.actor(ActorId::Npc(dark[0])).unwrap();
    let me = s.sim.player.pos;
    assert!(me.y < t.water(me.x, me.z) - 2.5, "the traveler stayed down");
    assert!(!of(&all, "touched").is_empty(), "it dove and clawed them: it's {:?} from them, swimming {}", a.pos - me, a.swimming);
}
