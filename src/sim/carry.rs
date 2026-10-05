//! Hands-on dealings with creatures: petting them, picking up the small
//! ones (who may like it, squirm free after a while, dart off or nip), and
//! what the use key means for a being in front of you. Everything reads the
//! body, temper and how the animal feels about you, never a species' name.

use super::actions::{ActErr, Action, Outcome};
use super::actor::{GestureKind, GestureRun, Task};
use super::props::P_EDIBLE;
use super::{ActorId, Note, Sim, Target};
use crate::world::species::Mind;
use glam::Vec3;
use serde_json::json;

/// Of what a body can lift, how much of a live, wriggling animal it carries.
const CARRY_SHARE: f32 = 0.8;
/// How close a body must be to touch another (beyond its reach and their side).
const TOUCH_GAP: f32 = 0.3;

/// What the use key does to a being in front of you.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Approach {
    /// Stroke an animal close by.
    Pet,
    /// Climb onto a mount that lets you.
    Ride,
    /// An animal further off: call it.
    Call,
    /// A person: a nod or a handshake close by, a wave further off.
    Greet(GestureKind),
    /// Hand over what you hold (food to an animal is feeding it).
    Give,
    /// Work the tool you hold on them.
    Strike,
}

fn fail<T>(s: impl Into<String>) -> Result<T, ActErr> {
    Err(ActErr::Fail(s.into()))
}

impl Sim {
    /// The being `who` carries in their arms, if any.
    pub fn carried_of(&self, who: ActorId) -> Option<ActorId> {
        self.actor_ids().into_iter().find(|a| self.actor(*a).is_some_and(|x| x.carried_by == Some(who)))
    }

    /// Ground distance between two bodies' sides.
    fn gap(&self, a: ActorId, b: ActorId) -> f32 {
        let (Some(x), Some(y)) = (self.actor(a), self.actor(b)) else { return f32::MAX };
        (Vec3::new(y.pos.x - x.pos.x, 0.0, y.pos.z - x.pos.z).length() - y.dims.radius).max(0.0)
    }

    /// Whether `who` can touch `other` (pet them, pick them up).
    pub fn within_touch(&self, who: ActorId, other: ActorId) -> bool {
        self.gap(who, other) <= self.reach_of(who) + TOUCH_GAP
    }

    /// How much an animal is willing to let `who` handle it: its liking
    /// for them, less its wariness. A person's is just their liking.
    fn ease_with(&self, other: ActorId, who: ActorId) -> f32 {
        let wary = match other {
            ActorId::Npc(c) => self.wariness(c),
            _ => 0.0,
        };
        self.social.affection(other, who) - wary
    }

    /// Whether `mount` would carry `who` now (as `ride` checks it).
    pub fn ride_ok(&self, who: ActorId, mount: ActorId) -> bool {
        let (Some(me), Some(m)) = (self.actor(who), self.actor(mount)) else { return false };
        if m.dims.seat.is_none() || me.seated() || self.rider_of(mount).is_some() || m.asleep {
            return false;
        }
        if m.dims.mass < me.dims.mass * 3.0 || me.dims.mass > self.strength(mount) * 1.6 {
            return false;
        }
        let tame = self.temper_of(mount).map(|t| t.tame).unwrap_or(0.5);
        self.ease_with(mount, who) >= 0.25 && tame >= 0.2
    }

    /// What the use key does to `other` for `who`, by what is in hand, how
    /// close they are, and what the other is.
    pub fn approach(&self, who: ActorId, other: ActorId) -> Approach {
        if self.actor(who).and_then(|a| a.held).is_some() {
            return if self.held_tool(who).is_some() { Approach::Strike } else { Approach::Give };
        }
        if self.carried_of(who) == Some(other) {
            return Approach::Pet;
        }
        let close = self.within_touch(who, other);
        if self.mind_of(other) != Mind::Sapient {
            return if close && self.ride_ok(who, other) {
                Approach::Ride
            } else if close {
                Approach::Pet
            } else {
                Approach::Call
            };
        }
        if self.gap(who, other) > 2.5 {
            return Approach::Greet(GestureKind::Wave);
        }
        let known = self.social.rel(who, other).is_some_and(|r| r.familiarity >= 0.3) && self.social.affection(who, other) > 0.2;
        Approach::Greet(if known { GestureKind::Handshake } else { GestureKind::Nod })
    }

