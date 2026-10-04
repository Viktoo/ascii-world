//! Life together. Relationships (affection, trust, rivalry) change with what
//! people do with each other. Shared activities are built from the same
//! primitives, not special-cased: catch is throw plus catch between two
//! people who like each other; carrying a log is two holds on one heavy
//! thing; a hug is a contact gesture both agree to.

use crate::world::species::Mind;
use super::actions::{Action, ActErr, Outcome};
use super::actor::{GestureKind, GestureRun, Task};
use super::npc::Npc;
use super::props::*;
use super::things::ThingId;
use super::{ActorId, Note, Request, Sim, Target};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Rel {
    pub affection: f32,
    pub trust: f32,
    pub rivalry: f32,
    pub familiarity: f32,
    #[serde(default)]
    pub family: bool,
    #[serde(default)]
    pub partner: bool,
    /// Who of the two owns the other (an animal and its person), by actor code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<i64>,
    #[serde(default)]
    pub last: f64,
}

impl Rel {
    pub fn describe(&self) -> String {
        let mut v = Vec::new();
        if self.partner {
            v.push("your partner");
        } else if self.family {
            v.push("family");
        }
        if self.rivalry > 0.4 {
            v.push("a rival");
        }
        let feel = match self.affection {
            a if a > 0.7 => "dear to you",
            a if a > 0.4 => "a friend",
            a if a > 0.1 => "friendly",
            a if a < -0.3 => "disliked",
            _ => "an acquaintance",
        };
        v.push(feel);
        v.join(", ")
    }
}

/// Something two people can do together.
#[derive(Clone, Debug, PartialEq)]
pub enum Activity {
    Catch,
    Carry,
    Gesture(GestureKind),
    Dance,
    Walk,
    Other(String),
}

impl Activity {
    pub fn parse(s: &str) -> Activity {
        let l = s.trim().to_lowercase();
        if l.contains("catch") || l.contains("throw") || l.contains("ball") || l == "play" {
            Activity::Catch
        } else if l.contains("carry") || l.contains("lift") {
            Activity::Carry
        } else if l.contains("dance") {
            Activity::Dance
        } else if l.contains("walk") || l.contains("follow") || l.contains("come with") {
            Activity::Walk
        } else if let Some(k) = GestureKind::parse(&l).or_else(|| l.split_whitespace().find_map(GestureKind::parse)) {
            Activity::Gesture(k)
        } else {
            Activity::Other(s.trim().to_string())
        }
    }

