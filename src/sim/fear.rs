//! Fear, and coming upon the dead.
//!
//! Each being keeps, toward each other, how much it fears they can hurt it
//! (`Rel::fear`, one way each). It comes only from health going down at
//! someone's hand: being hurt by them, or seeing them hurt or kill. The
//! timid take it harder. It fades over days. What it does is plain: near
//! someone it fears (more so when that one holds a tool that can strike),
//! a being keeps away; one that thinks for itself is asked what to do with
//! the fear in mind (plead, back off, run, stand its ground), and the fear
//! is in what it is told about the people around it and in its talk.
//!
//! Whoever comes upon a body stops, shaken by how close they were to the
//! dead, and remembers it; whoever saw the death already knows.

use super::actor::Task;
use super::{ActorId, Note, Request, Sim, Target};
use crate::render::sky::DAY_SECONDS;
use crate::world::species::Mind;
use glam::Vec3;
use serde_json::json;

/// Fear halves over about this many game days.
const FADE_DAYS: f64 = 2.0;
/// How far a blow or a death is seen (m).
const SIGHT: f32 = 25.0;
/// How near someone feared must be to move them (m).
const NEAR: f32 = 9.0;
/// How near a body must be to be come upon (m).
const FOUND: f32 = 16.0;
/// Fear moves a being at most this often (s).
const FEAR_GAP: f64 = 12.0;

impl Sim {
    /// `victim`'s health went down by `hurt` at `by`'s hand (to nothing, if
    /// `died`): the victim comes to fear them, and so does everyone who saw.
    pub fn hurt_by(&mut self, victim: ActorId, by: ActorId, hurt: f32, died: bool) {
        if victim == by {
            return;
        }
        let Some(at) = self.actor(victim).map(|a| a.pos) else { return };
        if !died {
            let k = self.timid(victim);
            self.social.add_fear(victim, by, (hurt * 1.6 + 0.1) * k);
        }
        let seen: Vec<i64> = self
            .cast
            .npcs
            .iter()
            .filter(|n| n.here() && !n.a.asleep && (n.a.pos - at).length() < SIGHT)
            .map(|n| n.def.id)
            .filter(|c| ActorId::Npc(*c) != victim && ActorId::Npc(*c) != by)
            .collect();
        let shock = if died { 0.45 } else { 0.08 + hurt * 0.6 };
        for c in seen {
            let k = self.timid(ActorId::Npc(c));
            self.social.add_fear(ActorId::Npc(c), by, shock * k);
        }
        // Those who saw it die needn't find the body to know.
        if let (true, ActorId::Npc(dead)) = (died, victim) {
            for n in self.cast.npcs.iter_mut().filter(|n| n.here() && !n.a.asleep && (n.a.pos - at).length() < SIGHT) {
                if !n.seen_dead.contains(&dead) {
                    n.seen_dead.push(dead);
                }
            }
        }
    }

    /// How much harder fear takes them: 0.4 for the bravest, 1.4 the timid.
    fn timid(&self, who: ActorId) -> f32 {
        match who {
            ActorId::Npc(c) => self.cast.get(c).map(|n| 1.4 - n.traits.brave).unwrap_or(1.0),
            ActorId::Player => 1.0,
        }
    }

    /// Fear fades.
    pub fn fade_fear(&mut self, dt: f32) {
        let k = (-(dt as f64) / (FADE_DAYS * DAY_SECONDS) * std::f64::consts::LN_2).exp() as f32;
        let mut any = false;
        for r in self.social.rels.values_mut() {
            for f in r.fear.iter_mut() {
                if *f > 0.0 {
                    *f = if *f * k < 0.01 { 0.0 } else { *f * k };
                    any = true;
                }
            }
        }
        self.social.dirty |= any;
    }

    /// Whether `who` holds something that can strike (a tool with a swing,
    /// a thrust or a chop in it), and what.
    pub fn armed(&self, who: ActorId) -> Option<String> {
        let (id, tool) = self.held_tool(who)?;
        tool.motions.iter().any(|m| matches!(m.as_str(), "swing" | "thrust" | "chop")).then(|| self.thing_name(id))
    }

    /// How much `me` fears `o` right now: their fear of them, more when
    /// `o` is armed and close.
    pub fn menace(&self, me: ActorId, o: ActorId) -> f32 {
        let f = self.social.fear(me, o);
        if f <= 0.0 {
            return 0.0;
        }
        let (Some(a), Some(b)) = (self.actor(me), self.actor(o)) else { return 0.0 };
        let d = (a.pos - b.pos).length();
        let armed = if self.armed(o).is_some() { 1.0 } else { 0.6 };
        f * armed * (1.0 - 0.5 * (d / NEAR).min(1.0))
    }

    /// The fear `me` has of `o`, in words for their planner and their talk:
    /// "You are terrified of the traveler, who holds a sword 2 m from you."
    pub fn fear_line(&self, me: i64, o: ActorId) -> Option<String> {
        let words = super::social::fear_words(self.social.fear(ActorId::Npc(me), o))?;
        let name = self.actor_name(o);
        let what = words.trim_end_matches(" of them");
        let d = match (self.actor(ActorId::Npc(me)), self.actor(o)) {
            (Some(a), Some(b)) => (a.pos - b.pos).length(),
            _ => f32::MAX,
        };
        let armed = self.armed(o).filter(|_| d < 30.0).map(|w| format!(", who holds {} {d:.0} m from you", super::actions::the(&w))).unwrap_or_default();
        Some(format!("You are {what} of {name}{armed}: they can hurt you."))
    }

