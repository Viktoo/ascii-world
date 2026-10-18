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
    let st = Settlement { x: h.x, z: h.z, road: Some((None, 1.2)), lamps: Some(TypeRef::Existing(lamp)), ground, bridge: None, vehicle: Some(TypeRef::Existing(boat)), caves: vec![], walkways: vec![] };
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

/// A hollow dug under flat ground (a cellar dwelling): its mouth opens the
/// surface, a passage runs down to a chamber; what the plan sets "near" it
/// lies on its floor; a villager walks in through the mouth and down, and
/// out again; a stone dropped in there lands on the floor, not the land above.
fn cave_dwelling(w: &W) -> (crate::settle::CaveSpec, crate::model::CommitOk) {
    let stone = builtin_id(w, "stone");
    let lamp = builtin_id(w, "lamppost");
    let h = flat_spot(w, 50.0);
    let r = crate::world::region_of(h.x, h.z);
    let (ox, oz) = (r.0 as f32 * crate::world::REGION, r.1 as f32 * crate::world::REGION);
    let plan = serde_json::json!([{ "what": "cave", "name": "the Hollow", "mouth": [h.x - ox, h.z - oz], "toward": 90, "length": 12, "depth": 4, "room": 5 }]);
    let caves = crate::settle::parse_caves(plan.as_array().unwrap(), ox, oz);
    assert_eq!(caves.len(), 1);
    let spec = caves[0].clone();
    let st = crate::model::Settlement { x: h.x, z: h.z, road: None, caves: caves.clone(), ..Default::default() };
    let region = crate::model::RegionCommit { r, plan_json: "{}".into(), info: Default::default(), characters: vec![], settlement: Some(st) };
    let room = spec.room_at();
    let mut things = Vec::new();
    for (k, ty) in [stone, stone, lamp].into_iter().enumerate() {
        let a = k as f32 * 2.1;
        things.push(crate::model::Placement { ty: crate::model::TypeRef::Existing(ty), x: room.x + a.cos() * 1.5, z: room.y + a.sin() * 1.5, y: None, under: true, rot_y: 0.0, scale: 1.0, params: [k as f32, 1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5] });
    }
    (spec, commit_places(w, things, Some(region)))
}

#[test]
fn a_dwelling_dug_under_the_ground() {
    let w = world_with("cave", 69, bare());
    let (spec, ok) = cave_dwelling(&w);
    let t = &ok.snapshot.terrain;
    let room = spec.room_at();
    let surface = t.height(room.x, room.y);
    let floor = t.floor(room.x, surface - 1.6, room.y);
    assert!(surface - floor > 3.0, "a chamber well under the ground: floor {floor:.1}, surface {surface:.1}");
    assert!(!t.solid(Vec3::new(room.x, floor + 1.5, room.y)), "air in the chamber");
    assert!(t.solid(Vec3::new(room.x, surface - 0.4, room.y)), "a roof of earth over it");
    // The mouth opens the surface.
    let m = spec.mouth + spec.dir * 1.0;
    assert!(t.floor(m.x, t.height(m.x, m.y) + 0.5, m.y) < t.height(m.x, m.y) - 0.3, "the way down opens at the mouth");
    // What was set in it lies on its floor.
    let inside: Vec<&crate::world::Placed> = ok.snapshot.instances.iter().filter(|p| (glam::Vec2::new(p.pos.x, p.pos.z) - room).length() < 3.0).collect();
    assert_eq!(inside.len(), 3, "the stones and the lamp: {inside:?}");
    for p in &inside {
        let base = p.pos.y + ok.snapshot.type_of(p.type_id).unwrap().bottom * p.scale;
        assert!((base - floor).abs() < 0.5, "on the cave floor, not the land above: {base} vs {floor}");
    }
    // A villager walks in (by the mouth, down the passage) and out again.
    let ola = add_char(&w, "Ola", "kind", &[], Vec3::new(spec.mouth.x - 8.0, 0.0, spec.mouth.y + 6.0));
    let mut s = session(&w, 69, None);
    calm(&mut s);
    let o = ActorId::Npc(ola);
    let goal = Vec3::new(room.x, floor, room.y);
    s.sim.cast.get_mut(ola).unwrap().plan.push_back(Action::Goto { target: Target::Point(goal.to_array()), run: false });
    s.run(60.0, 0.1);
    let p = s.sim.actor(o).unwrap().pos;
    assert!(Vec3::new(p.x - goal.x, 0.0, p.z - goal.z).length() < 2.5, "she got to the chamber: {p} vs {goal}");
    assert!(p.y < surface - 2.5, "down in it, not on the land above: {} (surface {surface})", p.y);
    let out = Vec3::new(spec.mouth.x - 6.0, s.sim.snap.terrain.height(spec.mouth.x - 6.0, spec.mouth.y), spec.mouth.y);
    s.sim.cast.get_mut(ola).unwrap().plan.push_back(Action::Goto { target: Target::Point(out.to_array()), run: false });
    for _ in 0..600 {
        s.step(0.1);
    }
    let p = s.sim.actor(o).unwrap().pos;
    assert!((p - out).length() < 2.0, "and out again: {p} vs {out}");
    // A stone dropped in the chamber falls to its floor.
    let st = s.sim.type_by_name("stone").unwrap().id;
    let id = s.sim.spawn_thing(st, Vec3::new(room.x, floor + 2.0, room.y), 0.0, 1.0, Default::default(), false).unwrap();
    if let Some(th) = s.sim.things.get_mut(id) {
        th.asleep = false;
    }
    s.run(3.0, 0.05);
    let sp = s.sim.things.get(id).unwrap().pos;
    assert!(sp.y < floor + 0.6 && sp.y > floor - 0.3, "on the cave floor: {} vs {floor}", sp.y);
}

