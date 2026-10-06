//! Remains, flinching and favouring a hurt: what a blow does to a body,
//! shown by the body itself and read from its shape, not drawn per weapon.

use super::*;
use crate::sim::remains::lowest_point;

/// The remains of a being, if it left any.
fn remains_of(s: &Session, cid: i64) -> Option<super::super::things::Thing> {
    s.sim.things.live().find(|t| t.origin.remains == Some(cid)).cloned()
}

/// How the remains lie: (lowest point above the ground, highest point
/// above it, centre's distance from `from` less the feet's).
fn lie(s: &Session, t: &super::super::things::Thing, from: Vec3) -> (f32, f32, f32) {
    let ty = s.sim.snap.type_of(t.type_id).unwrap().clone();
    let g = super::super::render::thing_inst(t, &ty, [0.0; 4]);
    let ground = s.sim.snap.terrain.height(t.pos.x, t.pos.z);
    let low = lowest_point(&g, &ty.ct).unwrap() - ground;
    // The highest point: the lowest of the shape turned upside down is
    // not to hand, so march down from above instead, coarsely.
    let c = g.center();
    let r = g.radius();
    let mut high = f32::MIN;
    for i in 0..15 {
        for j in 0..15 {
            let x = c.x + r * ((i as f32 + 0.5) / 15.0 * 2.0 - 1.0);
            let z = c.z + r * ((j as f32 + 0.5) / 15.0 * 2.0 - 1.0);
            let mut y = c.y + r;
            while y > c.y - r {
                if g.sdf(&ty.ct, Vec3::new(x, y, z)) < 0.01 {
                    high = high.max(y);
                    break;
                }
                y -= 0.02;
            }
        }
    }
    let flat = |p: Vec3| Vec3::new(p.x - from.x, 0.0, p.z - from.z).length();
    (low, high - ground, flat(c) - flat(t.pos))
}

/// Struck down with a sword, a person doesn't vanish: their body lies on
/// the ground where they fell, away from the blow, called by their name,
/// in their clothes, and a real thing the world goes on acting on.
#[test]
fn the_struck_down_leave_their_bodies() {
    let w = world_with("remains", 71, bare());
    let sword = add_type(&w, &fixture("physics/sword.js"));
    let p = flat_spot(&w, 30.0);
    let si = place(&w, sword, p + Vec3::new(0.5, 0.0, 0.0), 0.0);
    let cid = add_char(&w, "Bo", "timid", &[], p + Vec3::new(0.0, 0.0, 1.6));
    let mut s = session(&w, 71, None);
    calm(&mut s);
    s.sim.cfg.hunting = true;
    s.sim.player.pos = p;
    act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Instance(si) }, 0.5);
    for _ in 0..60 {
        if s.sim.cast.get(cid).unwrap().dead {
            break;
        }
        // Keep Bo in reach (a blow knocks them back; the timid run).
        s.sim.cast.get_mut(cid).unwrap().a.pos = ground(&w, p.x, p.z + 1.6);
        s.sim.cast.get_mut(cid).unwrap().a.task = None;
        act_once(&mut s, ActorId::Player, Action::Use { target: None, on: Some(Target::Actor(ActorId::Npc(cid))), at: None }, 1.2);
    }
    assert!(s.sim.cast.get(cid).unwrap().dead, "Bo was struck down");
    assert!(!events(&s.sim, "died").is_empty());
    let body = remains_of(&s, cid).expect("Bo's body stays");
    assert_eq!(s.sim.thing_name(body.id), "body of Bo");
    assert!(body.props[P_ALIVE] <= 0.0 && body.props[P_BODY] > 0.5, "a body, no longer alive");
    // It falls over, then lies still.
    s.run(1.5, 0.05);
    assert!(s.sim.falling(&remains_of(&s, cid).unwrap()).is_none(), "the fall is over");
    let body = remains_of(&s, cid).unwrap();
    assert!(body.tilt().angle_between(glam::Quat::IDENTITY) > 1.3, "lying down, not standing: {:.2}", body.tilt().angle_between(glam::Quat::IDENTITY));
    let (low, high, away) = lie(&s, &body, p);
    assert!(low.abs() < 0.06, "resting on the ground, not in it or above it: {low:.3} m");
    assert!(high < 0.75, "nothing of it stands up: top {high:.2} m");
    assert!(away > 0.3, "fell away from the blow: {away:.2} m");
    // A thing like any other: named, found by name, acted on by the rules.
    assert_eq!(s.sim.find_named("Bo", p, ActorId::Player), Some(Target::Thing(body.id)));
    assert_eq!(s.sim.find_named("the body", p, ActorId::Player), Some(Target::Thing(body.id)));
    // Knocked about and come to rest again, it still lies on the ground.
    if let Some(t) = s.sim.things.get_mut(body.id) {
        t.vel = Vec3::new(1.5, 1.0, 0.0);
        t.asleep = false;
    }
    s.run(4.0, 0.05);
    let moved = remains_of(&s, cid).unwrap();
    assert!(moved.asleep, "came to rest");
    let (low, _, _) = lie(&s, &moved, p);
    assert!(low.abs() < 0.08, "rests on the ground again: {low:.3} m");
    // It is still there after a save and a load.
    s.save();
    let s2 = session(&w, 72, None);
    let again = remains_of(&s2, cid).expect("the body was saved");
    assert!(again.tilt().angle_between(glam::Quat::IDENTITY) > 1.3 && s2.sim.thing_name(again.id) == "body of Bo");
    sound(&s);
}

