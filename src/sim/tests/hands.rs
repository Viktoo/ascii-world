//! The traveler's own body: their look is kept with the world, their hands
//! move with what they do, and from inside only arms at work are in view.

use super::*;
use crate::sim::actor::{GRIP, GestureKind};
use crate::sim::interp::BeingFx;

/// How far below the middle of the view a point is (radians), and whether
/// it is inside the view at all (a 16:9 view of `fov_y`).
fn in_view(cam: &crate::render::Camera, p: Vec3) -> (f32, bool) {
    let (f, r, u) = cam.basis();
    let d = p - cam.pos;
    let z = d.dot(f);
    let below = (-d.dot(u)).atan2(z);
    let side = d.dot(r).atan2(z);
    let half = cam.fov_y * 0.5;
    let wide = (half.tan() * 16.0 / 9.0).atan();
    (below, z > 0.0 && below.abs() < half && side.abs() < wide)
}

fn eye_cam(s: &Session) -> crate::render::Camera {
    let p = s.sim.player.pos + Vec3::Y * s.sim.player.dims.eye;
    crate::render::Camera { pos: p, yaw: s.sim.player.yaw, pitch: 0.0, fov_y: 1.05, roll: 0.0 }
}

#[test]
fn the_travelers_look_is_kept_with_the_world() {
    let w = world("ownlook", 41);
    let mut s = session(&w, 1, None);
    let body = s.sim.own_body_type().unwrap().ct.meta.body.clone().unwrap();
    let skin = body.slider("skin").expect("people have a skin slider");
    let before = s.sim.own_sliders()[skin];
    let mut fx = BeingFx { on_self: true, ..Default::default() };
    fx.look.insert("skin".into(), 0.9);
    s.sim.apply_being(ActorId::Player, ActorId::Player, &fx);
    assert!((s.sim.own_sliders()[skin] - 0.9).abs() < 1e-4, "was {before}");
    // A new session on the same world still has it.
    let s2 = session(&w, 2, None);
    assert!((s2.sim.own_sliders()[skin] - 0.9).abs() < 1e-4);
    // Out-of-range wishes keep to the body's range.
    fx.look.insert("skin".into(), 7.0);
    s.sim.apply_being(ActorId::Player, ActorId::Player, &fx);
    assert!(s.sim.own_sliders()[skin] <= 1.0);
}

#[test]
fn hands_move_with_what_is_done() {
    let w = world("ownhands", 42);
    let stone = builtin_id(&w, "stone");
    let mut s = session(&w, 3, None);
    let me = ActorId::Player;
    assert_eq!(s.sim.hand_move(me, &Action::Do { text: "punch the door".into(), on: None, at: None }), Some(GestureKind::Punch));
    assert_eq!(s.sim.hand_move(me, &Action::Do { text: "sing to the sky".into(), on: None, at: None }), Some(GestureKind::Touch));
    let p = s.sim.player.pos;
    let id = s.sim.spawn_thing(stone, p + Vec3::new(0.3, 0.0, 0.3), 0.0, 1.0, Default::default(), true).unwrap();
    let r = act_once(&mut s, me, Action::Hold { target: Target::Thing(id) }, 0.0);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(s.sim.player.gesture.as_ref().map(|g| g.kind), Some(GestureKind::Grab), "reaches to take it");
    // A full hand doesn't punch: it would be a blow with what it holds.
    assert_eq!(s.sim.hand_move(me, &Action::Do { text: "punch the door".into(), on: None, at: None }), Some(GestureKind::Touch));
    for _ in 0..20 {
        s.sim.step(0.1);
    }
    assert!(s.sim.player.pose[GRIP] > 0.5, "the hand closes on it: {:?}", s.sim.player.pose);
}

#[test]
fn from_inside_only_arms_at_work_are_seen() {
    let w = world("ownview", 43);
    let stone = builtin_id(&w, "stone");
    let mut s = session(&w, 4, None);
    for _ in 0..10 {
        s.sim.step(0.1);
    }
    let cam = eye_cam(&s);
    let v = s.sim.own_view(&cam).expect("the traveler has a body");
    let (below, seen) = in_view(&cam, v.hand(false));
    assert!(!seen, "an arm at rest is out of sight ({below:.2} rad below)");
    let p = s.sim.player.pos;
    let id = s.sim.spawn_thing(stone, p + Vec3::new(0.3, 0.0, 0.3), 0.0, 1.0, Default::default(), true).unwrap();
    act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Thing(id) }, 2.0);
    let cam = eye_cam(&s);
    let v = s.sim.own_view(&cam).unwrap();
    let (below, seen) = in_view(&cam, v.hand(false));
    assert!(seen && below > 0.1, "a hand holding something is low in view ({below:.2} rad below)");
    // What it holds is drawn by that hand, and so is the body: its head
    // and chest are cut away about the eye.
    let drawn = s.sim.draw_first_person(&cam, crate::render::VIEW_DIST);
    let body = drawn.insts.iter().find(|g| g.cuts[0][3] > 0.0).expect("the body is drawn");
    let ty = s.sim.own_body_type().unwrap().clone();
    assert!(body.sdf(&ty.ct, cam.pos) > 0.0, "the eye is not inside the body");
    let held = drawn.insts.iter().find(|g| g.info[0] == stone).unwrap();
    assert!((held.center() - v.hand(false)).length() < 0.3, "in the hand");
}