/// Pictures of the dwelling (look at them): down the mouth from outside,
/// and inside the chamber by its lamp at night.
#[test]
#[ignore]
fn cave_png() {
    let dir = std::env::var("POCKET_PNG_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let w = world_with("cavepng", 69, bare());
    let (spec, _) = cave_dwelling(&w);
    let mut s = session(&w, 69, None);
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
    let back = spec.mouth - spec.dir * 7.0;
    let eye = Vec3::new(back.x, t.height(back.x, back.y) + 1.8, back.y);
    let m = spec.mouth + spec.dir * 4.0;
    png_of(&mut s, &snap, &gpu, look(eye, Vec3::new(m.x, t.height(m.x, m.y) - 2.5, m.y)), &format!("{dir}/pocket-cave-mouth.png"));
    let room = spec.room_at();
    let floor = t.floor(room.x, t.height(room.x, room.y) - 1.6, room.y);
    let at = room - spec.dir * 3.0;
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.95;
    s.sim.step(0.05);
    png_of(&mut s, &snap, &gpu, look(Vec3::new(at.x, floor + 1.6, at.y), Vec3::new(room.x + spec.dir.x * 3.0, floor + 0.8, room.y + spec.dir.y * 3.0)), &format!("{dir}/pocket-cave-inside.png"));
}

/// Shaping the land in play: the traveler digs a canal off a river (it
/// fills with the river's water, above the sea), lays a road, and digs a
/// tunnel into the ground; all of it is still there after a save and a
/// load. A villager only does a stretch at a time.
#[test]
fn the_land_is_shaped_in_play() {
    use super::super::interp::{GroundFx, InterpEffect, PendingInterp};
    let w = river_land("shape");
    let (p, river) = river_spot(&w);
    let crate::ground::Shape::Line { pts, half } = &river.shape else { unreachable!() };
    let i = pts.len() / 2;
    let d = (glam::Vec2::from(pts[(i + 1).min(pts.len() - 1)]) - glam::Vec2::from(pts[i.saturating_sub(1)])).normalize();
    let side = glam::Vec2::new(-d.y, d.x);
    let mut s = session(&w, 71, None);
    let t = s.sim.snap.terrain.clone();
    // Standing out from the bank, facing the river: a canal dug to it.
    let stand = glam::Vec2::new(p.x, p.z) + side * (half + river.edge + 22.0);
    s.sim.player.pos = Vec3::new(stand.x, t.height(stand.x, stand.y), stand.y);
    let deed = |s: &mut Session, at: Vec3, g: GroundFx| {
        let pi = PendingInterp { actor: ActorId::Player, text: "dig".into(), held: None, target: Some(Target::Point(at.to_array())), hit: None, key: String::new(), at: 0.0, aim: None };
        s.sim.apply_interp(&pi, &InterpEffect { narration: "Done.".into(), ground: vec![g], ..Default::default() });
    };
    deed(&mut s, p, GroundFx { depth_m: 1.5, wide_m: 4.0, long_m: Some(30.0), way: "canal".into(), ..Default::default() });
    let mid = stand - side * 10.0;
    assert!(t.water_depth(mid.x, mid.y) > 0.5, "the canal holds water: {}", t.water_depth(mid.x, mid.y));
    assert!(t.water(mid.x, mid.y) > crate::terrain::WATER_LEVEL + 1.0, "the river's water, above the sea: {}", t.water(mid.x, mid.y));
    // A road laid off the other way, in its colour.
    let away = stand + side * 25.0;
    deed(&mut s, Vec3::new(away.x, 0.0, away.y), GroundFx { depth_m: 0.0, wide_m: 3.0, long_m: Some(20.0), way: "road".into(), color: Some([200.0, 180.0, 140.0]), ..Default::default() });
    let on = stand + side * 10.0;
    assert!(t.on_road(on.x, on.y, 0.0), "the road is there");
    // A tunnel dug into the ground the way they look.
    s.sim.player.yaw = 0.0;
    let ahead = s.sim.player.pos + Vec3::new(0.0, 0.0, 10.0);
    deed(&mut s, ahead, GroundFx { depth_m: 4.0, wide_m: 4.0, long_m: Some(10.0), way: "tunnel".into(), ..Default::default() });
    assert_eq!(t.carve.read().ground.cave_layer(crate::ground::Layer::Made).len(), 1, "a tunnel");
    // Saved, and back after loading.
    super::super::persist::save(&mut s.sim);
    let s2 = session(&w, 71, None);
    let t2 = s2.sim.snap.terrain.clone();
    assert!(t2.water_depth(mid.x, mid.y) > 0.5, "the canal after loading");
    assert!(t2.on_road(on.x, on.y, 0.0), "the road after loading");
    assert_eq!(t2.carve.read().ground.cave_layer(crate::ground::Layer::Made).len(), 1, "the tunnel after loading");
}

/// The traveler in the lagoon: floats with the eyes just over the water,
/// dives (no deeper than the bed), comes up again when let be, and swims
/// to the island and climbs out onto it.
#[test]
fn the_traveler_swims_and_climbs_out() {
    let w = world_with("swim", 68, bare());
    let hut = add_type(&w, &fixture("physics/hut.js"));
    let (h, ok) = canal_city(&w, hut);
    let mut s = session(&w, 68, None);
    s.sim.flip(ok.snapshot.clone());
    let t = ok.snapshot.terrain.clone();
    let (x, z) = (h.x, h.z - 65.0);
    let (bed, wl) = (t.height(x, z), t.water(x, z));
    assert!(wl - bed > 1.5, "deep water there: {bed} under {wl}");
    // Dropped in from the surface: under, then up again, afloat.
    s.sim.player.pos = Vec3::new(x, wl, z);
    s.sim.player.grounded = false;
    let eye = |s: &Session| s.sim.player.pos.y + s.sim.player.eye_height();
    for _ in 0..120 {
        s.sim.step(1.0 / 30.0);
    }
    assert!(s.sim.player.swimming, "swimming");
    assert!((eye(&s) - wl).abs() < 0.25 && eye(&s) > wl, "afloat, eyes just over the water: {} at {wl}", eye(&s));
    // Down (c held): under the water, never into the bed.
    s.sim.player.swim = -1.0;
    for _ in 0..90 {
        s.sim.step(1.0 / 30.0);
    }
    assert!(eye(&s) < wl - 0.2, "under: {}", eye(&s));
    assert!(eye(&s) >= bed + 0.3, "not into the bed: eyes at {} over {bed}", eye(&s));
    // Let go: back up.
    s.sim.player.swim = 0.0;
    for _ in 0..150 {
        s.sim.step(1.0 / 30.0);
    }
    assert!(eye(&s) > wl, "up again: {}", eye(&s));
    // Up (space held) never lifts it out of the water.
    s.sim.player.swim = 1.0;
    for _ in 0..60 {
        s.sim.step(1.0 / 30.0);
    }
    assert!(eye(&s) < wl + 0.3, "no higher than afloat: {}", eye(&s));
    s.sim.player.swim = 0.0;
    // Swim north to the island and out onto it.
    let mut walked = 0.0;
    while walked < 40.0 {
        let step = Vec3::Z * 3.0 * s.sim.gait_factor(ActorId::Player) / 30.0;
        s.sim.walk(ActorId::Player, step);
        s.sim.step(1.0 / 30.0);
        walked += step.z;
    }
    let p = s.sim.player.pos;
    assert!(p.z > h.z - 35.0, "reached the island: {p}");
    assert!(!s.sim.player.swimming && (p.y - t.height(p.x, p.z)).abs() < 0.1 && p.y > wl, "out on dry land: {p}");
}

/// Pictures from in the lagoon (look at them): afloat with the eyes just
/// over the water, under it looking level towards the island, and looking
/// up at the surface. Under the water everything is the water's colour.
#[test]
#[ignore]
fn underwater_png() {
    let dir = std::env::var("POCKET_PNG_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let w = world_with("underwater", 66, bare());
    let hut = add_type(&w, &fixture("physics/hut.js"));
    let (h, _) = canal_city(&w, hut);
    let mut s = session(&w, 66, None);
    s.sim.t = crate::render::sky::DAY_SECONDS * 0.42;
    let gpu = crate::render::gpu::Gpu::new().ok();
    let live = Arc::new(Mutex::new(crate::model::Live::default()));
    let mut model = crate::model::WorldModel::load(w.db.clone(), gpu.clone(), live).unwrap();
    let snap = model.snapshot().unwrap();
    s.sim.flip(snap.clone());
    let cam = |y: f32, pitch: f32| crate::render::Camera { pos: Vec3::new(h.x, y, h.z - 50.0), yaw: 0.0, pitch, fov_y: 1.05, roll: 0.0 };
    png_of(&mut s, &snap, &gpu, cam(0.12, -0.05), &format!("{dir}/pocket-swim-afloat.png"));
    png_of(&mut s, &snap, &gpu, cam(-0.9, 0.0), &format!("{dir}/pocket-swim-under.png"));
    png_of(&mut s, &snap, &gpu, cam(-1.2, 0.9), &format!("{dir}/pocket-swim-up.png"));
}

/// Water in dug ground, in a land with no sea and ground water 1.5 m down:
/// a pit dug past it fills up to it, a shallow one stays dry, and a pond
/// dug to be filled holds water to its brim (on whatever height it is),
/// none of it running out over the land round it.
#[test]
fn dug_ground_fills_from_the_ground_water_or_to_its_brim() {
    use crate::sim::interp::{GroundFx, InterpEffect, PendingInterp};
    let look = crate::world::Look { name: "Dry".into(), sea: None, groundwater: Some(1.5), ..Default::default() };
    let w = world_with("groundwater", 84, look);
    let mut s = session(&w, 84, None);
    let t = s.sim.snap.terrain.clone();
    assert_eq!(t.water(w.spawn.x, w.spawn.z), crate::ground::NO_WATER, "no sea");
    let mut dig = |s: &mut Session, p: Vec3, g: GroundFx| {
        s.sim.player.pos = p - Vec3::X * 16.0;
        let pi = PendingInterp { actor: ActorId::Player, text: "dig".into(), held: None, target: Some(Target::Point(p.to_array())), hit: None, key: String::new(), at: 0.0, aim: None };
        s.sim.apply_interp(&pi, &InterpEffect { narration: "Dug.".into(), ground: vec![g], ..Default::default() });
    };
    let wet = |p: Vec3| t.water(p.x, p.z) - t.height(p.x, p.z);
    // Level spots, high or low (nothing is under a sea here).
    let level = |d: f32, a: f32| {
        (0..64).map(|k| a + k as f32 * 0.4).map(|a| w.spawn + Vec3::new(a.cos(), 0.0, a.sin()) * d).find(|p| t.normal(p.x, p.z).y > 0.985).expect("a level spot")
    };

    let deep = level(30.0, 0.3);
    let raw = t.raw_height(deep.x, deep.z);
    dig(&mut s, deep, GroundFx { depth_m: 3.0, wide_m: 6.0, ..Default::default() });
    assert!((t.water(deep.x, deep.z) - (raw - 1.5)).abs() < 0.2, "up to the ground water: {} under {raw}", t.water(deep.x, deep.z));

    let shallow = level(30.0, 2.5);
    dig(&mut s, shallow, GroundFx { depth_m: 1.0, wide_m: 6.0, ..Default::default() });
    assert!(wet(shallow) < 0.0, "above it, dry: {}", wet(shallow));

    let pond = level(70.0, 4.5);
    dig(&mut s, pond, GroundFx { depth_m: 1.2, wide_m: 14.0, fill: true, ..Default::default() });
    assert!(wet(pond) > 0.5, "the pond holds water: {}", wet(pond));
    assert!(t.water(pond.x, pond.z) > t.raw_height(pond.x, pond.z) - 1.0, "to near its brim");
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let q = pond + Vec3::new(a.cos(), 0.0, a.sin()) * 14.0;
        assert!(wet(q) < 0.0, "none out over the land at {k}: {}", wet(q));
    }
}
