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


/// Seats: two sit on a bench and a third finds no room; a villager takes
/// the city's boat across the lagoon (out of the canal, round the island)
/// to the far shore and walks on; a passenger rides along with them.
#[test]
fn benches_seat_two_and_a_boat_crosses_the_lagoon() {
    let w = world_with("rowing", 67, bare());
    let hut = add_type(&w, &fixture("physics/hut.js"));
    let (h, ok) = canal_city(&w, hut);
    let bench = builtin_id(&w, "bench");
    let boat_t = builtin_id(&w, "boat");
    let boat_i = ok.snapshot.instances.iter().find(|p| p.type_id == boat_t).unwrap().clone();
    let bp = Vec3::new(h.x + 20.0, 0.0, h.z + 20.0);
    let bench_i = {
        let y = ok.snapshot.terrain.height(bp.x, bp.z) + 0.48;
        w.db.with(|c| db::add_instance(c, bench as i64, w.version, bp.x, y, bp.z, 0.0, 1.0, &[1.0, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5])).unwrap()
    };
    // Ola stands on the quay by the boat; Bo across the island.
    let quay = crate::model::dry_near(&ok.snapshot.terrain, boat_i.pos);
    let ola = add_char(&w, "Ola", "kind", &[], quay);
    let bo = add_char(&w, "Bo", "kind", &[], Vec3::new(bp.x + 3.0, ok.snapshot.terrain.height(bp.x + 3.0, bp.z + 2.0), bp.z + 2.0));
    let mut s = session(&w, 67, None);
    calm(&mut s);
    let (me, o, b) = (ActorId::Player, ActorId::Npc(ola), ActorId::Npc(bo));
    // Two on the bench, then no room.
    s.sim.player.pos = Vec3::new(bp.x - 1.0, s.sim.snap.terrain.height(bp.x - 1.0, bp.z + 1.0), bp.z + 1.0);
    s.sim.act(me, Action::Ride { target: Target::Instance(bench_i) }).unwrap();
    assert!(s.sim.player.aboard.is_some(), "sitting");
    let bench_t = s.sim.player.aboard.unwrap();
    let seat_y = s.sim.player.pos.y;
    assert!(seat_y > s.sim.snap.terrain.height(bp.x, bp.z) - 0.5, "on the bench, not in the ground");
    s.sim.cast.get_mut(bo).unwrap().a.pos = Vec3::new(bp.x + 1.0, s.sim.snap.terrain.height(bp.x + 1.0, bp.z + 1.0), bp.z + 1.0);
    s.sim.act(b, Action::Ride { target: Target::Thing(bench_t) }).unwrap();
    assert_eq!(s.sim.riders_in(bench_t).len(), 2, "two sit on it");
    let seats: Vec<u8> = s.sim.riders_in(bench_t).iter().map(|r| r.1).collect();
    assert!(seats.contains(&0) && seats.contains(&1), "side by side: {seats:?}");
    s.sim.act(me, Action::Dismount).unwrap();
    assert!(s.sim.player.aboard.is_none());
    // Ola goes to the far shore, past the lagoon: she takes the boat.
    let far = crate::model::dry_near(&s.sim.snap.terrain, Vec3::new(h.x + 15.0, 0.0, h.z + 115.0));
    s.sim.cast.get_mut(ola).unwrap().plan.push_back(Action::Goto { target: Target::Point(far.to_array()), run: false });
    let mut boarded = false;
    let mut max_depth: f32 = 0.0;
    for _ in 0..1800 {
        s.step(0.1);
        if let Some(v) = s.sim.actor(o).unwrap().aboard {
            boarded = true;
            let p = s.sim.things.get(v).unwrap().pos;
            max_depth = max_depth.max(s.sim.snap.terrain.water_depth(p.x, p.z));
        }
        let p = s.sim.actor(o).unwrap().pos;
        if boarded && s.sim.actor(o).unwrap().aboard.is_none() && Vec3::new(p.x - far.x, 0.0, p.z - far.z).length() < 4.0 {
            break;
        }
    }
    let p = s.sim.actor(o).unwrap().pos;
    assert!(boarded, "she took the boat: {:?}", events(&s.sim, "plan_failed"));
    assert!(max_depth > 1.0, "out on the water ({max_depth:.1} m deep)");
    assert!(Vec3::new(p.x - far.x, 0.0, p.z - far.z).length() < 6.0, "and got to the far shore: {p} vs {far}");
    assert!(s.sim.actor(o).unwrap().aboard.is_none(), "on foot again");
}