    /// Fear at work, once a second: near someone they fear enough, a being
    /// keeps away, or (if it thinks) decides what to do about it.
    pub fn step_fear(&mut self) {
        let t = self.t;
        let ids: Vec<i64> = self
            .cast
            .npcs
            .iter()
            .filter(|n| n.here() && !n.a.asleep && !n.a.carried() && t - n.feared_at > FEAR_GAP && self.talking_to != Some(n.def.id))
            .map(|n| n.def.id)
            .collect();
        for c in ids {
            let me = ActorId::Npc(c);
            let Some(pos) = self.actor(me).map(|a| a.pos) else { continue };
            let worst = self
                .actor_ids()
                .into_iter()
                .filter(|o| *o != me && self.actor(*o).is_some_and(|a| (a.pos - pos).length() < NEAR))
                .map(|o| (o, self.menace(me, o)))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            let Some((o, m)) = worst else { continue };
            let brave = self.cast.get(c).map(|n| n.traits.brave).unwrap_or(0.5);
            let at = 0.2 + 0.3 * brave;
            if m <= at {
                continue;
            }
            if let Some(n) = self.cast.get_mut(c) {
                n.feared_at = t;
            }
            let thinks = self.has_llm && self.mind_of(me) == Mind::Sapient;
            let opos = self.actor(o).map(|a| a.pos).unwrap_or(pos);
            // Past what they can think through: they run first.
            if !thinks || m > at + 0.35 {
                let name = self.actor_name(o);
                self.run_from(c, &name, Some(o.key()), opos);
            }
            if thinks {
                let ctx = self.fear_line(c, o).unwrap_or_else(|| format!("{} is close, and you fear them.", self.actor_name(o)));
                let context = self.decide_context(c, &ctx);
                self.request_weighted(Request::Decide { cid: c, event: "afraid".into(), context }, pos, 1.0);
            }
        }
    }

    /// Bodies come upon, once a second: whoever thinks and walks by one
    /// they hadn't known of stops, is shaken, and remembers it.
    pub fn step_found_bodies(&mut self) {
        let bodies: Vec<(i64, Vec3)> = self.things.live().filter_map(|t| t.origin.remains.map(|c| (c, t.pos))).collect();
        if bodies.is_empty() {
            return;
        }
        let t = self.t;
        let mut found: Vec<(i64, i64, Vec3)> = Vec::new();
        for n in self.cast.npcs.iter().filter(|n| n.here() && !n.a.asleep && n.species.mind == Mind::Sapient) {
            for (dead, at) in &bodies {
                if *dead != n.def.id && !n.seen_dead.contains(dead) && (n.a.pos - *at).length() < FOUND {
                    found.push((n.def.id, *dead, *at));
                }
            }
        }
        for (c, dead, at) in found {
            let me = ActorId::Npc(c);
            let them = ActorId::Npc(dead);
            if let Some(n) = self.cast.get_mut(c) {
                n.seen_dead.push(dead);
            }
            // How close they were to the dead, and whether the dead was one of their own kind of mind.
            let rel = self.social.rel(me, them);
            let close = rel.map(|r| if r.family || r.partner { 1.0 } else { r.affection.clamp(0.0, 1.0) }).unwrap_or(0.0);
            let person = self.cast.get(dead).is_some_and(|n| n.species.mind == Mind::Sapient);
            let weight = (0.55 + 0.45 * close) * if person { 1.0 } else { 0.4 };
            let name = self.actor_name(me);
            let body = self.remains_name(dead);
            let who = rel.map(|r| r.describe()).unwrap_or_else(|| "a stranger".into());
            let msg = format!("{name} came upon the {body}");
            self.event("found_body", Some(me), Some(them.key()), msg.clone(), Some(at), json!({ "weight": (weight * 100.0).round() / 100.0 }));
            self.note_near(at, 30.0, Note::Ambient(format!("{}.", super::physics::cap(&msg))));
            let memory = format!("I came upon the {body}, lying dead");
            if let Some(n) = self.cast.get_mut(c) {
                n.hear_news(t, &memory, weight);
                n.curiosity_bump(weight);
            }
            self.out.push(Request::Witness { cid: c, text: memory, importance: weight });
            // Shaken: they drop what they were doing and stare.
            if weight >= 0.5 {
                if let Some(n) = self.cast.get_mut(c) {
                    n.plan.clear();
                    n.shock_at = t;
                    n.doing = format!("staring at the {body}");
                }
                self.set_task(me, Task::Face { target: Target::Point(at.to_array()), until: t + 4.0 });
            }
            if self.has_llm {
                let ctx = format!("You have just come upon the {body} ({who}), lying dead on the ground. How shaken you are, 0–1: {weight:.1}.");
                let context = self.decide_context(c, &ctx);
                self.request_weighted(Request::Decide { cid: c, event: "found_body".into(), context }, at, weight);
            } else if weight >= 0.5 && self.cast.get(c).is_some_and(|n| n.traits.brave < 0.4) {
                self.run_from(c, &format!("the {body}"), None, at);
            }
        }
    }
}
