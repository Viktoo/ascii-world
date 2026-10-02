//! Beings that are not people: what their species and body change about the
//! shared verbs (strength, sounds instead of words, contact distance), and
//! the cheap mind of animals (follow their person, fetch, graze, rest).
//! Everything reads the taxonomy (`world::species`), never a species' name.

use super::actions::Action;
use super::props::*;
use super::things::ThingId;
use super::{ActorId, Note, Sim, Target};
use crate::world::species::{Mind, Speech};
use glam::Vec3;
use serde_json::json;

impl Sim {
    /// How much an actor can lift alone (kg): a person's strength, scaled
    /// with body mass (a dog carries a stick, a horse a sack, a cat a mouse).
    pub fn strength(&self, who: ActorId) -> f32 {
        let m = self.actor(who).map(|a| a.dims.mass).unwrap_or(70.0);
        (STRENGTH * (m / 70.0).powf(2.0 / 3.0)).clamp(0.3, 4000.0)
    }

    /// The species' mind for an actor (the traveller is a person).
    pub fn mind_of(&self, who: ActorId) -> Mind {
        match who {
            ActorId::Player => Mind::Sapient,
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.species.mind).unwrap_or(Mind::Sapient),
        }
    }

    /// Whether an actor speaks in words.
    pub fn speaks(&self, who: ActorId) -> bool {
        match who {
            ActorId::Player => true,
            ActorId::Npc(c) => self.cast.get(c).is_none_or(|n| n.species.speech == Speech::Words),
        }
    }

    /// One of an animal's noises: `happy` picks a friendly one, otherwise
    /// the last (a growl, a hiss).
    pub fn noise(&mut self, cid: i64, happy: bool) -> Option<String> {
        let n = self.cast.get_mut(cid)?;
        if n.species.speech == Speech::None || n.species.sounds.is_empty() {
            return None;
        }
        let s = &n.species.sounds;
        let i = if happy || s.len() == 1 {
            n.rng = crate::noise::pcg(n.rng);
            (n.rng as usize) % (s.len() - (s.len() > 2) as usize).max(1)
        } else {
            s.len() - 1
        };
        Some(s[i].clone())
    }

    /// An animal makes a noise: heard nearby, remembered like a line.
    pub fn make_noise(&mut self, cid: i64, happy: bool) {
        let who = ActorId::Npc(cid);
        let Some(at) = self.actor(who).map(|a| a.pos) else { return };
        let Some(noise) = self.noise(cid, happy) else { return };
        let name = self.actor_name(who);
        self.note_near(at, 22.0, Note::Info(format!("{} gives {}.", super::physics::cap(&name), with_article(&noise))));
        self.event("sound", Some(who), None, format!("{name}: {noise}"), Some(at), json!({ "sound": noise }));
    }

    /// Words said by someone who has no words come out as a noise: the
    /// friendly kinds of line as a friendly one.
    pub fn speak_as_animal(&mut self, cid: i64, text: &str) {
        let l = text.to_lowercase();
        let unhappy = ["no", "not now", "rather not", "fire", "get back", "burning"].iter().any(|w| l.starts_with(w) || l.contains(w));
        self.make_noise(cid, !unhappy);
    }

    /// Extra room a contact gesture needs between two bodies bigger than a
    /// person's: the distance is kept between their sides.
    pub fn contact_gap(&self, a: ActorId, b: ActorId) -> f32 {
        let r = |x: ActorId| self.actor(x).map(|a| (a.dims.radius - 0.3).max(0.0)).unwrap_or(0.0);
        r(a) + r(b)
    }

    /// How much more an animal must like someone before it lets them close.
    pub fn wariness(&self, cid: i64) -> f32 {
        match self.cast.get(cid) {
            Some(n) if n.species.mind != Mind::Sapient => (n.species.temper.wary - 0.5) * 0.5 - (n.species.temper.tame - 0.5) * 0.2,
            _ => 0.0,
        }
    }

    /// The person an animal belongs to, if any (the nearest when several).
    pub fn owner_of(&self, cid: i64) -> Option<ActorId> {
        let me = ActorId::Npc(cid);
        let pos = self.actor(me)?.pos;
        let mut best: Option<(f32, ActorId)> = None;
        for o in self.actor_ids() {
            if o == me {
                continue;
            }
            if self.social.rel(me, o).and_then(|r| r.owner) == Some(o.code()) {
                let d = self.actor(o).map(|a| (a.pos - pos).length()).unwrap_or(f32::MAX);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, o));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The traveller spoke to (or called) an animal: it answers with a noise
    /// and its body, by how it feels about them. No words, no LLM.
    pub fn animal_answers(&mut self, cid: i64) {
        let me = ActorId::Npc(cid);
        let Some(pos) = self.actor(me).map(|a| a.pos) else { return };
        let aff = self.social.affection(me, ActorId::Player) - self.wariness(cid);
        let player = self.player.pos;
        if let Some(n) = self.cast.get_mut(cid) {
            n.a.asleep = false;
            n.a.face(player - pos, 10.0);
            n.think_at = self.t + 4.0;
        }
        if aff > 0.15 {
            self.make_noise(cid, true);
            let _ = self.act(me, Action::Gesture { kind: "wag".into(), to: None });
            self.social.bond(me, ActorId::Player, 0.03, self.t);
            self.set_doing(cid, "greeting the traveller");
        } else if aff < -0.25 {
            self.make_noise(cid, false);
            let away = (pos - player).normalize_or_zero() * 4.0;
            self.plan(me, vec![Action::Goto { target: Target::Point((pos + away).to_array()), run: true }], "keep away from the traveller", false);
            self.set_doing(cid, "backing away");
        } else {
            self.make_noise(cid, true);
            let _ = self.act(me, Action::Gesture { kind: "nod".into(), to: None });
            self.social.bond(me, ActorId::Player, 0.02, self.t);
            self.set_doing(cid, "eyeing the traveller");
        }
    }

    /// Something thrown lately by someone this animal likes, for fetching.
    fn fetchable(&self, cid: i64, range: f32) -> Option<(ThingId, ActorId)> {
        let me = ActorId::Npc(cid);
        let pos = self.actor(me)?.pos;
        let strength = self.strength(me);
        let mut best: Option<(f32, ThingId, ActorId)> = None;
        for t in self.things.live() {
            let Some((by, when)) = t.thrown_by else { continue };
            if by == me || self.t - when > 25.0 || t.held() || t.anchored || t.mass() > strength || t.props[P_FIRE] > 0.0 {
                continue;
            }
            if self.social.affection(me, by) < 0.2 {
                continue;
            }
            let d = (t.pos - pos).length();
            if d < range && best.is_none_or(|b| d < b.0) {
                best = Some((d, t.id, by));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// A thing was thrown: playful animals nearby look up from what they do.
    pub fn animals_notice_throw(&mut self, at: Vec3) {
        let t = self.t;
        for n in self.cast.npcs.iter_mut() {
            if n.species.mind != Mind::Sapient && n.species.temper.playful > 0.5 && !n.a.asleep && (n.a.pos - at).length() < 30.0 && n.plan.is_empty() {
                n.think_at = n.think_at.min(t + 0.3);
                if matches!(n.a.task, Some(super::actor::Task::Follow { .. }) | Some(super::actor::Task::Wait { .. }) | None) {
                    n.a.task = None;
                }
            }
        }
    }

    /// What an animal does next: cheap scoring, no LLM, through the same
    /// actions as everyone else.
    pub(super) fn think_animal(&mut self, cid: i64) {
        let t = self.t;
        let me = ActorId::Npc(cid);
        let Some(n) = self.cast.get(cid) else { return };
        let pos = n.a.pos;
        let needs = n.needs;
        let temper = n.species.temper;
        let home = n.def.home;
        let held = n.a.held;
        let next = |s: &mut Sim, secs: f64| {
            if let Some(n) = s.cast.get_mut(cid) {
                n.think_at = s.t + secs;
            }
        };
        // Bring back what was thrown.
        if held.is_none() && temper.playful > 0.5 {
            if let Some((id, by)) = self.fetchable(cid, 30.0) {
                self.plan(me, vec![Action::Hold { target: Target::Thing(id) }, Action::Goto { target: Target::Actor(by), run: true }, Action::Give { to: Target::Actor(by) }], "fetch", false);
                self.set_doing(cid, "fetching");
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.fun = (n.needs.fun - 0.15).max(0.0);
                }
                self.event("fetch", Some(me), Some(format!("thing:{id}")), format!("{} runs after the {}", self.actor_name(me), self.thing_name(id)), Some(pos), json!({ "for": by }));
                next(self, 3.0);
                return;
            }
        }
        // Holding something with nothing to do with it: bring it to its person, or drop it.
        if let Some(h) = held {
            let to = self.owner_of(cid).filter(|o| self.actor(*o).is_some_and(|a| (a.pos - pos).length() < 40.0 && a.held.is_none()));
            match to {
                Some(o) if (self.actor(o).map(|a| a.pos).unwrap_or(pos) - pos).length() > 1.6 => {
                    self.plan(me, vec![Action::Goto { target: Target::Actor(o), run: false }, Action::Give { to: Target::Actor(o) }], "bring it back", false);
                }
                _ => {
                    let _ = h;
                    self.plan(me, vec![Action::Drop], "", false);
                }
            }
            next(self, 3.0);
            return;
        }
        let owner = self.owner_of(cid);
        let owner_at = owner.and_then(|o| self.actor(o).filter(|a| !a.asleep).map(|a| a.pos));
        let mut best: (f32, &str) = (0.15 + 0.1 * self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0), "wander");
        fn consider(best: &mut (f32, &'static str), score: f32, what: &'static str) {
            if score > best.0 {
                *best = (score, what);
            }
        }
        if let Some(op) = owner_at {
            let d = (op - pos).length();
            if d < 80.0 && d > 4.0 {
                consider(&mut best, 0.35 + needs.social * 0.6, "follow");
            }
        }
        let food = self.food_for(cid, pos, 25.0);
        if needs.hunger > 0.45 && food.is_some() {
            consider(&mut best, needs.hunger * 1.1, "eat");
        }
        if needs.fatigue > 0.6 {
            consider(&mut best, needs.fatigue * 0.8, "rest");
        }
        if needs.fun > 0.5 && temper.playful > 0.5 {
            consider(&mut best, needs.fun * temper.playful * 0.7, "play");
        }
        match best.1 {
            "follow" => {
                let o = owner.unwrap_or(ActorId::Player);
                let last = self.cast.get(cid).map(|n| n.last_greet).unwrap_or(f64::MIN);
                let mut steps = vec![Action::Goto { target: Target::Actor(o), run: true }];
                // A greeting after time apart.
                if t - last > 120.0 {
                    steps.push(Action::Gesture { kind: "wag".into(), to: None });
                    if let Some(n) = self.cast.get_mut(cid) {
                        n.last_greet = t;
                    }
                    self.make_noise(cid, true);
                }
                steps.push(Action::Follow { target: Target::Actor(o), secs: Some(10.0) });
                let oname = self.actor_name(o);
                self.plan(me, steps, &format!("stay with {oname}"), false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.social = (n.needs.social - 0.2).max(0.0);
                }
                self.set_doing(cid, &format!("following {oname}"));
                next(self, 4.0);
            }
            "eat" => {
                let Some((tg, _)) = food else { return };
                self.plan(me, vec![Action::Eat { target: Some(tg) }], "eat", false);
                self.set_doing(cid, "grazing");
                next(self, 4.0);
            }
            "rest" => {
                self.plan(me, vec![Action::Gesture { kind: "sit".into(), to: None }, Action::Wait { secs: 15.0 }], "rest", false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.fatigue = (n.needs.fatigue - 0.25).max(0.0);
                }
                self.set_doing(cid, "lying down");
                next(self, 16.0);
            }
            "play" => {
                // Dash about near its person (or home).
                let centre = owner_at.unwrap_or(home);
                let a = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0) * std::f32::consts::TAU;
                let p = centre + Vec3::new(a.cos(), 0.0, a.sin()) * 6.0;
                self.plan(me, vec![Action::Goto { target: Target::Point([p.x, 0.0, p.z]), run: true }, Action::Gesture { kind: "wag".into(), to: None }], "play", false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.fun = (n.needs.fun - 0.2).max(0.0);
                }
                self.set_doing(cid, "dashing about");
                next(self, 4.0);
            }
            _ => {
                let centre = owner_at.unwrap_or(home);
                self.wander(cid, centre, 12.0);
                self.set_doing(cid, "sniffing about");
                let r = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.5);
                next(self, 4.0 + r as f64 * 6.0);
            }
        }
    }

    /// Something this species eats, near `p`: food anyone eats, and for
    /// plant eaters, grass and leaves.
    pub fn food_for(&mut self, cid: i64, p: Vec3, range: f32) -> Option<(Target, Vec3)> {
        let diet = self.cast.get(cid)?.species.diet.clone();
        let strength = self.strength(ActorId::Npc(cid));
        let plants = diet.plants > 0.5;
        self.nearest_matching(p, range, |pr| (pr[P_EDIBLE] > 0.05 && pr[P_MASS] < strength.max(1.0)) || (plants && pr[P_ALIVE] > 0.0 && pr[P_MASS] < 1.0 && pr[P_FIRE] <= 0.0))
    }
}

fn with_article(s: &str) -> String {
    let l = s.to_lowercase();
    if l.starts_with("a ") || l.starts_with("an ") || l.starts_with("the ") || s.ends_with('!') {
        s.to_string()
    } else {
        format!("{} {s}", super::article(s))
    }
}