/// Bodies of every build lie as their build does: a dog rolls onto its
/// side, its whole length on the ground.
#[test]
fn a_dog_rolls_onto_its_side() {
    let w = world("remains-dog", 73);
    let a = dry_spot(&w, 12.0, 0.4);
    let rex = add_being(&w, "Rex", "dog", &[], a);
    let mut s = session(&w, 73, None);
    calm(&mut s);
    let at = s.sim.cast.get(rex).unwrap().a.pos;
    let from = at + s.sim.cast.get(rex).unwrap().a.right() * -2.0;
    s.sim.player.pos = Vec3::new(from.x, s.sim.snap.terrain.height(from.x, from.z), from.z);
    s.sim.kill(rex, "struck down by a test", Some(ActorId::Player));
    let body = remains_of(&s, rex).expect("the dog's body stays");
    let mid = s.sim.falling(&body).expect("it goes over, not pops over");
    assert!(mid.tilt().angle_between(glam::Quat::IDENTITY) < 0.2, "just begun to fall");
    s.run(1.5, 0.05);
    let local_up = body.tilt() * Vec3::Y;
    assert!(local_up.y.abs() < 0.2 && local_up.x > 0.8, "on its side, away from the blow: up is {local_up:?}");
    let (low, high, _) = lie(&s, &body, from);
    assert!(low.abs() < 0.06, "on the ground: {low:.3} m");
    assert!(high < s.sim.cast.get(rex).unwrap().a.dims.height, "lower than it stood: {high:.2} m");
    assert!(s.sim.thing_name(body.id).contains("Rex"));
}

/// A blow rocks the body that takes it, for a moment; badly hurt, a body
/// goes slower and lurches, and in a day it has mended.
#[test]
fn a_blow_rocks_the_body_and_a_hurt_slows_it() {
    let w = world_with("flinch", 74, bare());
    let sword = add_type(&w, &fixture("physics/sword.js"));
    let p = flat_spot(&w, 30.0);
    let si = place(&w, sword, p + Vec3::new(0.5, 0.0, 0.0), 0.0);
    let cid = add_char(&w, "Ada", "brave", &[], p + Vec3::new(0.0, 0.0, 1.6));
    let mut s = session(&w, 74, None);
    calm(&mut s);
    s.sim.cfg.hunting = false;
    s.sim.player.pos = p;
    act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Instance(si) }, 0.5);
    s.sim.cast.get_mut(cid).unwrap().a.pos = ground(&w, p.x, p.z + 1.6);
    let me = ActorId::Player;
    s.sim.act(me, Action::Use { target: None, on: Some(Target::Actor(ActorId::Npc(cid))), at: None }).unwrap();
    for _ in 0..40 {
        if !events(&s.sim, "struck").is_empty() {
            break;
        }
        s.step(0.02);
    }
    assert!(!events(&s.sim, "struck").is_empty(), "the blow landed");
    let a = s.sim.actor(ActorId::Npc(cid)).unwrap().clone();
    let j = a.jolt.expect("the blow rocked Ada");
    assert!(j.dir.dot(a.pos - p) > 0.0, "away from the one who struck");
    s.step(0.06);
    let a = s.sim.actor(ActorId::Npc(cid)).unwrap().clone();
    let q = a.sway(s.sim.t, 0.0).expect("leaning with the blow");
    assert!(q.angle_between(glam::Quat::IDENTITY) > 0.05);
    assert!(s.sim.gait_factor(ActorId::Npc(cid)) < 0.5, "knocked off her step");
    s.run(1.0, 0.05);
    let a = s.sim.actor(ActorId::Npc(cid)).unwrap().clone();
    assert!(a.sway(s.sim.t, 0.0).is_none(), "and upright again");
    // Badly hurt: slower, stooped, and mending.
    let well = s.sim.gait_factor(ActorId::Npc(cid));
    s.sim.cast.get_mut(cid).unwrap().props[P_HEALTH] = 0.15;
    let hurt = s.sim.cast.get(cid).unwrap().hurt();
    assert!(hurt > 0.7, "{hurt}");
    assert!(s.sim.gait_factor(ActorId::Npc(cid)) < well * 0.7, "goes slower hurt");
    assert!(s.sim.actor(ActorId::Npc(cid)).unwrap().sway(s.sim.t, hurt).is_some(), "stooped");
    s.sim.feel_bodies(crate::render::sky::DAY_SECONDS as f32 * 0.5);
    let h = s.sim.cast.get(cid).unwrap().props[P_HEALTH];
    assert!(h > 0.6, "half a day mends most of it: {h:.2}");
    sound(&s);
}

