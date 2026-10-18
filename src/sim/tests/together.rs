//! Other travelers in the world (playing together): the world lives around
//! them too, the dark hunts them, they fall and get up, and what they carry
//! waits for them while they are away.

use super::*;

/// Who someone touched lately.
fn touched(s: &Session, who: ActorId) -> bool {
    events(&s.sim, "touched").iter().any(|e| e.subject.as_deref() == Some(who.key().as_str()))
}

#[test]
fn people_near_a_guest_live_however_far_the_host_is() {
    let w = world("together near", 21);
    let p = dry_spot(&w, 30.0, 0.5);
    let ada = add_char(&w, "Ada", "calm", &[], p + Vec3::new(2.0, 0.0, 0.0));
    let mut s = session(&w, 21, None);
    s.sim.player.pos = p + Vec3::new(600.0, 0.0, 600.0);
    run_for(&mut s, 1.0);
    let t0 = s.sim.cast.get(ada).unwrap().last_sim;
    assert!(s.sim.t - t0 > 0.5, "far from everyone she waits");
    s.sim.add_traveler(1, "Bea", &Default::default(), p, 0.0);
    run_for(&mut s, 1.0);
    assert!(s.sim.t - s.sim.cast.get(ada).unwrap().last_sim < 0.3, "by Bea she lives");
    assert!(s.sim.dist_to_travelers(p) < 0.01);
}

#[test]
fn the_dark_hunts_a_guest_who_falls_and_gets_up() {
    let w = world("together dark", 7);
    let mut s = session(&w, 7, None);
    s.sim.cfg.difficulty = 1;
    s.sim.player.pos = w.spawn;
    s.sim.t = before_dusk(1.0, 2.0);
    run_for(&mut s, 4.0);
    let dark = the_dark(&s);
    assert!(!dark.is_empty() && s.sim.dark());
    for id in s.sim.cast.npcs.iter().filter(|n| n.species.hostile).map(|n| n.def.id).collect::<Vec<_>>() {
        s.sim.cast.get_mut(id).unwrap().a.pos = w.spawn + Vec3::new(500.0, 0.0, 500.0);
        s.sim.cast.get_mut(id).unwrap().think_at = f64::MAX;
    }
    // Bea is out alone, far from the host, looking away from it.
    let bea = ground(&w, w.spawn.x + 90.0, w.spawn.z);
    s.sim.add_traveler(1, "Bea", &Default::default(), bea, 0.0);
    let n = s.sim.cast.get_mut(dark[0]).unwrap();
    n.a.pos = ground(&w, bea.x, bea.z - 12.0);
    n.think_at = 0.0;
    let mut reached = false;
    for _ in 0..(90.0 / 0.1) as usize {
        s.step(0.1);
        if touched(&s, ActorId::Traveler(1)) {
            reached = true;
            break;
        }
    }
    assert!(reached, "it went for Bea: {:?}", s.sim.cast.get(dark[0]).map(|n| (n.doing.clone(), (n.a.pos - bea).length())));
    assert!(!touched(&s, ActorId::Player), "not the host, far off");
    let t = &s.sim.travelers[&1];
    assert!(t.wounds > 0.2 && s.sim.health() == 1.0, "Bea is hurt ({:.2}), the host not", t.wounds);
    // Struck down: she falls, lets go, and gets up at the beginning a little later.
    let stone = builtin_id(&w, "stone");
    let st = s.sim.spawn_thing(stone, bea + Vec3::Y * 0.3, 0.0, 1.0, Default::default(), true).unwrap();
    s.sim.hand_to(ActorId::Traveler(1), st);
    assert!(s.sim.wound(ActorId::Traveler(1), 1.0, ActorId::Npc(dark[0]), "clawed"));
    assert!(s.sim.traveler_fallen(ActorId::Traveler(1)) && s.sim.travelers[&1].a.held.is_none());
    assert!(s.sim.things.get(st).is_some_and(|t| t.holder.is_none()), "the stone fell from her hand");
    assert!(!s.sim.fallen(), "the host is still up");
    let before = s.sim.t;
    run_for(&mut s, super::super::travelers::GET_UP_AFTER as f32 + 1.0);
    let t = &s.sim.travelers[&1];
    assert!(t.fallen.is_none() && t.wounds == 0.0, "up again, whole");
    assert!((Vec3::new(t.a.pos.x, 0.0, t.a.pos.z) - Vec3::new(w.spawn.x, 0.0, w.spawn.z)).length() < 0.5, "at the world's beginning");
    assert!(s.sim.t - before < 60.0, "no night skipped for everyone else");
}

