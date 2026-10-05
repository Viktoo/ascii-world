//! Bodies have properties. Every being's body is a property vector like a
//! thing's, started from its species (a living body keeps itself at 36°,
//! plus whatever its species declares: a fire spirit is hot, a ghost cold)
//! and saved as the difference. Bodies near the traveler join the rules
//! pass, so a world's own rules reach people: a curse that spreads by touch
//! curses whoever holds the idol, a kiln warms those by it, wading wets.
//! What someone holds or wears touches them every pass, and a touch between
//! two people (a hug, a gift handed over) runs the rules between their
//! bodies once, with `kindness` on whoever means it warmly.
//!
//! The traveler's body is never in the rules: they are never cursed, soaked
//! or burnt by them. In a touch they take part with a stand-in body whose
//! changes are dropped.

use super::npc::Npc;
use super::props::*;
use super::{ActorId, Note, Sim};
use serde_json::json;

/// How long a touch counts for, by what it is (s of the rules), and how
/// close it is (m): a hug is a full embrace, kind words are only near.
pub const HUG_SECS: f32 = 3.0;
pub const GIFT_SECS: f32 = 2.0;
pub const WORDS_SECS: f32 = 2.0;
pub const EMBRACE: f32 = 0.1;
pub const HAND: f32 = 0.3;
pub const NEAR: f32 = 0.8;
/// The most charges kindness earns in one day.
const KIND_PER_DAY: i32 = 4;

impl Sim {
    /// A plain body of this being's species, as the rules see it.
    pub fn body_base(&self, n: &Npc) -> Props {
        let mut p = self.vocab.defaults.clone();
        p[P_MASS] = n.a.dims.mass.max(1.0);
        p[P_TEMP] = 36.0;
        p[P_HEAT] = 36.0;
        p[P_ALIVE] = 1.0;
        p[P_BODY] = 1.0;
        p[P_BURNS] = 0.0;
        for (k, v) in &n.species.props {
            if let Some(i) = self.vocab.lookup(k).filter(|i| self.vocab.writable(*i)) {
                p[i] = *v;
            }
        }
        sanitize(&mut p);
        self.vocab.clamp(&mut p);
        p
    }

    /// Fit every body to the vocabulary: new ones from their species and
    /// what was saved, older ones grown to properties added since.
    pub fn fit_bodies(&mut self) {
        let len = self.vocab.len();
        for i in 0..self.cast.npcs.len() {
            let n = &self.cast.npcs[i];
            if n.props.len() == len {
                continue;
            }
            let props = if n.props.is_empty() {
                let base = self.body_base(n);
                let s = &n.def.state;
                let mut p = apply_diff(&self.vocab, &base, &serde_json::Value::Object(s.props.clone()).to_string());
                // Older saves kept darkness apart from the body.
                if s.corruption > 0.0 && !s.props.contains_key("corruption") {
                    p[P_CORRUPT] = s.corruption.clamp(0.0, 1.0);
                }
                p
            } else {
                let mut p = n.props.clone();
                p.truncate(len);
                while p.len() < len {
                    p.push(self.vocab.defaults[p.len()]);
                }
                p
            };
            self.cast.npcs[i].props = props;
        }
    }

    /// What a body's save keeps: where it differs from its species' plain one.
    pub fn body_diff(&self, n: &Npc) -> serde_json::Map<String, serde_json::Value> {
        if n.props.len() != self.vocab.len() {
            return n.def.state.props.clone();
        }
        diff(&self.vocab, &n.props, &self.body_base(n))
    }

    /// How readily darkness takes hold of this body: the world's difficulty,
    /// except for the dark's own (their touch does it, not the rules).
    pub fn susceptibility(&self, n: &Npc) -> f32 {
        if n.species.touch.harms() { 0.0 } else { self.level().spread }
    }

    /// What the engine keeps true of a body before the rules see it: its
    /// mass, that it is a body, and how readily darkness takes it.
    pub fn refresh_body(&mut self, cid: i64) {
        let Some(n) = self.cast.get(cid) else { return };
        if n.props.len() != self.vocab.len() {
            return;
        }
        let (mass, sus) = (n.a.dims.mass.max(1.0), self.susceptibility(n));
        if let Some(n) = self.cast.get_mut(cid) {
            n.props[P_MASS] = mass;
            n.props[P_BODY] = 1.0;
            n.props[P_SUSCEPT] = sus;
        }
    }