    pub fn label(&self) -> String {
        match self {
            Activity::Catch => "play catch".into(),
            Activity::Carry => "carry it together".into(),
            Activity::Gesture(k) => k.name().replace('_', " "),
            Activity::Dance => "dance".into(),
            Activity::Walk => "walk together".into(),
            Activity::Other(s) => s.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Proposal {
    pub from: ActorId,
    pub to: ActorId,
    pub activity: Activity,
    pub thing: Option<ThingId>,
    pub at: f64,
    /// Characters answer after a moment's thought.
    pub answer_at: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum JointKind {
    Catch { ball: ThingId },
    Carry { thing: ThingId, dest: Vec3 },
    Gesture(GestureKind),
    Dance,
    Walk,
}

#[derive(Clone, Debug)]
pub struct Joint {
    pub id: u64,
    pub kind: JointKind,
    pub a: ActorId,
    pub b: ActorId,
    pub started: f64,
    pub until: f64,
    pub phase: u8,
    pub next: f64,
    pub count: u32,
}

impl Joint {
    pub fn has(&self, x: ActorId) -> bool {
        self.a == x || self.b == x
    }
    pub fn other(&self, x: ActorId) -> ActorId {
        if self.a == x { self.b } else { self.a }
    }
    pub fn label(&self) -> &'static str {
        match self.kind {
            JointKind::Catch { .. } => "catch",
            JointKind::Carry { .. } => "carry",
            JointKind::Gesture(_) => "gesture",
            JointKind::Dance => "dance",
            JointKind::Walk => "walk",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChatLine {
    pub at: f64,
    pub who: i64,
    pub to: i64,
    pub text: String,
}

#[derive(Default)]
pub struct Social {
    pub rels: BTreeMap<(i64, i64), Rel>,
    pub proposals: Vec<Proposal>,
    pub joints: Vec<Joint>,
    pub chats: VecDeque<ChatLine>,
    chat_want: Vec<(i64, i64, f64)>,
    seen: HashMap<i64, HashSet<String>>,
    seeded: HashSet<i64>,
    next_id: u64,
    pub dirty: bool,
}

fn key(a: ActorId, b: ActorId) -> (i64, i64) {
    let (x, y) = (a.code(), b.code());
    if x <= y { (x, y) } else { (y, x) }
}

impl Social {
    pub fn rel(&self, a: ActorId, b: ActorId) -> Option<&Rel> {
        self.rels.get(&key(a, b))
    }

    pub fn rel_mut(&mut self, a: ActorId, b: ActorId) -> &mut Rel {
        self.dirty = true;
        self.rels.entry(key(a, b)).or_default()
    }

    pub fn affection(&self, a: ActorId, b: ActorId) -> f32 {
        self.rel(a, b).map(|r| r.affection - r.rivalry * 0.5).unwrap_or(0.0)
    }

    /// Time spent well together.
    pub fn bond(&mut self, a: ActorId, b: ActorId, amount: f32, t: f64) {
        if a == b {
            return;
        }
        let r = self.rel_mut(a, b);
        r.affection = (r.affection + amount).clamp(-1.0, 1.0);
        r.trust = (r.trust + amount * 0.5).clamp(-1.0, 1.0);
        r.familiarity = (r.familiarity + amount.abs()).min(1.0);
        r.last = t;
    }

    pub fn busy(&self, a: ActorId) -> bool {
        self.joints.iter().any(|j| j.has(a))
    }

    pub fn joint_of(&self, a: ActorId) -> Option<&Joint> {
        self.joints.iter().find(|j| j.has(a))
    }

    pub fn want_chat(&mut self, a: i64, b: i64, t: f64) {
        if !self.chat_want.iter().any(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a)) {
            self.chat_want.push((a, b, t));
        }
    }

    pub fn seen_novelty(&mut self, cid: i64, what: &str) {
        self.seen.entry(cid).or_default().insert(what.to_lowercase());
    }

    pub fn has_seen(&self, cid: i64, what: &str) -> bool {
        self.seen.get(&cid).is_some_and(|s| s.contains(&what.to_lowercase()))
    }

    fn next(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// Relationships from the personas ("Ola: daughter") for pairs not yet known.
    pub fn seed_from_personas(&mut self, cast: &super::npc::Cast, book: &crate::world::species::SpeciesBook) {
        let fresh: Vec<&Npc> = cast.npcs.iter().filter(|n| !self.seeded.contains(&n.def.id)).collect();
        if fresh.is_empty() {
            return;
        }
        let ids: Vec<i64> = fresh.iter().map(|n| n.def.id).collect();
        for n in &cast.npcs {
            for m in &cast.npcs {
                if n.def.id >= m.def.id || !(ids.contains(&n.def.id) || ids.contains(&m.def.id)) {
                    continue;
                }
                let a = ActorId::Npc(n.def.id);
                let b = ActorId::Npc(m.def.id);
                if self.rel(a, b).is_some() {
                    continue;
                }
                let mut r = Rel::default();
                if n.def.home.distance(m.def.home) < 200.0 {
                    r.familiarity = 0.3;
                    r.affection = 0.1;
                }
                // How their peoples feel about each other, to start with.
                if n.species.name != m.species.name {
                    if let Some(at) = book.attitude(&n.species.name, &m.species.name) {
                        r.affection = (r.affection + at.affection).clamp(-1.0, 1.0);
                        r.trust = (r.trust + at.trust).clamp(-1.0, 1.0);
                        r.rivalry = (r.rivalry + at.rivalry).clamp(0.0, 1.0);
                    }
                }
                for (x, y) in [(n, m), (m, n)] {
                    for rel in &x.def.persona.relationships {
                        let l = rel.to_lowercase();
                        let yname = y.name().to_lowercase();
                        let first = yname.split_whitespace().next().unwrap_or("").to_string();
                        if !(l.contains(&yname) || (!first.is_empty() && l.contains(&first))) {
                            continue;
                        }
                        let has = |ws: &[&str]| ws.iter().any(|w| l.contains(w));
                        // An animal and its person.
                        let (xa, ya) = (!x.species.is_human() && x.species.mind != Mind::Sapient, !y.species.is_human() && y.species.mind != Mind::Sapient);
                        if xa && !ya && has(&["owner", "master", "mistress", "keeper", "person", "human", "rider", "herder", "shepherd"]) {
                            r.owner = Some(y.def.id);
                        } else if ya && !xa && (has(&["pet", "dog", "cat", "horse", "hound", "pony", "mount", "goat", "flock", "herd"]) || l.contains(&y.species.name)) {
                            r.owner = Some(x.def.id);
                        }
                        if r.owner.is_some() {
                            r.affection = r.affection.max(0.8);
                            r.trust = r.trust.max(0.8);
                            r.familiarity = r.familiarity.max(0.9);
                            continue;
                        }
                        if has(&["wife", "husband", "spouse", "partner", "lover", "betrothed", "sweetheart", "girlfriend", "boyfriend", "fianc", "married"]) {
                            r.partner = true;
                            r.affection = r.affection.max(0.85);
                        } else if has(&["son", "daughter", "mother", "father", "brother", "sister", "child", "parent", "grand", "aunt", "uncle", "cousin", "niece", "nephew", "twin", "family"]) {
                            r.family = true;
                            r.affection = r.affection.max(0.75);
                        } else if has(&["rival", "enemy", "foe", "distrust", "dislike", "feud", "resent", "hate", "suspicious"]) {
                            r.rivalry = r.rivalry.max(0.6);
                            r.affection = r.affection.min(-0.3);
                        } else if has(&["friend", "companion", "mentor", "apprentice", "ally", "fond", "trusts"]) {
                            r.affection = r.affection.max(0.5);
                        }
                        r.familiarity = r.familiarity.max(0.7);
                        r.trust = r.trust.max(r.affection * 0.6);
                    }
                }
                self.rels.insert(key(a, b), r);
                self.dirty = true;
            }
        }
        for i in ids {
            self.seeded.insert(i);
        }
    }
}

impl Sim {
    // ------------------------------------------------------------ proposals

    pub fn propose(&mut self, from: ActorId, to: ActorId, activity: &str, with: Option<ThingId>) -> Result<Outcome, ActErr> {
        if from == to {
            return Err(ActErr::Fail("propose it to someone else".into()));
        }
        if self.social.busy(to) {
            return Err(ActErr::Fail(format!("{} is busy", self.actor_name(to))));
        }
        let act = Activity::parse(activity);
        let thing = match (&act, with) {
            (_, Some(t)) => Some(t),
            (Activity::Catch, None) => self.find_ball(from),
            (Activity::Carry, None) => None,
            _ => None,
        };
        if matches!(act, Activity::Catch) && thing.is_none() {
            return Err(ActErr::Fail("there is nothing to throw".into()));
        }
        if matches!(act, Activity::Carry) && thing.is_none_or(|t| self.things.get(t).is_none_or(|x| x.mass() <= self.strength(from) || x.anchored)) {
            return Err(ActErr::Fail("carry what? (something too heavy for one)".into()));
        }
        let delay = 0.8 + self.rand() as f64 * 0.8;
        self.social.proposals.retain(|p| !(p.from == from && p.to == to));
        self.social.proposals.push(Proposal { from, to, activity: act.clone(), thing, at: self.t, answer_at: self.t + delay });
        let fname = self.actor_name(from);
        let tname = self.actor_name(to);
        let what = act.label();
        let at = self.actor(from).map(|a| a.pos).unwrap_or_default();
        self.event("proposed", Some(from), Some(to.key()), format!("{fname} asked {tname} to {what}"), Some(at), json!({ "activity": what }));
        if to == ActorId::Player {
            self.notes.push(Note::Info(format!("{} asks you: {what}? (y / n)", super::physics::cap(&fname))));
        } else if from != ActorId::Player {
            self.note_near(at, 20.0, Note::Line { id: Some(from), who: fname.clone(), text: format!("{}, {what}?", tname) });
        }
        Ok(Outcome::ok(format!("{fname} asks {tname} to {what}")))
    }

    fn find_ball(&mut self, who: ActorId) -> Option<ThingId> {
        let me = self.actor(who)?.clone();
        if let Some(h) = me.held.filter(|h| self.things.get(*h).is_some_and(|t| is_toy(&t.props) || (t.props[P_BOUNCE] >= 0.3 && t.mass() <= 3.0))) {
            return Some(h);
        }
        let (t, _) = self.nearest_matching(me.pos, 25.0, |p| is_toy(p) || (p[P_BOUNCE] >= 0.3 && p[P_MASS] <= 3.0))?;
        self.liven(&t)
    }

    pub fn answer(&mut self, who: ActorId, from: Option<ActorId>, yes: bool) -> Result<Outcome, ActErr> {
        let i = self.social.proposals.iter().rposition(|p| p.to == who && from.is_none_or(|f| p.from == f)).ok_or(ActErr::Fail("nobody asked you anything".into()))?;
        let p = self.social.proposals.remove(i);
        let name = self.actor_name(who);
        let other = self.actor_name(p.from);
        let what = p.activity.label();
        let at = self.actor(who).map(|a| a.pos);
        if !yes {
            self.event("declined", Some(who), Some(p.from.key()), format!("{name} said no to {other}: {what}"), at, json!({}));
            if who != ActorId::Player {
                self.say_template(who, "no", &other);
            }
            self.social.rel_mut(who, p.from).affection -= 0.02;
            return Ok(Outcome::ok(format!("{name} says no")));
        }
        self.event("accepted", Some(who), Some(p.from.key()), format!("{name} agreed to {what} with {other}"), at, json!({}));
        let kind = match p.activity {
            Activity::Catch => p.thing.map(|ball| JointKind::Catch { ball }),
            Activity::Carry => p.thing.map(|thing| {
                let lead = self.actor(p.from).cloned();
                let dest = lead.map(|a| a.pos + a.forward() * 15.0).unwrap_or_default();
                JointKind::Carry { thing, dest }
            }),
            Activity::Gesture(k) => Some(JointKind::Gesture(k)),
            Activity::Dance => Some(JointKind::Dance),
            Activity::Walk => Some(JointKind::Walk),
            Activity::Other(text) => {
                // Free-form: the one asked goes along with the asker.
                if let ActorId::Npc(c) = who {
                    self.plan(who, vec![Action::Follow { target: Target::Actor(p.from), secs: Some(60.0) }], &text, false);
                    self.set_goal(c, &text);
                }
                None
            }
        };
        if let Some(k) = kind {
            self.start_joint(k, p.from, who);
        }
        if who != ActorId::Player {
            self.say_template(who, "yes", &other);
        }
        Ok(Outcome::ok(format!("{name} agrees to {what}")))
    }

    fn set_goal(&mut self, cid: i64, g: &str) {
        if let Some(n) = self.cast.get_mut(cid) {
            n.goal = g.to_string();
        }
    }

    /// Would this character agree?
    fn npc_wants(&mut self, cid: i64, from: ActorId, act: &Activity) -> bool {
        let me = ActorId::Npc(cid);
        let Some(n) = self.cast.get(cid) else { return false };
        if n.a.asleep || (self.night() && !matches!(act, Activity::Gesture(_))) {
            return false;
        }
        let aff = self.social.affection(me, from) - self.wariness(cid);
        let rel = self.social.rel(me, from).cloned().unwrap_or_default();
        let tr = n.traits;
        let needs = n.needs;
        match act {
            Activity::Catch => needs.fun * 0.6 + tr.playful * 0.6 + aff * 0.5 > 0.45,
            Activity::Carry => tr.generous * 0.6 + aff * 0.6 + 0.1 > 0.4,
            Activity::Gesture(k) => {
                if *k == GestureKind::Kiss {
                    rel.partner || aff >= k.intimacy()
                } else {
                    aff >= k.intimacy()
                }
            }
            Activity::Dance => tr.playful * 0.5 + aff * 0.7 > 0.5,
            Activity::Walk => aff > 0.15,
            Activity::Other(_) => aff > 0.35,
        }
    }

    // ------------------------------------------------------------ joints

    pub fn start_joint(&mut self, kind: JointKind, a: ActorId, b: ActorId) {
        let id = self.social.next();
        let t = self.t;
        let until = t + match kind {
            JointKind::Catch { .. } => 150.0,
            JointKind::Carry { .. } => 180.0,
            JointKind::Gesture(k) => 30.0 + k.duration() as f64,
            JointKind::Dance => 40.0,
            JointKind::Walk => 90.0,
        };
        self.social.joints.retain(|j| !j.has(a) && !j.has(b));
        for who in [a, b] {
            if let ActorId::Npc(c) = who {
                if let Some(n) = self.cast.get_mut(c) {
                    n.plan.clear();
                    n.a.task = None;
                    n.a.asleep = false;
                }
            }
        }
        let label = match &kind {
            JointKind::Catch { .. } => "playing catch".to_string(),
            JointKind::Carry { .. } => "carrying something together".to_string(),
            JointKind::Gesture(k) => format!("going to {}", k.name().replace('_', " ")),
            JointKind::Dance => "dancing".to_string(),
            JointKind::Walk => "walking together".to_string(),
        };
        for who in [a, b] {
            if let ActorId::Npc(c) = who {
                self.set_doing(c, &label);
            }
        }
        let names = format!("{} and {}", self.actor_name(a), self.actor_name(b));
        self.event("together", Some(a), Some(b.key()), format!("{names}: {label}"), self.actor(a).map(|x| x.pos), json!({ "kind": format!("{kind:?}") }));
        self.social.joints.push(Joint { id, kind, a, b, started: t, until, phase: 0, next: t + 0.5, count: 0 });
    }

    fn end_joint(&mut self, id: u64, why: &str) {
        let Some(i) = self.social.joints.iter().position(|j| j.id == id) else { return };
        let j = self.social.joints.remove(i);
        let names = format!("{} and {}", self.actor_name(j.a), self.actor_name(j.b));
        let bond = match j.kind {
            JointKind::Catch { .. } => 0.04 + 0.01 * j.count.min(10) as f32,
            JointKind::Carry { .. } => 0.08,
            JointKind::Gesture(k) => 0.04 + k.intimacy().max(0.0) * 0.1,
            JointKind::Dance => 0.1,
            JointKind::Walk => 0.05,
        };
        if why == "done" {
            self.social.bond(j.a, j.b, bond, self.t);
            // Time spent together is a warm touch for both.
            self.kind_touch(j.a, j.b, 1.0, super::body::HUG_SECS, super::body::EMBRACE, true, true);
        }
        for who in [j.a, j.b] {
            if let ActorId::Npc(c) = who {
                if let Some(n) = self.cast.get_mut(c) {
                    n.think_at = self.t + 1.0;
                    n.doing = "idle".into();
                    if matches!(j.kind, JointKind::Catch { .. } | JointKind::Dance) {
                        n.needs.fun = (n.needs.fun - 0.3).max(0.0);
                    }
                    n.needs.social = (n.needs.social - 0.2).max(0.0);
                }
            }
        }
        if let JointKind::Carry { thing, .. } = j.kind {
            if self.things.get(thing).is_some_and(|t| t.holder.is_some()) {
                self.drop_thing(thing, Vec3::ZERO);
            }
        }
        let what = match j.kind {
            JointKind::Catch { .. } => format!("{names} finished playing catch ({} throws)", j.count),
            JointKind::Carry { .. } => format!("{names} put down what they carried"),
            JointKind::Gesture(k) => format!("{names} shared a {}", k.name().replace('_', " ")),
            JointKind::Dance => format!("{names} danced together"),
            JointKind::Walk => format!("{names} walked together"),
        };
        let at = self.actor(j.a).map(|x| x.pos);
        self.event("together_end", Some(j.a), Some(j.b.key()), if why == "done" { what.clone() } else { format!("{what} ({why})") }, at, json!({ "why": why }));
        if why == "done" {
            if let Some(p) = at {
                self.witness(p, 20.0, &what, 0.35, &[]);
                for (x, y) in [(j.a, j.b), (j.b, j.a)] {
                    if let ActorId::Npc(c) = x {
                        self.out.push(Request::Witness { cid: c, text: format!("I {} with {}.", short_what(&j.kind), self.actor_name(y)), importance: 0.45 });
                    }
                }
            }
        }
    }

    pub fn step_social(&mut self, _dt: f32) {
        let t = self.t;
        // Proposals: expire, and characters answer.
        self.social.proposals.retain(|p| t - p.at < 25.0);
        let due: Vec<Proposal> = self.social.proposals.iter().filter(|p| matches!(p.to, ActorId::Npc(_)) && t >= p.answer_at).cloned().collect();
        for p in due {
            let ActorId::Npc(c) = p.to else { continue };
            let yes = self.npc_wants(c, p.from, &p.activity);
            let _ = self.answer(p.to, Some(p.from), yes);
        }
        // Joints.
        let ids: Vec<u64> = self.social.joints.iter().map(|j| j.id).collect();
        for id in ids {
            self.step_joint(id);
        }
        // Conversations between characters.
        let wants = std::mem::take(&mut self.social.chat_want);
        for (a, b, since) in wants {
            if t - since > 60.0 {
                continue;
            }
            let (Some(pa), Some(pb)) = (self.cast.get(a).map(|n| n.a.pos), self.cast.get(b).map(|n| n.a.pos)) else { continue };
            if (pa - pb).length() > 3.5 {
                self.social.chat_want.push((a, b, since));
                continue;
            }
            self.converse(a, b);
        }
        // Lines waiting to be said.
        while self.social.chats.front().is_some_and(|c| c.at <= t) {
            let Some(c) = self.social.chats.pop_front() else { break };
            let to = Target::Actor(ActorId::Npc(c.to));
            self.say(ActorId::Npc(c.who), &c.text, Some(to));
        }
    }

    fn step_joint(&mut self, id: u64) {
        let Some(j) = self.social.joints.iter().find(|j| j.id == id).cloned() else { return };
        let t = self.t;
        let (Some(pa), Some(pb)) = (self.actor(j.a).map(|x| x.pos), self.actor(j.b).map(|x| x.pos)) else {
            self.end_joint(id, "someone left");
            return;
        };
        if t > j.until {
            self.end_joint(id, "done");
            return;
        }
        let dist = (pa - pb).length();
        if dist > 40.0 && j.phase > 0 {
            self.end_joint(id, "they drifted apart");
            return;
        }
        let set = |s: &mut Sim, f: &dyn Fn(&mut Joint)| {
            if let Some(x) = s.social.joints.iter_mut().find(|x| x.id == id) {
                f(x);
            }
        };
        match j.kind.clone() {
            JointKind::Catch { ball } => {
                let Some(b) = self.things.get(ball).cloned() else {
                    self.end_joint(id, "the ball is gone");
                    return;
                };
                if j.count >= 14 || (self.night() && j.count > 2) {
                    self.end_joint(id, "done");
                    return;
                }
                match b.holder {
                    Some(h) if j.has(h) => {
                        let other = j.other(h);
                        let op = self.actor(other).map(|x| x.pos).unwrap_or_default();
                        let hp = self.actor(h).map(|x| x.pos).unwrap_or_default();
                        let d = (op - hp).length();
                        if h == ActorId::Player {
                            // The player throws when they like.
                            if let ActorId::Npc(c) = other {
                                if let Some(n) = self.cast.get_mut(c) {
                                    n.a.task = Some(Task::Face { target: Target::Actor(ActorId::Player), until: t + 1.0 });
                                }
                            }
                            return;
                        }
                        if t < j.next {
                            if let Some(a) = self.actor_mut(h) {
                                a.face(op - hp, 0.2);
                            }
                            return;
                        }
                        if d < 3.0 {
                            let back = hp + (hp - op).normalize_or_zero() * 4.0;
                            self.set_task(h, Task::Goto { target: Target::Point(back.to_array()), stop: 0.5, run: false, deadline: t + 6.0 });
                            set(self, &|x| x.next = t + 1.5);
                            return;
                        }
                        if d > 15.0 {
                            self.set_task(h, Task::Goto { target: Target::Actor(other), stop: 9.0, run: false, deadline: t + 10.0 });
                            set(self, &|x| x.next = t + 1.0);
                            return;
                        }
                        if self.rand() < 0.4 {
                            let on = self.actor_name(other);
                            self.say_template(h, "catch", &on);
                        }
                        let _ = self.act(h, Action::Throw { at: Some(Target::Actor(other)), dir: None, force: None });
                        set(self, &|x| {
                            x.next = t + 5.0;
                            x.phase = 1;
                        });
                    }
                    Some(_) => self.end_joint(id, "someone else took the ball"),
                    None => {
                        // In the air, or lying somewhere: whoever is nearest fetches it.
                        if b.asleep && t > j.next - 3.5 {
                            let da = (pa - b.pos).length();
                            let db = (pb - b.pos).length();
                            let fetch = if j.b == ActorId::Player || (da <= db && j.a != ActorId::Player) { j.a } else { j.b };
                            if fetch != ActorId::Player && self.actor(fetch).is_some_and(|x| x.task.is_none()) {
                                if let ActorId::Npc(c) = fetch {
                                    if self.cast.get(c).is_some_and(|n| n.plan.is_empty()) {
                                        if j.phase == 1 && self.rand() < 0.5 {
                                            let other = self.actor_name(j.other(fetch));
                                            self.say_template(fetch, "missed", &other);
                                        }
                                        self.plan(fetch, vec![Action::Hold { target: Target::Thing(ball) }], "fetch the ball", false);
                                        set(self, &|x| {
                                            x.next = t + 2.0;
                                            x.phase = 2;
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
            JointKind::Carry { thing, dest } => {
                let Some(th) = self.things.get(thing).cloned() else {
                    self.end_joint(id, "it is gone");
                    return;
                };
                if j.phase == 0 {
                    // Both take hold.
                    for who in [j.a, j.b] {
                        let holding = th.holder == Some(who) || th.co_holder == Some(who);
                        if !holding && who != ActorId::Player && self.actor(who).is_some_and(|x| x.task.is_none()) {
                            let idle = match who {
                                ActorId::Npc(c) => self.cast.get(c).is_some_and(|n| n.plan.is_empty()),
                                _ => false,
                            };
                            if idle {
                                let mut steps = Vec::new();
                                if self.actor(who).is_some_and(|x| x.held.is_some_and(|h| h != thing)) {
                                    steps.push(Action::Drop);
                                }
                                steps.push(Action::Hold { target: Target::Thing(thing) });
                                self.plan(who, steps, "lift it together", false);
                            }
                        }
                    }
                    if th.holder.is_some() && th.co_holder.is_some() {
                        set(self, &|x| {
                            x.phase = 1;
                            x.next = t;
                        });
                    }
                    return;
                }
                if th.holder.is_none() || th.co_holder.is_none() {
                    self.end_joint(id, "done");
                    return;
                }
                // The leader walks; the other keeps step.
                let (lead, follow) = if j.b == ActorId::Player { (j.b, j.a) } else { (j.a, j.b) };
                if follow != ActorId::Player {
                    self.set_task(follow, Task::Follow { who: lead, dist: 1.3, until: t + 2.0 });
                }
                if lead != ActorId::Player {
                    let lp = self.actor(lead).map(|x| x.pos).unwrap_or_default();
                    if (Vec3::new(dest.x - lp.x, 0.0, dest.z - lp.z)).length() < 1.2 {
                        self.end_joint(id, "done");
                    } else if self.actor(lead).is_some_and(|x| x.task.is_none()) {
                        let fp = self.actor(follow).map(|x| x.pos).unwrap_or_default();
                        // Wait for the other end before walking on.
                        if (fp - lp).length() < 2.4 {
                            self.set_task(lead, Task::Goto { target: Target::Point(dest.to_array()), stop: 1.0, run: false, deadline: t + 2.0 });
                        }
                    }
                }
            }
            JointKind::Gesture(k) => {
                let want = k.distance() + self.contact_gap(j.a, j.b);
                if j.phase == 0 {
                    // Come close: characters walk to the other.
                    for (who, other) in [(j.a, j.b), (j.b, j.a)] {
                        if who == ActorId::Player {
                            continue;
                        }
                        if dist > want + 0.25 && self.actor(who).is_some_and(|x| x.task.is_none()) {
                            let stop = if other == ActorId::Player || who == j.b { want } else { want + 0.6 };
                            self.set_task(who, Task::Goto { target: Target::Actor(other), stop, run: false, deadline: t + 20.0 });
                        }
                    }
                    if dist <= want + 0.3 {
                        for (who, other) in [(j.a, j.b), (j.b, j.a)] {
                            let op = self.actor(other).map(|x| x.pos).unwrap_or_default();
                            if let Some(x) = self.actor_mut(who) {
                                x.task = None;
                                let d = op - x.pos;
                                x.face(d, 10.0);
                                x.gesture = Some(GestureRun { kind: k, with: Some(other), t0: t, dur: k.duration() });
                            }
                        }
                        let names = format!("{} and {}", self.actor_name(j.a), self.actor_name(j.b));
                        let verb = match k {
                            GestureKind::Hug => "hug",
                            GestureKind::Kiss => "kiss",
                            GestureKind::Handshake => "shake hands",
                            GestureKind::HighFive => "high-five",
                            _ => "gesture",
                        };
                        let msg = format!("{names} {verb}");
                        self.event(&format!("{}", k.name()), Some(j.a), Some(j.b.key()), msg.clone(), Some(pa), json!({}));
                        // Bodies that touch run the world's rules between them, warmly.
                        if k.contact() {
                            self.touch_bodies(j.a, j.b, 0.5, 0.5, super::body::HUG_SECS, super::body::EMBRACE);
                        }
                        self.note_near(pa, 25.0, Note::seen(format!("{msg}."), j.has(ActorId::Player)));
                        set(self, &|x| {
                            x.phase = 1;
                            x.next = t + k.duration() as f64;
                        });
                    } else if t - j.started > 25.0 {
                        self.end_joint(id, "they never met");
                    }
                } else if t >= j.next {
                    self.end_joint(id, "done");
                }
            }
            JointKind::Dance => {
                if j.phase == 0 {
                    if dist > 2.2 {
                        for (who, other) in [(j.a, j.b), (j.b, j.a)] {
                            if who != ActorId::Player && self.actor(who).is_some_and(|x| x.task.is_none()) {
                                self.set_task(who, Task::Goto { target: Target::Actor(other), stop: 1.5, run: false, deadline: t + 20.0 });
                            }
                        }
                        return;
                    }
                    set(self, &|x| {
                        x.phase = 1;
                        x.next = t;
                    });
                }
                if t >= j.next {
                    for (who, other) in [(j.a, j.b), (j.b, j.a)] {
                        let op = self.actor(other).map(|x| x.pos).unwrap_or_default();
                        if let Some(x) = self.actor_mut(who) {
                            let d = op - x.pos;
                            x.face(d, 10.0);
                            x.gesture = Some(GestureRun { kind: GestureKind::Dance, with: Some(other), t0: t, dur: 6.0 });
                        }
                    }
                    let c = j.count;
                    set(self, &|x| {
                        x.next = t + 6.0;
                        x.count = c + 1;
                    });
                    if c >= 2 {
                        self.end_joint(id, "done");
                    }
                }
            }
            JointKind::Walk => {
                if j.b != ActorId::Player && self.actor(j.b).is_some_and(|x| x.task.is_none()) {
                    self.set_task(j.b, Task::Follow { who: j.a, dist: 1.8, until: t + 3.0 });
                }
                if j.a != ActorId::Player && self.actor(j.a).is_some_and(|x| x.task.is_none()) {
                    let p = pa + self.actor(j.a).map(|x| x.forward()).unwrap_or(Vec3::Z) * 12.0;
                    self.set_task(j.a, Task::Goto { target: Target::Point(p.to_array()), stop: 1.0, run: false, deadline: t + 15.0 });
                }
            }
        }
    }

    // ------------------------------------------------------------ catching and gifts

    /// `who` sees something thrown at them: get ready to catch it.
    pub fn ready_to_catch(&mut self, who: ActorId, from: ActorId, secs: f64) {
        let t = self.t;
        let willing = match who {
            ActorId::Player => true,
            ActorId::Npc(c) => {
                let in_game = self.social.joint_of(who).is_some_and(|j| j.has(from));
                let n = self.cast.get(c);
                n.is_some_and(|n| !n.a.asleep && n.a.held.is_none()) && (in_game || self.social.affection(who, from) > -0.3)
            }
        };
        if !willing {
            return;
        }
        let fp = self.actor(from).map(|a| a.pos).unwrap_or_default();
        if let Some(a) = self.actor_mut(who) {
            a.catching = t + secs;
            let d = fp - a.pos;
            a.face(d, 10.0);
            if who != ActorId::Player {
                a.task = Some(Task::Face { target: Target::Actor(from), until: t + secs });
            }
        }
    }

    pub fn on_caught(&mut self, who: ActorId, thrower: Option<ActorId>, _id: ThingId) {
        let t = self.t;
        if let Some(th) = thrower {
            if let Some(j) = self.social.joints.iter_mut().find(|j| j.has(who) && j.has(th) && matches!(j.kind, JointKind::Catch { .. })) {
                j.count += 1;
                j.next = t + 1.2;
                j.phase = 1;
            }
            self.social.bond(who, th, 0.01, t);
            if who != ActorId::Player && self.rand() < 0.35 {
                let n = self.actor_name(th);
                self.say_template(who, "caught", &n);
            }
        }
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get_mut(c) {
                n.needs.fun = (n.needs.fun - 0.05).max(0.0);
            }
        }
    }

    /// Someone threw something through an opening: a little joy for them and
    /// anyone watching who likes them.
    pub fn on_scored(&mut self, who: ActorId, what: &str) {
        let _ = what;
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get_mut(c) {
                n.needs.fun = (n.needs.fun - 0.15).max(0.0);
            }
            if self.rand() < 0.5 {
                self.say_template(who, "cheer", "");
            }
            let t = self.t;
            if let Some(n) = self.cast.get_mut(c) {
                n.a.gesture = Some(GestureRun { kind: GestureKind::Cheer, with: None, t0: t + 0.3, dur: 2.0 });
            }
        }
    }

    pub fn npc_accepts_gift(&mut self, cid: i64, from: ActorId, _id: ThingId) -> bool {
        let me = ActorId::Npc(cid);
        self.social.rel(me, from).is_none_or(|r| r.rivalry < 0.7) && self.cast.get(cid).is_some_and(|n| !n.a.asleep)
    }

    pub fn on_gift(&mut self, to: ActorId, from: ActorId, id: ThingId) {
        let ActorId::Npc(c) = to else { return };
        let fname = self.actor_name(from);
        self.say_template(to, "thanks", &fname);
        let tname = self.thing_name(id);
        self.out.push(Request::Witness { cid: c, text: format!("{fname} gave me {} {tname}.", super::article(&tname)), importance: 0.6 });
        self.ask(c, "got_gift", &format!("{fname} just gave you {} {tname}.", super::article(&tname)));
    }

    // ------------------------------------------------------------ gestures

    pub fn contact_gesture(&mut self, who: ActorId, other: ActorId, k: GestureKind) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let oname = self.actor_name(other);
        // Busy with each other (a game of catch): stop for a hug.
        if let Some(j) = self.social.joint_of(who).filter(|j| j.has(other)).map(|j| j.id) {
            self.end_joint(j, "done");
        }
        if self.social.busy(who) || self.social.busy(other) {
            return Err(ActErr::Fail(format!("{oname} is busy")));
        }
        match other {
            ActorId::Player => {
                self.propose(who, other, &k.name(), None)?;
                Ok(Outcome::ok(format!("{name} offers {oname} a {}", k.name().replace('_', " "))))
            }
            ActorId::Npc(c) => {
                if !self.npc_wants(c, who, &Activity::Gesture(k)) {
                    let at = self.actor(other).map(|a| a.pos);
                    self.event("refused", Some(other), Some(who.key()), format!("{oname} stepped back from {name}'s {}", k.name().replace('_', " ")), at, json!({}));
                    if let Some(p) = at {
                        self.note_near(p, 20.0, Note::seen(format!("{oname} steps back."), who == ActorId::Player));
                    }
                    self.social.rel_mut(who, other).affection -= 0.03;
                    return Err(ActErr::Fail(format!("{oname} steps back")));
                }
                self.start_joint(JointKind::Gesture(k), who, other);
                Ok(Outcome::ok(format!("{name} and {oname} {}", k.name().replace('_', " "))))
            }
        }
    }

    /// Someone waved (bowed, pointed…) at a character.
    pub fn on_gestured(&mut self, who: ActorId, from: ActorId, k: GestureKind) {
        let ActorId::Npc(c) = who else { return };
        let aff = self.social.affection(who, from);
        if self.cast.get(c).is_none_or(|n| n.a.asleep) {
            return;
        }
        let back = match k {
            GestureKind::Wave if aff > -0.2 => Some(GestureKind::Wave),
            GestureKind::Bow => Some(GestureKind::Nod),
            GestureKind::Cheer if aff > 0.2 => Some(GestureKind::Cheer),
            GestureKind::Dance if aff > 0.3 => Some(GestureKind::Dance),
            _ => None,
        };
        let t = self.t;
        if let Some(b) = back {
            let fp = self.actor(from).map(|a| a.pos).unwrap_or_default();
            if let Some(n) = self.cast.get_mut(c) {
                let d = fp - n.a.pos;
                n.a.face(d, 10.0);
                n.a.gesture = Some(GestureRun { kind: b, with: Some(from), t0: t + 0.5, dur: b.duration() });
            }
            self.social.bond(who, from, 0.01, t);
        }
        let fname = self.actor_name(from);
        self.npc_note_event(c, &format!("{fname} {}ed at me", k.name()));
    }

    // ------------------------------------------------------------ talk

    /// Two characters meet and talk: through the LLM if the player can hear,
    /// otherwise in a few plain words. They also pass on news (gossip).
    fn converse(&mut self, a: i64, b: i64) {
        let (Some(pa), Some(_)) = (self.cast.get(a).map(|n| n.a.pos), self.cast.get(b)) else { return };
        // With the dark close, people fall quiet.
        if self.dread_near(pa, 35.0) {
            return;
        }
        // Without shared words there is no talk and no news: a noise, a look.
        if !self.speaks(ActorId::Npc(a)) || !self.speaks(ActorId::Npc(b)) {
            for x in [a, b] {
                if !self.speaks(ActorId::Npc(x)) {
                    let other = if x == a { b } else { a };
                    let happy = self.social.affection(ActorId::Npc(x), ActorId::Npc(other)) > -0.2;
                    self.make_noise(x, happy);
                }
            }
            self.social.bond(ActorId::Npc(a), ActorId::Npc(b), 0.02, self.t);
            return;
        }
        let an = self.actor_name(ActorId::Npc(a));
        let bn = self.actor_name(ActorId::Npc(b));
        let t = self.t;
        for (x, y) in [(a, b), (b, a)] {
            let yp = self.actor(ActorId::Npc(y)).map(|q| q.pos).unwrap_or_default();
            if let Some(n) = self.cast.get_mut(x) {
                n.a.task = Some(Task::Face { target: Target::Actor(ActorId::Npc(y)), until: t + 8.0 });
                let d = yp - n.a.pos;
                n.a.face(d, 10.0);
                n.needs.social = (n.needs.social - 0.3).max(0.0);
            }
        }
        let near_player = self.dist_to_player(pa) <= self.cfg.chat_llm_range;
        if near_player && self.has_llm {
            let rel = self.social.rel(ActorId::Npc(a), ActorId::Npc(b)).map(|r| r.describe()).unwrap_or_default();
            let ctx_a = self.decide_context(a, &format!("You meet {bn} ({rel}) and stop to talk."));
            self.request(Request::Chat { a, b, context: ctx_a }, pa);
        } else {
            let lines = self.small_talk(a, b);
            for (i, (who, text)) in lines.into_iter().enumerate() {
                let to = if who == a { b } else { a };
                self.social.chats.push_back(ChatLine { at: t + 0.2 + i as f64 * 2.4, who, to, text });
            }
        }
        // Gossip: the most important recent thing one remembers, the other now knows.
        for (x, y) in [(a, b), (b, a)] {
            let mems = self.db.memories(x).unwrap_or_default();
            let xn = self.actor_name(ActorId::Npc(x));
            if let Some(m) = mems.iter().rev().take(12).filter(|m| m.importance >= 0.4 && !m.text.starts_with(&format!("{} told me", self.actor_name(ActorId::Npc(y))))).max_by(|p, q| p.importance.total_cmp(&q.importance)) {
                let text = format!("{xn} told me: {}", m.text.trim_start_matches("I ").trim());
                self.out.push(Request::Witness { cid: y, text, importance: (m.importance * 0.7).min(0.6) });
            }
        }
        self.social.bond(ActorId::Npc(a), ActorId::Npc(b), 0.03, t);
        self.event("chatted", Some(ActorId::Npc(a)), Some(ActorId::Npc(b).key()), format!("{an} and {bn} talked"), Some(pa), json!({}));
    }

    /// A few plain lines when no LLM speaks for them: a greeting if they
    /// haven't met for a while, then something one of them saw, or how
    /// they feel.
    fn small_talk(&mut self, a: i64, b: i64) -> Vec<(i64, String)> {
        let first = |s: &Sim, c: i64| s.actor_name(ActorId::Npc(c)).split_whitespace().next().unwrap_or("").to_string();
        let (af, bf) = (first(self, a), first(self, b));
        let t = self.t;
        let hour = self.hour();
        let k = crate::noise::pcg((t * 10.0) as u32 ^ (a as u32).wrapping_mul(31) ^ b as u32);
        let mut out = Vec::new();
        let last = self.social.rel(ActorId::Npc(a), ActorId::Npc(b)).map(|r| r.last).unwrap_or(0.0);
        if last == 0.0 || t - last > 600.0 {
            out.push((a, super::npc::template("greet", &bf, hour, k)));
            out.push((b, super::npc::template("greet", &af, hour, k / 7)));
        }
        // What they saw lately that mattered most to them (whatever it was:
        // nothing here lists what counts as news); something big stays news
        // for longer.
        let fresh = |n: &super::npc::Npc| n.news.iter().filter(|(at, _, imp)| *imp >= 0.3 && t - at < 600.0 * (1.0 + 2.0 * *imp as f64)).cloned().collect::<Vec<_>>();
        let mine = self.cast.get(a).map(fresh).unwrap_or_default();
        let theirs = self.cast.get(b).map(fresh).unwrap_or_default();
        let seen = mine.iter().max_by(|x, y| x.2.total_cmp(&y.2).then(x.0.total_cmp(&y.0)));
        match seen {
            Some((_, text, _)) => {
                let both = theirs.iter().any(|(_, x, _)| x == text);
                out.push((a, format!("Have you heard? {}", super::physics::cap(text.trim()))));
                let reply = if both { ["I saw it too!", "I did. Strange times.", "Hard to miss."][(k % 3) as usize] } else { ["No! Really?", "I missed that.", "You're joking."][(k % 3) as usize] };
                out.push((b, reply.to_string()));
            }
            None => {
                let needs = self.cast.get(a).map(|n| n.needs).unwrap_or_default();
                let line = if needs.hunger > 0.6 {
                    "I could eat a horse."
                } else if needs.fatigue > 0.6 {
                    "I'm worn through today."
                } else if needs.fun > 0.7 {
                    "Nothing ever happens round here."
                } else {
                    ["Fine weather for it.", "Quiet day.", "All well at home?", "Busy, busy."][(k % 4) as usize]
                };
                out.push((a, line.to_string()));
                if out.len() < 4 {
                    out.push((b, ["Aye.", "Isn't it.", "You're not wrong.", "Mm."][(k / 3 % 4) as usize].to_string()));
                }
            }
        }
        out
    }

    /// Lines of an overheard conversation arrived.
    pub fn on_chat(&mut self, a: i64, b: i64, lines: Vec<(i64, String)>) {
        let t = self.t;
        for (i, (who, text)) in lines.into_iter().take(6).enumerate() {
            let to = if who == a { b } else { a };
            if who != a && who != b {
                continue;
            }
            self.social.chats.push_back(ChatLine { at: t + 0.3 + i as f64 * 2.8, who, to, text });
        }
    }
}

fn short_what(k: &JointKind) -> String {
    match k {
        JointKind::Catch { .. } => "played catch".into(),
        JointKind::Carry { .. } => "carried something heavy".into(),
        JointKind::Gesture(g) => format!("shared a {}", g.name().replace('_', " ")),
        JointKind::Dance => "danced".into(),
        JointKind::Walk => "went for a walk".into(),
    }
}