#[test]
fn what_a_guest_carries_waits_for_them() {
    let w = world("together carry", 23);
    let p = dry_spot(&w, 20.0, 1.0);
    let stone = builtin_id(&w, "stone");
    let (st, hat) = {
        let mut s = session(&w, 23, None);
        s.sim.add_traveler(2, "Cy", &Default::default(), p, 0.0);
        let st = s.sim.spawn_thing(stone, p + Vec3::Y * 0.3, 0.0, 1.0, Default::default(), true).unwrap();
        let hat = s.sim.spawn_thing(stone, p + Vec3::Y * 0.3, 0.0, 0.3, Default::default(), true).unwrap();
        s.sim.hand_to(ActorId::Traveler(2), st);
        s.sim.things.get_mut(hat).unwrap().worn = Some(ActorId::Traveler(2));
        run_for(&mut s, 0.5);
        // Cy leaves: what they hold and wear goes with them.
        s.sim.remove_traveler(2);
        assert!(s.sim.things.get(st).is_none() && s.sim.things.get(hat).is_none(), "gone with Cy");
        assert_eq!(s.sim.stowed[&2].len(), 2);
        run_for(&mut s, 0.5);
        crate::sim::persist::save(&mut s.sim);
        (st, hat)
    };
    // The world opened again, without Cy: still kept for them.
    let mut s = session(&w, 23, None);
    assert!(s.sim.things.get(st).is_none(), "not lying about while Cy is away");
    s.sim.add_traveler(2, "Cy", &Default::default(), p + Vec3::new(1.0, 0.0, 0.0), 0.0);
    assert_eq!(s.sim.travelers[&2].a.held, Some(st), "back in Cy's hand");
    assert_eq!(s.sim.things.get(st).and_then(|t| t.holder), Some(ActorId::Traveler(2)));
    assert_eq!(s.sim.things.get(hat).and_then(|t| t.worn), Some(ActorId::Traveler(2)), "and still worn");
    run_for(&mut s, 0.5);
    assert!(s.sim.check_invariants().is_empty(), "{:?}", s.sim.check_invariants());
}

#[test]
fn a_guest_rides_a_horse_and_drives_a_cart() {
    let w = world_with("together ride", 68, bare());
    let cart = add_type(&w, &fixture("vehicles/cart.js"));
    let a = flat_spot(&w, 30.0);
    let ci = place(&w, cart, a, 0.0);
    let horse = add_being(&w, "Bram", "horse", &[], a + Vec3::new(10.0, 0.0, 0.0));
    let mut s = session(&w, 68, None);
    calm(&mut s);
    let bea = ActorId::Traveler(1);
    // The horse knows her: she rides it.
    let hb = ActorId::Npc(horse);
    s.sim.add_traveler(1, "Bea", &Default::default(), s.sim.actor(hb).unwrap().pos + Vec3::new(1.2, 0.0, 0.0), 0.0);
    s.sim.social.bond(hb, bea, 0.6, s.sim.t);
    let r = s.sim.act(bea, Action::Ride { target: Target::Actor(hb) });
    assert!(r.is_ok(), "{r:?}");
    let start = s.sim.actor(hb).unwrap().pos;
    for _ in 0..30 {
        s.sim.drive_as(bea, Vec3::new(0.0, 0.0, 1.0), 0.0, 0.1);
        s.step(0.1);
    }
    let hp = s.sim.actor(hb).unwrap().pos;
    assert!((hp - start).length() > 5.0, "the horse carried her ({:.1} m)", (hp - start).length());
    let me = s.sim.travelers[&1].a.pos;
    assert!(Vec3::new(me.x - hp.x, 0.0, me.z - hp.z).length() < 1.0, "on its back");
    s.sim.act(bea, Action::Dismount).unwrap();
    // The cart: in the driver's seat, she drives it.
    s.sim.travelers.get_mut(&1).unwrap().a.pos = ground(&w, a.x + 1.5, a.z);
    s.sim.act(bea, Action::Ride { target: Target::Instance(ci) }).unwrap();
    let id = s.sim.travelers[&1].a.aboard.expect("in the cart");
    let p0 = s.sim.things.get(id).unwrap().pos;
    for _ in 0..40 {
        s.sim.steer(bea, 1.0, 0.0, 0.0, 0.1);
        s.step(0.1);
    }
    assert!((s.sim.things.get(id).unwrap().pos - p0).length() > 0.5, "the cart moved");
    assert!((s.sim.travelers[&1].a.pos - s.sim.things.get(id).unwrap().pos).length() < 3.0, "with her in it");
}

#[test]
fn what_a_guest_makes_is_theirs() {
    let w = world("together made", 25);
    let mut s = session(&w, 25, None);
    let p = s.sim.player.pos;
    s.sim.add_traveler(1, "Bea", &Default::default(), p, 0.0);
    s.sim.note_creation("type:1", "made", "kite", Some(ActorId::Traveler(1)), "", p);
    let rows = w.db.with(|c| db::creations(c)).unwrap();
    assert_eq!(rows.iter().find(|r| r.key == "type:1").map(|r| r.made_by.as_str()), Some("Bea"), "made by Bea, not the traveler");
}