/// The long dead are gone in time, once nobody is by; what they wore is left.
#[test]
fn old_remains_go_unseen() {
    let w = world("remains-old", 75);
    let a = dry_spot(&w, 12.0, 0.4);
    let ola = add_char(&w, "Ola", "calm", &[], a);
    let mut s = session(&w, 75, None);
    calm(&mut s);
    s.sim.kill(ola, "taken by a test", None);
    let id = remains_of(&s, ola).unwrap().id;
    s.sim.step_remains();
    assert!(s.sim.things.get(id).is_some_and(|t| !t.removed), "fresh remains stay");
    s.sim.things.get_mut(id).unwrap().born = s.sim.t - crate::render::sky::DAY_SECONDS * 4.0;
    s.sim.player.pos = a + Vec3::new(20.0, 0.0, 0.0);
    s.sim.step_remains();
    assert!(remains_of(&s, ola).is_some(), "not while the traveler is by");
    s.sim.player.pos = a + Vec3::new(200.0, 0.0, 0.0);
    s.sim.step_remains();
    assert!(remains_of(&s, ola).is_none(), "gone, unseen");
}

/// Strike `cid` with the sword the traveler holds until they die (or 60 tries).
fn strike_down(s: &mut Session, cid: i64, at: Vec3) {
    for _ in 0..60 {
        if s.sim.cast.get(cid).unwrap().dead {
            return;
        }
        let p = Vec3::new(at.x, s.sim.snap.terrain.height(at.x, at.z), at.z);
        s.sim.cast.get_mut(cid).unwrap().a.pos = p;
        s.sim.cast.get_mut(cid).unwrap().a.task = None;
        s.sim.cast.get_mut(cid).unwrap().plan.clear();
        act_once(s, ActorId::Player, Action::Use { target: None, on: Some(Target::Actor(ActorId::Npc(cid))), at: None }, 1.2);
    }
}

/// Fear comes from health going down at someone's hand: the struck fear
/// the striker (the timid more than the brave), those who watch a killing
/// fear the killer, and it fades over days.
#[test]
fn a_killing_seen_is_feared() {
    let w = world_with("fear", 77, bare());
    let sword = add_type(&w, &fixture("physics/sword.js"));
    let p = flat_spot(&w, 30.0);
    let si = place(&w, sword, p + Vec3::new(0.5, 0.0, 0.0), 0.0);
    let bo = add_char(&w, "Bo", "calm", &[], p + Vec3::new(0.0, 0.0, 1.6));
    let ada = add_char(&w, "Ada", "timid, nervous", &[], p + Vec3::new(6.0, 0.0, 6.0));
    let gus = add_char(&w, "Gus", "brave, bold soldier", &[], p + Vec3::new(-6.0, 0.0, 6.0));
    let far = add_char(&w, "Nell", "calm", &[], p + Vec3::new(120.0, 0.0, 0.0));
    let mut s = session(&w, 77, None);
    calm(&mut s);
    s.sim.cfg.hunting = true;
    for (c, d) in [(ada, Vec3::new(6.0, 0.0, 6.0)), (gus, Vec3::new(-6.0, 0.0, 6.0)), (far, Vec3::new(120.0, 0.0, 0.0))] {
        let q = p + d;
        s.sim.cast.get_mut(c).unwrap().a.pos = ground(&w, q.x, q.z);
    }
    s.sim.player.pos = p;
    act_once(&mut s, ActorId::Player, Action::Hold { target: Target::Instance(si) }, 0.5);
    let me = ActorId::Player;
    // One blow: Bo fears the traveler.
    act_once(&mut s, me, Action::Use { target: None, on: Some(Target::Actor(ActorId::Npc(bo))), at: None }, 1.2);
    assert!(s.sim.social.fear(ActorId::Npc(bo), me) > 0.1, "the struck fear the striker");
    assert_eq!(s.sim.social.fear(me, ActorId::Npc(bo)), 0.0, "one way: the striker doesn't fear the struck");
    strike_down(&mut s, bo, p + Vec3::new(0.0, 0.0, 1.6));
    assert!(s.sim.cast.get(bo).unwrap().dead, "Bo was killed");
    let (fa, fg, fn_) = (s.sim.social.fear(ActorId::Npc(ada), me), s.sim.social.fear(ActorId::Npc(gus), me), s.sim.social.fear(ActorId::Npc(far), me));
    assert!(fa > 0.4, "Ada saw the killing and fears the killer: {fa:.2}");
    assert!(fg > 0.05 && fg < fa, "the brave fear less: Gus {fg:.2}, Ada {fa:.2}");
    assert_eq!(fn_, 0.0, "Nell, far off, saw nothing");
    assert!(s.sim.cast.get(ada).unwrap().seen_dead.contains(&bo), "who saw the death needn't find the body");
    // Her planner and her talk are told.
    let ctx = s.sim.decide_context(ada, "test");
    assert!(ctx.contains("afraid") || ctx.contains("terrified"), "fear is in what she's told: {ctx}");
    assert!(ctx.contains("body of Bo"), "and the body is among what's around her");
    assert!(s.sim.fear_line(ada, me).is_some_and(|l| l.contains("sword")), "{:?}", s.sim.fear_line(ada, me));
    // The armed killer comes close: she runs.
    let ap = s.sim.actor(ActorId::Npc(ada)).unwrap().pos;
    s.sim.player.pos = ap + Vec3::new(2.0, 0.0, 0.0);
    s.sim.cast.get_mut(ada).unwrap().feared_at = f64::MIN;
    s.sim.step_fear();
    assert!(events(&s.sim, "fled").iter().any(|e| e.actor == Some(ActorId::Npc(ada))), "Ada runs from the armed killer");
    // Over days it fades.
    s.sim.fade_fear((crate::render::sky::DAY_SECONDS * 2.0) as f32);
    let later = s.sim.social.fear(ActorId::Npc(ada), me);
    assert!((later - fa).abs() > 0.1 && (later / fa - 0.5).abs() < 0.05, "halved in two days: {fa:.2} → {later:.2}");
    sound(&s);
}