/// What drives a vehicle matters: a horse cart crawls when pushed by hand
/// and goes at a horse's pace with one harnessed (the horse walking before
/// it); a bicycle goes at its rider's strength and tires them; a sail goes
/// with the wind.
#[test]
fn beasts_pull_carts_legs_pedal_and_sails_need_wind() {
    let w = world_with("power", 68, bare());
    let src = fixture("vehicles/cart.js").replace("name: \"hand cart\"", "name: \"horse cart\"").replace("on: \"land\" }", "on: \"land\", by: \"horse\" }");
    let cart = add_type(&w, &src);
    let bike = add_type(&w, &fixture("vehicles/cart.js").replace("name: \"hand cart\"", "name: \"bicycle\"").replace("on: \"land\" }", "on: \"land\", by: \"legs\" }"));
    let skiff = add_type(&w, &fixture("vehicles/cart.js").replace("name: \"hand cart\"", "name: \"sand yacht\"").replace("on: \"land\" }", "on: \"land\", by: \"sail\" }"));
    let a = flat_spot(&w, 30.0);
    let ci = place(&w, cart, a, 0.0);
    let bi = place(&w, bike, a + Vec3::new(12.0, 0.0, 0.0), 0.0);
    let si = place(&w, skiff, a + Vec3::new(-12.0, 0.0, 0.0), 0.0);
    let ola = add_char(&w, "Ola", "kind", &[], a + Vec3::new(13.5, 0.0, 0.0));
    let mut s = session(&w, 68, None);
    calm(&mut s);
    let me = ActorId::Player;
    // Pushed by hand: it crawls.
    s.sim.player.pos = ground(&w, a.x + 1.5, a.z);
    s.sim.act(me, Action::Ride { target: Target::Instance(ci) }).unwrap();
    let id = s.sim.player.aboard.unwrap();
    assert!(s.sim.hitched.get(&id).is_none(), "no beast near");
    let p0 = s.sim.things.get(id).unwrap().pos;
    for _ in 0..40 {
        s.sim.steer(me, 1.0, 0.0, 0.0, 0.1);
        s.step(0.1);
    }
    let crawl = (s.sim.things.get(id).unwrap().pos - p0).length();
    s.sim.act(me, Action::Dismount).unwrap();
    // A horse beside it: harnessed, it pulls.
    let horse = add_being(&w, "Bram", "horse", &[], a + Vec3::new(2.0, 0.0, 3.0));
    let mut s = session(&w, 68, None);
    calm(&mut s);
    let hb = ActorId::Npc(horse);
    s.sim.player.pos = ground(&w, a.x + 1.5, a.z);
    s.sim.act(me, Action::Ride { target: Target::Instance(ci) }).unwrap();
    let id = s.sim.player.aboard.unwrap();
    assert_eq!(s.sim.hitched.get(&id), Some(&hb), "the horse is harnessed: {:?}", events(&s.sim, "hitched"));
    let p0 = s.sim.things.get(id).unwrap().pos;
    for _ in 0..40 {
        s.sim.steer(me, 1.0, 0.0, 0.0, 0.1);
        s.step(0.1);
        let c = s.sim.things.get(id).unwrap();
        let h = s.sim.actor(hb).unwrap().pos;
        let ahead = Vec3::new(h.x - c.pos.x, 0.0, h.z - c.pos.z);
        assert!(ahead.dot(Vec3::new(c.yaw.sin(), 0.0, c.yaw.cos())) > 1.0, "the horse walks before the cart");
    }
    let pulled = (s.sim.things.get(id).unwrap().pos - p0).length();
    assert!(pulled > crawl * 3.0, "a horse pulls it ({pulled:.1} m) where a hand pushed it ({crawl:.1} m)");
    // Muscle: a weak rider pedals slower than a strong one, and tires.
    let bike_id = s.sim.liven(&Target::Instance(bi)).unwrap();
    let o = ActorId::Npc(ola);
    s.sim.cast.get_mut(ola).unwrap().a.pos = ground(&w, a.x + 13.5, a.z);
    s.sim.act(o, Action::Ride { target: Target::Thing(bike_id) }).unwrap();
    let strong = s.sim.drive_power(bike_id, o);
    s.sim.cast.get_mut(ola).unwrap().needs.fatigue = 0.9;
    let tired = s.sim.drive_power(bike_id, o);
    assert!(tired < strong * 0.7, "tired legs pedal slower: {tired:.2} vs {strong:.2}");
    s.sim.cast.get_mut(ola).unwrap().needs.fatigue = 0.0;
    for _ in 0..30 {
        s.sim.steer(o, 1.0, 0.0, 0.0, 0.1);
        s.step(0.1);
    }
    assert!(s.sim.cast.get(ola).unwrap().needs.fatigue > 0.0, "pedalling tires her");
    // Wind: a sail goes with it.
    let sail = s.sim.liven(&Target::Instance(si)).unwrap();
    s.sim.wx.wind = 0.0;
    let calm_air = s.sim.drive_power(sail, me);
    s.sim.wx.wind = 0.9;
    let gale = s.sim.drive_power(sail, me);
    assert!(gale > calm_air * 3.0, "the wind fills the sail: {gale:.2} vs {calm_air:.2}");
}

/// A land of rivers: springs in the hills run down to the sea, their water
/// standing above the sea's level, cut into the land; a street a
/// settlement lays across one is bridged, the river running on under it.
fn river_land(tag: &str) -> W {
    let look = crate::world::Look { name: "Rivers".into(), rivers: 1.0, ..Default::default() };
    world_with(tag, 71, look)
}

