//! Up off the ground: a hut set on a great tree's platform, a walkway
//! between two trees' platforms, a ladder up one of them; a villager on the
//! ground climbs, crosses and goes in, and comes down again.

use super::*;
use crate::model::{RegionCommit, Settlement};

struct Village {
    a: Vec3,
    b: Vec3,
    ok: crate::model::CommitOk,
    hut: u32,
}

fn tree_village(w: &W) -> Village {
    let tree = add_type(w, &fixture("physics/great_tree.js"));
    let bare = add_type(w, &fixture("physics/great_tree_bare.js"));
    let hut = add_type(w, &fixture("physics/tree_hut.js"));
    let a = flat_spot(w, 70.0);
    let b = a + Vec3::new(14.0, 0.0, 0.0);
    let gb = w.terrain.height(b.x, b.z);
    let up = crate::model::Placement { y: Some(gb + 8.0), rot_y: -std::f32::consts::FRAC_PI_2, ..pl(hut, b, 0.0) };
    let r = crate::world::region_of(a.x, a.z);
    let walk = serde_json::json!([{ "from": [3.3, 0.0, 8.0], "to": [10.7, 0.0, 8.0], "width": 1.4, "look": "timber" }]);
    let st = Settlement { x: a.x, z: a.z, road: None, walkways: crate::settle::parse_walkways(walk.as_array().unwrap(), a.x, a.z), ..Default::default() };
    let region = RegionCommit { r, plan_json: "{}".into(), info: Default::default(), characters: vec![], settlement: Some(st) };
    let ok = commit_places(w, vec![pl(tree, a, 0.0), pl(bare, b, 0.0), up], Some(region));
    Village { a, b, ok, hut }
}

#[test]
fn a_village_up_in_the_trees() {
    let w = world_with("trees", 75, bare());
    let v = tree_village(&w);
    let snap = &v.ok.snapshot;
    let t = &snap.terrain;
    let deck = t.height(v.b.x, v.b.z) + 8.0;
    // The hut stands on the platform.
    let hut = snap.instances.iter().find(|p| p.type_id == v.hut).expect("the hut is placed");
    assert!((hut.pos.y - deck).abs() < 0.2, "on the platform: {} vs {deck}", hut.pos.y);
    // The walkway joins the platforms, level with them.
    let walkway = builtin_id(&w, "walkway");
    let spans: Vec<&crate::world::Placed> = snap.instances.iter().filter(|p| p.type_id == walkway).collect();
    assert!(!spans.is_empty(), "a walkway is laid");
    for s in &spans {
        assert!((s.pos.y - deck).abs() < 0.3 && s.pos.x > v.a.x + 3.0 && s.pos.x < v.b.x - 3.0, "between the platforms at their height: {}", s.pos);
    }

    // A villager on the ground goes into the hut: up the ladder, over the walkway, in.
    let ola = add_char(&w, "Ola", "kind", &[], v.a + Vec3::new(3.0, 0.0, 9.0));
    let mut s = session(&w, 75, None);
    calm(&mut s);
    let o = ActorId::Npc(ola);
    let inside = Vec3::new(v.b.x - 0.3, deck, v.b.z);
    s.sim.cast.get_mut(ola).unwrap().plan.push_back(Action::Goto { target: Target::Point(inside.to_array()), run: false });
    let (mut climbed, mut crossed) = (false, false);
    for _ in 0..1500 {
        s.step(0.1);
        let a = s.sim.actor(o).unwrap();
        climbed |= a.ladder.is_some();
        crossed |= a.pos.x > v.a.x + 5.0 && a.pos.x < v.b.x - 5.0 && a.pos.y > deck - 0.5;
    }
    let at = s.sim.actor(o).unwrap().pos;
    assert!(climbed && crossed, "up the ladder ({climbed}) and over the walkway ({crossed}); at {at}");
    assert!(Vec3::new(at.x - inside.x, 0.0, at.z - inside.z).length() < 1.2 && (at.y - deck).abs() < 0.4, "in the hut: {at} vs {inside}");
    // And back down to the ground.
    let down = Vec3::new(v.a.x - 6.0, 0.0, v.a.z + 6.0);
    let down = Vec3::new(down.x, t.height(down.x, down.z), down.z);
    s.sim.cast.get_mut(ola).unwrap().plan.push_back(Action::Goto { target: Target::Point(down.to_array()), run: false });
    s.run(150.0, 0.1);
    let at = s.sim.actor(o).unwrap().pos;
    assert!((at - down).length() < 2.0, "down again: {at} vs {down}");
}

/// Pictures of the tree village (look at them): from the ground, and from
/// the walkway.
#[test]
#[ignore]
fn tree_village_png() {
    let dir = std::env::var("POCKET_PNG_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let w = world_with("treespng", 75, bare());
    let v = tree_village(&w);
    let mut s = session(&w, 75, None);
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
    let mid = (v.a + v.b) * 0.5;
    let g = t.height(mid.x, mid.z);
    png_of(&mut s, &snap, &gpu, look(Vec3::new(mid.x - 4.0, g + 1.7, mid.z + 22.0), Vec3::new(mid.x, g + 6.0, mid.z)), &format!("{dir}/pocket-trees.png"));
    png_of(&mut s, &snap, &gpu, look(Vec3::new(v.a.x + 2.5, g + 9.6, v.a.z + 0.3), Vec3::new(v.b.x, g + 9.0, v.b.z)), &format!("{dir}/pocket-trees-walkway.png"));
}
