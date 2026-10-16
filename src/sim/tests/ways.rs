//! Ways and water (docs/ways.md): a settlement shapes its land (a lagoon,
//! an island, a canal with quays, a street), buildings line its ways, a
//! bridge carries the street over the canal, and its boat floats.

use super::*;
use crate::model::{RegionCommit, Settlement, TypeRef};

/// A small city of canals on the lawn, built through the world model as a
/// region's plan would be.
fn canal_city(w: &W, hut: u32) -> (Vec3, crate::model::CommitOk) {
    let lamp = builtin_id(w, "lamppost");
    let boat = builtin_id(w, "boat");
    let h = flat_spot(w, 60.0);
    let r = crate::world::region_of(h.x, h.z);
    let (ox, oz) = (r.0 as f32 * crate::world::REGION, r.1 as f32 * crate::world::REGION);
    let (lx, lz) = (h.x - ox, h.z - oz);
    let plan = serde_json::json!([
        { "what": "lagoon", "circle": [lx, lz, 95], "how": "cut", "height": -1.8 },
        { "what": "island", "rect": [lx, lz, 110, 76, 0], "how": "fill", "height": 0.8 },
        { "what": "canal", "name": "the Long Canal", "line": [[lx - 70.0, lz], [lx + 70.0, lz]], "width": 8, "how": "cut", "height": -1.6,
          "rim": [215, 210, 195], "color": [40, 52, 48], "frontage": { "type": "test hut", "sides": "both", "every": 9 } },
        { "what": "street", "line": [[lx, lz - 36.0], [lx, lz + 36.0]], "width": 3, "color": [190, 180, 160] }
    ]);
    let ground = crate::settle::parse(plan.as_array().unwrap(), ox, oz).into_iter().map(|p| {
        let ty = p.frontage.as_ref().map(|_| TypeRef::Existing(hut));
        (p, ty)
    }).collect();
    let st = Settlement { x: h.x, z: h.z, road: Some((None, 1.2)), lamps: Some(TypeRef::Existing(lamp)), ground, bridge: None, vehicle: Some(TypeRef::Existing(boat)) };
    let region = RegionCommit { r, plan_json: "{}".into(), info: Default::default(), characters: vec![], settlement: Some(st) };
    (h, commit_places(w, vec![], Some(region)))
}

#[test]
fn a_city_of_canals_shapes_its_land() {
    let w = world_with("canals", 66, bare());
    let hut = add_type(&w, &fixture("physics/hut.js"));
    let (h, ok) = canal_city(&w, hut);
    let snap = &ok.snapshot;
    let t = &snap.terrain;
    // The land: the lagoon, the island in it, the canal through the island.
    assert!((t.height(h.x + 80.0, h.z + 50.0) + 1.8).abs() < 0.05, "the lagoon: {}", t.height(h.x + 80.0, h.z + 50.0));
    assert!((t.height(h.x + 30.0, h.z + 20.0) - 0.8).abs() < 0.1, "the island: {}", t.height(h.x + 30.0, h.z + 20.0));
    assert!((t.height(h.x + 30.0, h.z) + 1.6).abs() < 0.05, "the canal: {}", t.height(h.x + 30.0, h.z));
    assert!(t.water_depth(h.x + 30.0, h.z) > 1.5 && t.water_depth(h.x + 30.0, h.z + 20.0) == 0.0, "water in the canal, not on the island");
    // Houses line both quays, doors to the water.
    let huts: Vec<&crate::world::Placed> = snap.instances.iter().filter(|p| p.type_id == hut).collect();
    let north = huts.iter().filter(|p| p.pos.z > h.z).count();
    let south = huts.iter().filter(|p| p.pos.z < h.z).count();
    assert!(north >= 4 && south >= 4, "houses on both sides: {north} north, {south} south");
    for p in &huts {
        let e = snap.type_of(hut).unwrap();
        let door = crate::model::front_of(p, e);
        assert!((door.z - h.z).abs() < (p.pos.z - h.z).abs(), "the door at {door} faces the canal (house at {})", p.pos);
        assert!(t.height(p.pos.x, p.pos.z) > 0.3, "on dry land");
    }
    // A bridge carries the street over the canal, level with it at the ends.
    let bridge = builtin_id(&w, "bridge");
    let b = snap.instances.iter().find(|p| p.type_id == bridge).expect("a bridge");
    assert!((b.pos.x - h.x).abs() < 1.0 && (b.pos.z - h.z).abs() < 1.0, "where the street crosses: {}", b.pos);
    assert!((b.pos.y - 0.8).abs() < 0.3, "its ends at the street's height: {}", b.pos.y);
    // The boat floats on the water.
    let boat = builtin_id(&w, "boat");
    let bt = snap.instances.iter().find(|p| p.type_id == boat).expect("a boat");
    assert!(t.water_depth(bt.pos.x, bt.pos.z) > 0.8, "on water: {}", bt.pos);
    // Saved: the land comes back the same when the world is loaded again.
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let m = crate::model::WorldModel::load(w.db.clone(), None, live).unwrap();
    let t2 = m.terrain();
    for (dx, dz) in [(80.0, 50.0), (30.0, 20.0), (30.0, 0.0), (0.0, 0.0), (-50.0, 10.0)] {
        assert!((t2.height(h.x + dx, h.z + dz) - t.height(h.x + dx, h.z + dz)).abs() < 1e-4, "same land after loading at {dx}, {dz}");
    }
}

/// Pictures of the canal city (look at them): along the canal from a quay,
/// over the bridge from the street, and from above.
#[test]
#[ignore]
fn canal_city_png() {
    let dir = std::env::var("POCKET_PNG_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let w = world_with("canalpng", 66, bare());
    let hut = add_type(&w, &fixture("physics/hut.js"));
    let (h, _) = canal_city(&w, hut);
    let mut s = session(&w, 66, None);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.42;
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    s.sim.flip(snap.clone());
    let t = snap.terrain.clone();
    let look = |from: Vec3, to: Vec3| {
        let d = to - from;
        crate::render::Camera { pos: from, yaw: d.x.atan2(d.z), pitch: (d.y / Vec3::new(d.x, 0.0, d.z).length()).atan(), fov_y: 1.05, roll: 0.0 }
    };
    let quay = Vec3::new(h.x - 40.0, t.height(h.x - 40.0, h.z + 4.6) + 1.7, h.z + 4.6);
    png_of(&mut s, &snap, &gpu, look(quay, Vec3::new(h.x + 10.0, 0.5, h.z)), &format!("{dir}/pocket-canal-quay.png"));
    let street = Vec3::new(h.x + 0.5, t.height(h.x + 0.5, h.z - 22.0) + 1.7, h.z - 22.0);
    png_of(&mut s, &snap, &gpu, look(street, Vec3::new(h.x, 1.0, h.z + 10.0)), &format!("{dir}/pocket-canal-bridge.png"));
    let above = Vec3::new(h.x - 70.0, 45.0, h.z - 70.0);
    png_of(&mut s, &snap, &gpu, look(above, Vec3::new(h.x, 0.0, h.z)), &format!("{dir}/pocket-canal-above.png"));
}

