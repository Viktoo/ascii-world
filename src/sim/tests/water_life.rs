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