    /// The approach as words for the key hint ("pet", "feed the apple to").
    pub fn approach_words(&self, who: ActorId, other: ActorId) -> String {
        let held = self.actor(who).and_then(|a| a.held).map(|h| (self.thing_name(h), self.things.get(h).is_some_and(|t| t.props[P_EDIBLE] > 0.0)));
        match self.approach(who, other) {
            Approach::Pet => "pet".into(),
            Approach::Ride => "ride".into(),
            Approach::Call => "call".into(),
            Approach::Greet(GestureKind::Handshake) => "shake hands".into(),
            Approach::Greet(GestureKind::Nod) => "nod".into(),
            Approach::Greet(_) => "wave".into(),
            Approach::Give => match held {
                Some((n, true)) if self.mind_of(other) != Mind::Sapient => format!("feed the {n}"),
                Some((n, _)) => format!("give the {n}"),
                None => "give".into(),
            },
            Approach::Strike => format!("use the {}", held.map(|h| h.0).unwrap_or_default()),
        }
    }

    /// The action an approach means (None for a call, which isn't one).
    pub fn approach_action(&self, who: ActorId, other: ActorId, at: Option<[f32; 3]>) -> Option<Action> {
        let to = Target::Actor(other);
        Some(match self.approach(who, other) {
            Approach::Pet => Action::Pet { target: to },
            Approach::Ride => Action::Ride { target: to },
            Approach::Call => return None,
            Approach::Greet(k) => Action::Gesture { kind: k.name(), to: Some(to) },
            Approach::Give => Action::Give { to },
            Approach::Strike => Action::Use { target: None, on: Some(to), at },
        })
    }

    // ------------------------------------------------------------ petting