    /// A body as the rules see it: a being's own, or for the traveler a
    /// stand-in (a plain body carrying the darkness in them).
    pub fn body_props(&self, who: ActorId) -> Props {
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get(c).filter(|n| n.props.len() == self.vocab.len()) {
                let mut p = n.props.clone();
                p[P_SUSCEPT] = self.susceptibility(n);
                return p;
            }
        }
        let mut p = self.vocab.defaults.clone();
        if let Some(a) = self.actor(who) {
            p[P_MASS] = a.dims.mass.max(1.0);
        }
        p[P_TEMP] = 36.0;
        p[P_HEAT] = 36.0;
        p[P_ALIVE] = 1.0;
        p[P_BODY] = 1.0;
        if who == ActorId::Player {
            p[P_CORRUPT] = self.night.corruption;
            p[P_SUSCEPT] = self.level().spread;
        }
        p
    }

    /// Two bodies touch for `secs`: the world's pair rules run both ways
    /// between them, with `kindness` on each as meant (a hug, a gift). Only
    /// beings' bodies change; the traveler's stand-in is dropped.
    pub fn touch_bodies(&mut self, a: ActorId, b: ActorId, kind_a: f32, kind_b: f32, secs: f32, dist: f32) {
        if a == b {
            return;
        }
        let (mut pa, mut pb) = (self.body_props(a), self.body_props(b));
        // A twisted heart's touch isn't warm.
        pa[P_KIND] = kind_a.clamp(0.0, 1.0) * (1.0 - pa[P_CORRUPT]).max(0.0);
        pb[P_KIND] = kind_b.clamp(0.0, 1.0) * (1.0 - pb[P_CORRUPT]).max(0.0);
        let rules = self.rules.clone();
        let (hour, night) = (self.hour(), self.night() as u32 as f32);
        let (mut na, mut nb) = (pa.clone(), pb.clone());
        let va = super::rules::EntView { props: &pa, water: 0.0, held: 0.0, ground: 1.0, falling: 0.0, wind: 0.0 };
        let vb = super::rules::EntView { props: &pb, water: 0.0, held: 0.0, ground: 1.0, falling: 0.0, wind: 0.0 };
        for r in rules.iter().filter(|r| r.near.is_some()) {
            if super::rules::pair_self_ok(r, &va, secs, hour, night) {
                super::rules::run_pair(r, &va, &vb, dist, secs, hour, night, &mut na, &mut nb);
            }
            if super::rules::pair_self_ok(r, &vb, secs, hour, night) {
                super::rules::run_pair(r, &vb, &va, dist, secs, hour, night, &mut nb, &mut na);
            }
        }
        for (who, before, mut after, other) in [(a, pa, na, b), (b, pb, nb, a)] {
            let ActorId::Npc(c) = who else { continue };
            after[P_KIND] = 0.0;
            sanitize(&mut after);
            self.vocab.clamp(&mut after);
            let name = self.actor_name(who);
            let pos = self.actor(who).map(|x| x.pos).unwrap_or_default();
            if let Some(n) = self.cast.get_mut(c).filter(|n| n.props.len() == after.len()) {
                n.props = after.clone();
            } else {
                continue;
            }
            self.tell_body_change(c, &name, pos, &before, &after, Some(other));
            // The dark going out of someone through the traveler's kindness.
            if other == ActorId::Player && before[P_CORRUPT] >= 0.1 && after[P_CORRUPT] < 0.1 {
                self.event("cleansed", Some(ActorId::Player), Some(who.key()), format!("the traveler drew the dark out of {name}"), Some(pos), json!({}));
                self.notes.push(Note::Notable(format!("The dark goes out of {name}.")));
                self.witness(pos, 20.0, &format!("the traveler drew the dark out of {name}"), 0.7, &[]);
            }
        }
    }

    /// Someone is kind to someone else (a gift handed over, time spent
    /// together): their bodies touch with kindness, and the traveler's
    /// kindness earns a charge now and then (`earns`).
    pub fn kind_touch(&mut self, from: ActorId, to: ActorId, kindness: f32, secs: f32, dist: f32, both: bool, earns: bool) {
        self.touch_bodies(from, to, kindness, if both { kindness } else { 0.0 }, secs, dist);
        let other = if from == ActorId::Player { to } else if to == ActorId::Player && both { from } else { return };
        if !earns || !matches!(other, ActorId::Npc(_)) {
            return;
        }
        let day = super::night::NightState::day(self.t);
        if self.night.kind_today.0 != day {
            self.night.kind_today = (day, 0);
        }
        if self.night.kind_today.1 < KIND_PER_DAY {
            self.night.kind_today.1 += 1;
            let name = self.actor_name(other);
            self.gain_charges(1, &format!("{} is glad of you.", super::physics::cap(&name)));
        }
    }

    /// How much what is on a body hurts (by each property's harm), kept on
    /// the being: pain wears them out and is in what their planner sees.
    pub fn feel_bodies(&mut self, dt: f32) {
        let vocab = self.vocab.clone();
        for n in self.cast.npcs.iter_mut().filter(|n| n.here()) {
            n.pain = if n.props.len() == vocab.len() && !n.species.touch.harms() { vocab.harm(&n.props) } else { 0.0 };
            if n.pain > 0.05 {
                n.needs.fatigue = (n.needs.fatigue + n.pain * 0.004 * dt).min(1.0);
            }
        }
    }

    /// What stands out on a body, in words, for its own planner: "cursed
    /// 0.6 (how cursed it is), hurting".
    pub fn body_line(&self, cid: i64) -> Option<String> {
        let n = self.cast.get(cid)?;
        if n.props.len() != self.vocab.len() {
            return None;
        }
        let base = self.body_base(n);
        let mut parts = Vec::new();
        for (i, (v, b)) in n.props.iter().zip(&base).enumerate() {
            if self.vocab.engine_only(i) || i == P_MASS || (i == P_TEMP && (v - b).abs() < 6.0) {
                continue;
            }
            let unit = b.abs().max(1.0);
            if (v - b).abs() > unit * 0.15 {
                let meaning = self.vocab.meanings[i].split([',', ':']).next().unwrap_or("").trim().to_string();
                parts.push(format!("{} {:.1} ({meaning})", self.vocab.names[i], v));
            }
        }
        if n.pain > 0.05 {
            parts.push("it hurts".into());
        }
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}