/// Someone walking by a body they didn't know of stops, is shaken (more
/// for a friend than a stranger), remembers it, and isn't shaken again by
/// the same body. Animals don't take it in.
#[test]
fn a_body_is_come_upon() {
    let w = world("found", 78);
    let a = dry_spot(&w, 12.0, 0.4);
    let bo = add_char(&w, "Bo", "calm", &["Ola: dear friend"], a);
    let ola = add_char(&w, "Ola", "calm", &[], a + Vec3::new(60.0, 0.0, 0.0));
    let rex = add_being(&w, "Rex", "dog", &[], a + Vec3::new(0.0, 0.0, 60.0));
    let mut s = session(&w, 78, None);
    calm(&mut s);
    s.sim.social.rel_mut(ActorId::Npc(bo), ActorId::Npc(ola)).affection = 0.8;
    s.sim.player.pos = a + Vec3::new(0.0, 0.0, -100.0);
    s.sim.kill(bo, "taken by a test", None);
    s.sim.step_found_bodies();
    assert!(events(&s.sim, "found_body").is_empty(), "nobody near");
    let body = remains_of(&s, bo).unwrap().pos;
    for c in [ola, rex] {
        s.sim.cast.get_mut(c).unwrap().a.pos = body + Vec3::new(4.0, 0.0, 0.0);
    }
    s.sim.step_found_bodies();
    let found = events(&s.sim, "found_body");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].actor, Some(ActorId::Npc(ola)), "Ola found it; the dog doesn't take it in");
    assert!(found[0].data["weight"].as_f64().unwrap() > 0.8, "a friend: badly shaken");
    let n = s.sim.cast.get(ola).unwrap();
    assert!(n.news.iter().any(|(_, t, _)| t.contains("body of Bo")), "she remembers it");
    assert!(matches!(n.a.task, Some(super::super::actor::Task::Face { .. })), "she stops and stares");
    s.sim.step_found_bodies();
    assert_eq!(events(&s.sim, "found_body").len(), 1, "once per body");
}

/// A planner can choose to run from someone.
#[test]
fn flee_is_a_step() {
    let step = crate::sim::npc::parse_step(&serde_json::json!({ "do": "run_away", "target": "the traveler" })).unwrap();
    assert_eq!(step, Action::Flee { from: Target::Name("the traveler".into()) });
    let w = world("flee", 79);
    let a = dry_spot(&w, 12.0, 0.4);
    let ola = add_char(&w, "Ola", "calm", &[], a);
    let mut s = session(&w, 79, None);
    calm(&mut s);
    let start = s.sim.cast.get(ola).unwrap().a.pos;
    s.sim.player.pos = start + Vec3::new(2.0, 0.0, 0.0);
    s.sim.act(ActorId::Npc(ola), step).unwrap();
    s.run(4.0, 0.05);
    let now = s.sim.cast.get(ola).unwrap().a.pos;
    assert!((now - s.sim.player.pos).length() > (start - s.sim.player.pos).length() + 4.0, "she got away");
}