    /// Stroke an animal. One that is easy with you leans into it, makes a
    /// happy noise and likes you more; a wary one shies away.
    pub fn pet(&mut self, who: ActorId, other: ActorId) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let oname = self.actor_name(other);
        if who == other {
            return fail(format!("{name} can't pet themselves"));
        }
        let ActorId::Npc(c) = other else { return fail(format!("{oname} isn't an animal; try a hug")) };
        if self.mind_of(other) == Mind::Sapient {
            return fail(format!("{oname} isn't an animal; try a hug or a handshake"));
        }
        let Some(op) = self.actor(other).map(|a| a.pos) else { return fail("no such one") };
        let held = self.carried_of(who) == Some(other);
        if !held && !self.within_touch(who, other) {
            return Err(ActErr::TooFar { at: op, dist: self.gap(who, other) });
        }
        let t = self.t;
        if let Some(a) = self.actor_mut(who) {
            a.face(op - a.pos, 10.0);
            a.gesture = Some(GestureRun { kind: GestureKind::Pet, with: Some(other), t0: t, dur: GestureKind::Pet.duration() });
        }
        let woke = self.actor(other).is_some_and(|a| a.asleep);
        let ease = self.ease_with(other, who);
        let me = self.actor(who).map(|a| a.pos).unwrap_or(op);
        if ease < -0.25 && !held {
            // Not from you: it shies off, with a hiss or a growl.
            self.social.rel_mut(other, who).affection -= 0.02;
            self.make_noise(c, false);
            let away = (op - me).normalize_or_zero() * 4.0;
            self.plan(other, vec![Action::Goto { target: Target::Point((op + away).to_array()), run: true }], &format!("keep away from {name}"), false);
            self.set_doing(c, &format!("shying away from {name}"));
            self.event("refused", Some(other), Some(who.key()), format!("{oname} shied away from {name}'s hand"), Some(op), json!({ "pet": true }));
            return fail(format!("{oname} shies away from {name}'s hand"));
        }
        if let Some(n) = self.cast.get_mut(c) {
            n.a.asleep = false;
            n.needs.social = (n.needs.social - 0.25).max(0.0);
            n.think_at = t + 5.0;
            if !held {
                n.plan.clear();
                n.a.face(me - op, 10.0);
                n.a.task = Some(Task::Face { target: Target::Actor(who), until: t + 4.0 });
            }
        }
        // Liked more the less it is liked yet; a sleepy one is less pleased.
        let gain = if woke { 0.02 } else { 0.05 * (1.2 - ease.max(0.0)).clamp(0.3, 1.0) };
        self.social.bond(other, who, gain, t);
        self.make_noise(c, true);
        let _ = self.act(other, Action::Gesture { kind: "wag".into(), to: None });
        self.set_doing(c, &format!("being petted by {name}"));
        let msg = format!("{name} pets {oname}");
        self.event("petted", Some(who), Some(other.key()), msg.clone(), Some(op), json!({}));
        if who != ActorId::Player {
            self.note_near(op, 20.0, Note::Ambient(format!("{}.", super::physics::cap(&msg))));
        }
        Ok(Outcome::ok(msg))
    }

    // ------------------------------------------------------------ carrying

    /// Pick up a small animal. One that trusts you settles in your arms
    /// for a good while; one that doesn't is caught only if it is slow or
    /// asleep, squirms free in a few seconds and runs; a quick one darts
    /// off before you get hold of it; a fierce one may nip.
    pub fn pick_up(&mut self, who: ActorId, other: ActorId) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let oname = self.actor_name(other);
        let ActorId::Npc(c) = other else { return fail(format!("{name} can't pick up {oname}")) };
        if self.mind_of(other) == Mind::Sapient {
            return fail(format!("{name} can't pick up {oname}; try a hug"));
        }
        let Some((me, o)) = self.actor(who).cloned().zip(self.actor(other).cloned()) else { return fail("no such one") };
        if !me.dims.arms {
            return fail(format!("{name} has no arms to carry {oname} in"));
        }
        if let Some(h) = me.held {
            return fail(format!("hands are full: drop the {} first", self.thing_name(h)));
        }
        if let Some(x) = self.carried_of(who) {
            return fail(format!("arms are full: put {} down first", self.actor_name(x)));
        }
        if o.dims.mass > self.strength(who) * CARRY_SHARE {
            return fail(format!("{oname} is too big to pick up ({:.0} kg)", o.dims.mass));
        }
        if o.carried() || self.rider_of(other).is_some() {
            return fail(format!("{name} can't get hold of {oname} now"));
        }
        if !self.within_touch(who, other) {
            return Err(ActErr::TooFar { at: o.pos, dist: self.gap(who, other) });
        }
        let ease = self.ease_with(other, who);
        let temper = self.temper_of(other).unwrap_or_default();
        let willing = ease >= 0.3 && temper.tame >= 0.25;
        let t = self.t;
        if !willing && !o.asleep {
            let quick = self.speeds(other).1 > self.speeds(who).0 * 0.9;
            let fierce = self.cast.get(c).is_some_and(|n| n.species.diet.meat >= 0.5) || temper.bold > 0.65;
            let nips = fierce && self.cast.get_mut(c).is_some_and(|n| n.rand() < 0.6);
            if quick || nips {
                let r = self.social.rel_mut(other, who);
                r.trust = (r.trust - 0.05).max(-1.0);
                r.affection -= 0.03;
                self.make_noise(c, false);
                self.run_from(c, &name, Some(who.key()), me.pos);
                let (kind, msg) = if nips { ("bit", format!("{oname} nips {name}'s hand and wriggles away")) } else { ("dodged", format!("{oname} darts out of {name}'s reach")) };
                self.event(kind, Some(other), Some(who.key()), msg.clone(), Some(o.pos), json!({}));
                self.note_near(o.pos, 20.0, Note::seen(format!("{}.", super::physics::cap(&msg)), who == ActorId::Player));
                return fail(msg);
            }
        }
        // How long it stays: one that likes you settles; one that doesn't
        // only until it gets its legs free (a sleeping one wakes first).
        let stay = if willing { 40.0 + 80.0 * ease.min(1.0) } else { 2.0 + 3.0 * temper.tame + if o.asleep { 3.0 } else { 0.0 } };
        if let Some(n) = self.cast.get_mut(c) {
            n.a.carried_by = Some(who);
            n.a.carried_until = t + stay as f64;
            n.a.asleep = false;
            n.a.task = None;
            n.a.gesture = None;
            n.a.vy = 0.0;
            n.plan.clear();
            n.think_at = t + 1.0;
        }
        self.update_riders();
        let msg = if willing { format!("{name} picks up {oname}") } else { format!("{name} scoops up {oname}, who squirms") };
        if willing {
            self.make_noise(c, true);
            self.social.bond(other, who, 0.02, t);
            self.set_doing(c, &format!("in {name}'s arms"));
        } else {
            self.make_noise(c, false);
            let r = self.social.rel_mut(other, who);
            r.trust = (r.trust - 0.05).max(-1.0);
            self.set_doing(c, &format!("squirming in {name}'s arms"));
        }
        self.event("picked_up", Some(who), Some(other.key()), msg.clone(), Some(o.pos), json!({ "willing": willing }));
        if who != ActorId::Player {
            self.note_near(o.pos, 15.0, Note::Ambient(format!("{}.", super::physics::cap(&msg))));
        }
        Ok(Outcome::ok(msg))
    }

    /// Put down the animal `who` carries, in front of them.
    pub fn set_down(&mut self, who: ActorId) -> Result<Outcome, ActErr> {
        let Some(other) = self.carried_of(who) else { return fail("not carrying anyone") };
        let ActorId::Npc(c) = other else { return fail("not carrying anyone") };
        let name = self.actor_name(who);
        let oname = self.actor_name(other);
        self.let_go(c, false);
        let at = self.actor(other).map(|a| a.pos);
        let msg = format!("{name} sets {oname} down");
        self.event("set_down", Some(who), Some(other.key()), msg.clone(), at, json!({}));
        Ok(Outcome::ok(msg))
    }

    /// Hand the animal you carry to someone with arms (one it doesn't know
    /// well won't stay long with them).
    pub fn hand_over(&mut self, who: ActorId, to: ActorId) -> Result<Outcome, ActErr> {
        let Some(other) = self.carried_of(who) else { return fail("not carrying anyone") };
        let ActorId::Npc(c) = other else { return fail("not carrying anyone") };
        let (name, oname, tname) = (self.actor_name(who), self.actor_name(other), self.actor_name(to));
        if to == other {
            return self.set_down(who);
        }
        let Some(a) = self.actor(to).cloned() else { return fail("give them to whom?") };
        if !a.dims.arms || a.held.is_some() || self.carried_of(to).is_some() {
            return fail(format!("{tname} has no free arms"));
        }
        if self.gap(who, to) > self.reach_of(who) + 0.6 {
            return Err(ActErr::TooFar { at: a.pos, dist: self.gap(who, to) });
        }
        let ease = self.ease_with(other, to);
        let t = self.t;
        let stay = if ease >= 0.3 { 40.0 + 80.0 * ease.min(1.0) } else { 3.0 };
        if let Some(n) = self.cast.get_mut(c) {
            n.a.carried_by = Some(to);
            n.a.carried_until = t + stay as f64;
        }
        self.update_riders();
        let msg = format!("{name} hands {oname} to {tname}");
        self.event("gave", Some(who), Some(other.key()), msg.clone(), Some(a.pos), json!({ "to": to }));
        self.social.bond(who, to, 0.05, t);
        Ok(Outcome::ok(msg))
    }

    /// An animal leaves the arms that hold it: set down gently, or (`free`)
    /// wriggling free, running off if it didn't want to be held.
    pub fn let_go(&mut self, cid: i64, free: bool) {
        let me = ActorId::Npc(cid);
        let Some(by) = self.actor(me).and_then(|a| a.carried_by) else { return };
        let (p, fwd, r) = self.actor(by).map(|a| (a.pos, a.forward(), a.dims.radius)).unwrap_or_else(|| (self.actor(me).map(|a| a.pos).unwrap_or_default(), Vec3::Z, 0.3));
        let mine = self.actor(me).map(|a| a.dims.radius).unwrap_or(0.2);
        let spot = p + fwd * (r + mine + 0.2);
        let ground = self.snap.terrain.height(spot.x, spot.z).max(p.y);
        if let Some(a) = self.actor_mut(me) {
            a.carried_by = None;
            a.carried_until = 0.0;
            a.pos = Vec3::new(spot.x, ground, spot.z);
            a.vy = 0.0;
            a.grounded = true;
            a.support = None;
        }
        let name = self.actor_name(me);
        let holder = self.actor_name(by);
        let ease = self.ease_with(me, by);
        if free && ease < 0.3 {
            self.make_noise(cid, false);
            self.run_from(cid, &holder, Some(by.key()), p);
            self.note_near(spot, 20.0, Note::seen(format!("{} wriggles free of {holder} and bolts.", super::physics::cap(&name)), by == ActorId::Player));
            self.event("escaped", Some(me), Some(by.key()), format!("{name} wriggled free of {holder}"), Some(spot), json!({}));
        } else if free {
            self.note_near(spot, 20.0, Note::seen(format!("{} wriggles down from {holder}'s arms.", super::physics::cap(&name)), by == ActorId::Player));
            self.event("escaped", Some(me), Some(by.key()), format!("{name} wriggled down from {holder}'s arms"), Some(spot), json!({ "calm": true }));
            self.social.bond(me, by, 0.02, self.t);
        } else if ease >= 0.3 {
            self.social.bond(me, by, 0.02, self.t);
        }
        if let Some(n) = self.cast.get_mut(cid) {
            n.think_at = n.think_at.min(self.t + 1.0);
        }
    }

    /// A carried animal's turn: held still in the arms until it wants
    /// down (or its holder is gone). True while it is carried.
    pub(super) fn carried_tick(&mut self, cid: i64, dt: f32) -> bool {
        let me = ActorId::Npc(cid);
        let Some(by) = self.actor(me).and_then(|a| a.carried_by) else { return false };
        let holder_gone = self.actor(by).is_none_or(|a| a.asleep);
        if holder_gone || self.t >= self.actor(me).map(|a| a.carried_until).unwrap_or(0.0) {
            self.let_go(cid, true);
            return false;
        }
        let t = self.t;
        if let Some(n) = self.cast.get_mut(cid) {
            n.a.task = None;
            n.a.update_pose(t, dt, None);
        }
        true
    }

    /// Carried animals sit in their holder's arms.
    pub(super) fn update_carried(&mut self) {
        let carried: Vec<(ActorId, ActorId)> = self.actor_ids().into_iter().filter_map(|a| self.actor(a).and_then(|x| x.carried_by).map(|b| (a, b))).collect();
        for (a, by) in carried {
            let Some((hand, yaw)) = self.actor(by).map(|h| (h.hand(true), h.yaw)) else {
                if let Some(x) = self.actor_mut(a) {
                    x.carried_by = None;
                }
                continue;
            };
            if let Some(x) = self.actor_mut(a) {
                x.pos = hand - Vec3::Y * x.dims.height * 0.45;
                x.yaw = yaw + std::f32::consts::FRAC_PI_2;
                x.alt = 0.0;
                x.vy = 0.0;
            }
        }
    }
}