/// A spot on a river well above the sea, and its edit.
fn river_spot(w: &W) -> (Vec3, crate::ground::Edit) {
    let t = &w.terrain;
    let rivers: Vec<crate::ground::Edit> = t.carve.read().ground.layer(crate::ground::Layer::Land).iter().filter(|e| e.kind == "river").cloned().collect();
    assert!(!rivers.is_empty(), "rivers near the spawn");
    for e in &rivers {
        let crate::ground::Shape::Line { pts, .. } = &e.shape else { continue };
        let p = pts[pts.len() / 2];
        let (h, wl) = (t.height(p[0], p[1]), t.water(p[0], p[1]));
        if wl > crate::terrain::WATER_LEVEL + 2.0 && wl - h > 0.5 {
            return (Vec3::new(p[0], h, p[1]), e.clone());
        }
    }
    panic!("no river above the sea among {}", rivers.len());
}

#[test]
fn rivers_run_through_the_land_and_streets_bridge_them() {
    let w = river_land("river");
    let (p, river) = river_spot(&w);
    let t = &w.terrain;
    assert!(t.water_depth(p.x, p.z) > 0.5, "water in the river: {}", t.water_depth(p.x, p.z));
    // Its banks: dry ground a little way off to the side.
    let crate::ground::Shape::Line { pts, half } = &river.shape else { unreachable!() };
    let i = pts.len() / 2;
    let d = glam::Vec2::from(pts[(i + 1).min(pts.len() - 1)]) - glam::Vec2::from(pts[i.saturating_sub(1)]);
    let side = glam::Vec2::new(-d.y, d.x).normalize() * (half + river.edge + 3.0);
    assert!(t.water_depth(p.x + side.x, p.z + side.y) == 0.0, "dry beside it");
    // A street across it, through the world model as a plan would lay it.
    let r = crate::world::region_of(p.x, p.z);
    let (ox, oz) = (r.0 as f32 * crate::world::REGION, r.1 as f32 * crate::world::REGION);
    let a = glam::Vec2::new(p.x, p.z) - side * 2.5;
    let b = glam::Vec2::new(p.x, p.z) + side * 2.5;
    let plan = serde_json::json!([{ "what": "street", "line": [[a.x - ox, a.y - oz], [b.x - ox, b.y - oz]], "width": 3, "color": [150, 130, 100] }]);
    let ground = crate::settle::parse(plan.as_array().unwrap(), ox, oz).into_iter().map(|p| (p, None)).collect();
    let st = crate::model::Settlement { x: p.x, z: p.z, road: None, ground, ..Default::default() };
    let region = crate::model::RegionCommit { r, plan_json: "{}".into(), info: Default::default(), characters: vec![], settlement: Some(st) };
    let ok = commit_places(&w, vec![], Some(region));
    let t = &ok.snapshot.terrain;
    let bridge = builtin_id(&w, "bridge");
    let br = ok.snapshot.instances.iter().find(|q| q.type_id == bridge).expect("a bridge over the river");
    assert!((br.pos - p).length() < half + 4.0, "over the river: {} vs {p}", br.pos);
    assert!(t.water_depth(p.x, p.z) > 0.5, "the river still runs under it: {}", t.water_depth(p.x, p.z));
}

/// Pictures of a river (look at them): from its bank, and from above.
#[test]
#[ignore]
fn river_png() {
    let dir = std::env::var("POCKET_PNG_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let w = river_land("riverpng");
    let (p, river) = river_spot(&w);
    let mut s = session(&w, 71, None);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.42;
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    snap.terrain.lay_rivers(p.x, p.z);
    s.sim.flip(snap.clone());
    let t = snap.terrain.clone();
    let crate::ground::Shape::Line { pts, half } = &river.shape else { unreachable!() };
    let i = pts.len() / 2;
    let d = (glam::Vec2::from(pts[(i + 1).min(pts.len() - 1)]) - glam::Vec2::from(pts[i.saturating_sub(1)])).normalize();
    let side = glam::Vec2::new(-d.y, d.x);
    let look = |from: Vec3, to: Vec3| {
        let dd = to - from;
        crate::render::Camera { pos: from, yaw: dd.x.atan2(dd.z), pitch: (dd.y / Vec3::new(dd.x, 0.0, dd.z).length()).atan(), fov_y: 1.05, roll: 0.0 }
    };
    let bank = glam::Vec2::new(p.x, p.z) + side * (half + river.edge + 1.0) - d * 6.0;
    let eye = Vec3::new(bank.x, t.height(bank.x, bank.y) + 1.7, bank.y);
    let ahead = glam::Vec2::new(p.x, p.z) + d * 25.0;
    png_of(&mut s, &snap, &gpu, look(eye, Vec3::new(ahead.x, t.water(ahead.x, ahead.y), ahead.y)), &format!("{dir}/pocket-river-bank.png"));
    let above = Vec3::new(p.x - d.x * 50.0 + side.x * 30.0, p.y + 40.0, p.z - d.y * 50.0 + side.y * 30.0);
    png_of(&mut s, &snap, &gpu, look(above, p + Vec3::new(d.x * 20.0, 0.0, d.y * 20.0)), &format!("{dir}/pocket-river-above.png"));
}
