//! Goals as data. A goal is a row: whose it is, what they want (a condition
//! the sim can check where it can be, words otherwise), why, how much it
//! matters, by when, who it was promised to, and what they did toward it.
//!
//! Goals come from who people are (their persona's aims), from missions (a
//! deed that needs something), from promises (agreeing to make or fetch
//! something for someone), from trouble (pushing back a fire or a curse)
//! and from the planner's own longer aims. The sim checks them every few
//! seconds: one that is met is done (remembered, and a promisee is glad);
//! one past its deadline, or that can no longer be met, is given up
//! (remembered, and a promisee trusts them less). Words the sim can't check
//! get one look from the owner's planner, at their deadline.

use super::things::ThingId;
use super::{ActorId, Request, Sim};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// How often goals are checked (s).
pub const CHECK_SECS: f64 = 2.0;
/// How long a closed goal is kept (for the planner and the save), in days.
const KEEP_DAYS: f64 = 1.0;
/// Open goals a character keeps at most (the least important go first).
const MAX_OPEN: usize = 8;
/// How long a planner has to say whether a goal in words was met (s).
const JUDGE_SECS: f64 = 120.0;

/// What a goal wants, checkable where it can be.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Want {
    /// A thing like this in the owner's hands.
    Hold { what: String },
    /// Someone has a thing like this (it was handed to them, or they hold it).
    Has { who: ActorId, what: String },
    /// The owner is at a place (by name).
    Be { place: String },
    /// A thing is at a place.
    At { thing: ThingId, place: [f32; 3] },
    /// An incident (a fire, a spreading curse) is over.
    Over { incident: u32 },
    /// How the owner feels about someone reaches a level.
    Affection { with: ActorId, at_least: f32 },
    /// Anything else, in words: the owner's planner judges it at its deadline.
    Words { text: String },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    Done,
    Dropped,
}

/// Where a goal came from.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Persona,
    Mission,
    Promise,
    Incident,
    Plan,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Goal {
    pub id: u64,
    pub owner: i64,
    pub want: Want,
    /// What it is, in their words ("make the traveler a ball").
    pub text: String,
    /// Why ("they asked me", "who I am").
    #[serde(default)]
    pub why: String,
    pub priority: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<f64>,
    #[serde(default)]
    pub progress: f32,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promised_to: Option<ActorId>,
    /// What they did toward it, latest last (a few).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<String>,
    pub source: Source,
    pub since: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed: Option<f64>,
    /// Its planner was asked whether it was met (until this game time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judging: Option<f64>,
}

impl Goal {
    pub fn open(&self) -> bool {
        self.status == Status::Open
    }

    /// Someone else is in it: a promise, or wanting something of another.
    pub fn crosses(&self) -> bool {
        self.promised_to.is_some() || matches!(self.want, Want::Has { who, .. } | Want::Affection { with: who, .. } if who != ActorId::Npc(self.owner))
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Goals {
    pub list: Vec<Goal>,
    #[serde(default)]
    pub next: u64,
    /// Log events already looked at (for steps).
    #[serde(skip)]
    pub seen: u64,
    #[serde(skip)]
    pub acc: f64,
}

/// A promise or aim as the planner writes it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Pledge {
    /// What, in a few words ("make the traveler a ball").
    #[serde(default)]
    pub text: String,
    /// Who it is for (a name, "the traveler"), for a promise.
    #[serde(default)]
    pub to: Option<String>,
    /// The thing they will hand over, if any ("a leather ball").
    #[serde(default)]
    pub what: Option<String>,
    #[serde(default)]
    pub within_hours: Option<f32>,
}

/// Whether words naming a kind of thing fit a thing's name, the way people
/// mean them: "a ball" fits "leather ball".
pub fn fits(what: &str, name: &str) -> bool {
    let tidy = |s: &str| s.trim().to_lowercase().trim_start_matches("the ").trim_start_matches("a ").trim_start_matches("an ").trim().to_string();
    let (q, n) = (tidy(what), tidy(name));
    if q.is_empty() || n.is_empty() {
        return q.is_empty();
    }
    q == n || n.contains(&q) || q.contains(&n) || n.split_whitespace().any(|w| w.len() > 2 && q.split_whitespace().any(|x| x == w))
}

impl Sim {
    /// Open goals of a character, most important first.
    pub fn goals_of(&self, cid: i64) -> Vec<&Goal> {
        let mut v: Vec<&Goal> = self.goals.list.iter().filter(|g| g.owner == cid && g.open()).collect();
        v.sort_by(|a, b| b.priority.total_cmp(&a.priority).then(a.id.cmp(&b.id)));
        v
    }

    /// Take on a goal (the same want again only refreshes the one there is).
    /// Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub fn add_goal(&mut self, owner: i64, want: Want, text: &str, why: &str, priority: f32, deadline: Option<f64>, promised_to: Option<ActorId>, source: Source) -> u64 {
        let t = self.t;
        if let Some(g) = self.goals.list.iter_mut().find(|g| g.owner == owner && g.open() && g.want == want) {
            g.priority = g.priority.max(priority);
            g.deadline = match (g.deadline, deadline) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
            g.promised_to = g.promised_to.or(promised_to);
            return g.id;
        }
        self.goals.next = self.goals.next.max(1);
        let id = self.goals.next;
        self.goals.next += 1;
        let g = Goal { id, owner, want, text: text.trim().to_string(), why: why.trim().to_string(), priority: priority.clamp(0.0, 1.0), deadline, progress: 0.0, status: Status::Open, promised_to, steps: Vec::new(), source, since: t, closed: None, judging: None };
        let crosses = g.crosses();
        let name = self.actor_name(ActorId::Npc(owner));
        let line = match promised_to {
            Some(p) => format!("{name} promised {}: {}", self.actor_name(p), g.text),
            None => format!("{name} set out to {}", g.text),
        };
        let at = self.actor(ActorId::Npc(owner)).map(|a| a.pos);
        // Who they are isn't news; what they set out to do is.
        if source != Source::Persona {
            self.event("goal_set", Some(ActorId::Npc(owner)), promised_to.map(|p| p.key()), line, at, json!({ "goal": id, "crosses": crosses, "source": source }));
        }
        self.goals.list.push(g);
        // Too many at once: the least important open one is let go.
        let open: Vec<(u64, f32)> = self.goals.list.iter().filter(|g| g.owner == owner && g.open()).map(|g| (g.id, g.priority)).collect();
        if open.len() > MAX_OPEN {
            if let Some((least, _)) = open.into_iter().filter(|(gid, _)| *gid != id).min_by(|a, b| a.1.total_cmp(&b.1)) {
                self.close_goal(least, false, "too much else to do");
            }
        }
        id
    }

    /// A promise: to make or fetch something for someone, or anything in words.
    pub fn promise(&mut self, owner: i64, to: ActorId, text: &str, what: Option<&str>, hours: Option<f32>) -> u64 {
        let hours = hours.filter(|h| h.is_finite() && *h > 0.0).unwrap_or(6.0).clamp(0.5, 48.0) as f64;
        let deadline = Some(self.t + hours * super::headless::hour_s());
        let want = match what.map(str::trim).filter(|w| !w.is_empty()) {
            Some(w) => Want::Has { who: to, what: w.to_string() },
            None => Want::Words { text: text.to_string() },
        };
        let why = format!("{} asked", self.actor_name(to));
        self.add_goal(owner, want, text, &why, 0.8, deadline, Some(to), Source::Promise)
    }

    /// Who a character is: their persona's aims become goals, once.
    pub fn seed_goals(&mut self) {
        let ids: Vec<i64> = self.cast.npcs.iter().filter(|n| !n.dead && self.mind_of(ActorId::Npc(n.def.id)) == crate::world::species::Mind::Sapient).map(|n| n.def.id).collect();
        for cid in ids {
            if self.goals.list.iter().any(|g| g.owner == cid && g.source == Source::Persona) {
                continue;
            }
            let Some(n) = self.cast.get(cid) else { continue };
            let pe = n.def.persona.clone();
            for a in pe.aims.iter().take(3) {
                let want = a.want.as_ref().and_then(|w| self.want_from(cid, w)).unwrap_or_else(|| Want::Words { text: a.text.clone() });
                if !a.text.trim().is_empty() {
                    self.add_goal(cid, want, &a.text, if a.why.is_empty() { "who I am" } else { &a.why }, 0.4, None, None, Source::Persona);
                }
            }
            let words: String = pe.goals.trim().chars().take(160).collect();
            if !words.is_empty() && pe.aims.is_empty() {
                self.add_goal(cid, Want::Words { text: words.clone() }, &words, "who I am", 0.3, None, None, Source::Persona);
            }
        }
    }

    /// A structured want as written (names resolved by who is around).
    fn want_from(&mut self, cid: i64, v: &Value) -> Option<Want> {
        let kind = v.get("kind")?.as_str()?.trim().to_lowercase();
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string).unwrap_or_default();
        let at = self.actor(ActorId::Npc(cid)).map(|a| a.pos).unwrap_or_default();
        let me = ActorId::Npc(cid);
        let who = |sim: &mut Sim, k: &str| match sim.find_named(&s(k), at, me) {
            Some(super::Target::Actor(a)) => Some(a),
            _ => None,
        };
        Some(match kind.as_str() {
            "hold" => Want::Hold { what: s("what") },
            "has" => Want::Has { who: who(self, "who")?, what: s("what") },
            "be" => Want::Be { place: s("place") },
            "affection" => Want::Affection { with: who(self, "with")?, at_least: v.get("at_least").and_then(|x| x.as_f64()).unwrap_or(0.6) as f32 },
            _ => return None,
        })
    }

    /// A gift from one to another: promises of it are kept.
    pub fn goals_on_gift(&mut self, from: ActorId, to: ActorId, id: ThingId) {
        let ActorId::Npc(owner) = from else { return };
        let name = self.thing_name(id);
        for g in self.goals.list.iter_mut().filter(|g| g.owner == owner && g.open()) {
            if let Want::Has { who, what } = &g.want {
                if *who == to && fits(what, &name) {
                    g.progress = 1.0;
                }
            }
        }
    }

    /// Whether a goal is met (true), can no longer be (false), or neither yet.
    fn goal_state(&mut self, g: &Goal) -> Option<bool> {
        let me = ActorId::Npc(g.owner);
        let alive = |s: &Sim, a: ActorId| match a {
            ActorId::Player => true,
            ActorId::Npc(c) => s.cast.get(c).is_some_and(|n| !n.dead),
        };
        if !alive(self, me) {
            return Some(false);
        }
        let held_fits = |s: &Sim, a: ActorId, what: &str| s.actor(a).and_then(|x| x.held).is_some_and(|h| fits(what, &s.thing_name(h)));
        match &g.want {
            Want::Hold { what } => held_fits(self, me, what).then_some(true),
            Want::Has { who, what } => {
                if !alive(self, *who) {
                    Some(false)
                } else {
                    (g.progress >= 1.0 || held_fits(self, *who, what)).then_some(true)
                }
            }
            Want::Be { place } => {
                let at = self.actor(me).map(|a| a.pos)?;
                let there = self.find_named(place, at, me).and_then(|t| self.resolve(&t, me)).map(|r| r.pos)?;
                (Vec3::new(there.x - at.x, 0.0, there.z - at.z).length() < 6.0).then_some(true)
            }
            Want::At { thing, place } => {
                let t = self.things.get(*thing);
                match t {
                    None => Some(false),
                    Some(t) => ((t.pos - Vec3::from(*place)).length() < 3.0).then_some(true),
                }
            }
            Want::Over { incident } => self.incidents.get(*incident).is_none_or(|i| i.ended.is_some()).then_some(true),
            Want::Affection { with, at_least } => (self.social.affection(me, *with) >= *at_least).then_some(true),
            Want::Words { .. } => None,
        }
    }

    /// Check every open goal; called each second, works every `CHECK_SECS`.
    pub fn step_goals(&mut self, dt: f64) {
        self.goals.acc += dt;
        if self.goals.acc < CHECK_SECS {
            return;
        }
        self.goals.acc = 0.0;
        self.note_goal_steps();
        let t = self.t;
        let open: Vec<Goal> = self.goals.list.iter().filter(|g| g.open()).cloned().collect();
        for g in open {
            match self.goal_state(&g) {
                Some(true) => self.close_goal(g.id, true, ""),
                Some(false) => self.close_goal(g.id, false, "it can't be done now"),
                None => {
                    let due = g.deadline.is_some_and(|d| t > d);
                    if !due {
                        continue;
                    }
                    match (&g.want, g.judging) {
                        // Words: one look from the owner's planner, at the deadline.
                        (Want::Words { .. }, None) if self.has_llm => {
                            let text = format!(
                                "You set out to {}{}, and the time for it has come. Has it been done? Reply with \"kept\": true if it has, false if not, and steps only if you will still do something about it now.",
                                g.text,
                                g.promised_to.map(|p| format!(" (you promised {})", self.actor_name(p))).unwrap_or_default()
                            );
                            if self.ask_planner_now(g.owner, "goal_due", &text) {
                                if let Some(x) = self.goals.list.iter_mut().find(|x| x.id == g.id) {
                                    x.judging = Some(t + JUDGE_SECS);
                                }
                            } else {
                                self.close_goal(g.id, false, "the time ran out");
                            }
                        }
                        (Want::Words { .. }, Some(until)) if t <= until => {}
                        _ => self.close_goal(g.id, false, "the time ran out"),
                    }
                }
            }
        }
        // Closed goals are kept a day, then let go.
        let keep = crate::render::sky::DAY_SECONDS * KEEP_DAYS;
        self.goals.list.retain(|g| g.closed.is_none_or(|c| t - c < keep));
    }

    /// What the owners did since the last look, kept on their open goals.
    fn note_goal_steps(&mut self) {
        let total = self.log.total;
        let new = (total.saturating_sub(self.goals.seen) as usize).min(self.log.recent.len());
        self.goals.seen = total;
        if new == 0 {
            return;
        }
        let events: Vec<(i64, String)> = self.log.recent.iter().rev().take(new).filter_map(|e| match (e.actor, e.data.get("goal")) {
            (Some(ActorId::Npc(c)), None) => Some((c, e.text.clone())),
            _ => None,
        }).collect();
        for (c, text) in events.into_iter().rev() {
            for g in self.goals.list.iter_mut().filter(|g| g.owner == c && g.open() && g.source != Source::Persona) {
                g.steps.push(text.chars().take(80).collect());
                if g.steps.len() > 5 {
                    g.steps.remove(0);
                }
            }
        }
    }

    /// Close a goal: done, or given up (and why). Remembered by the owner;
    /// a promisee is glad, or trusts them less.
    pub fn close_goal(&mut self, id: u64, done: bool, why: &str) {
        let t = self.t;
        let Some(g) = self.goals.list.iter_mut().find(|g| g.id == id && g.open()) else { return };
        g.status = if done { Status::Done } else { Status::Dropped };
        g.closed = Some(t);
        g.judging = None;
        let g = g.clone();
        let me = ActorId::Npc(g.owner);
        let name = self.actor_name(me);
        let at = self.actor(me).map(|a| a.pos);
        let to = g.promised_to.map(|p| self.actor_name(p));
        let (kind, line, memory) = match (done, &to) {
            (true, Some(p)) => ("goal_met", format!("{name} kept their promise to {p}: {}", g.text), format!("I kept my promise to {p}: {}.", g.text)),
            (true, None) => ("goal_met", format!("{name} managed to {}", g.text), format!("I managed to {}.", g.text)),
            (false, Some(p)) => ("goal_dropped", format!("{name} broke their promise to {p}: {} ({why})", g.text), format!("I didn't keep my promise to {p}: {} ({why}).", g.text)),
            (false, None) => ("goal_dropped", format!("{name} gave up trying to {} ({why})", g.text), format!("I gave up trying to {} ({why}).", g.text)),
        };
        self.event(kind, Some(me), g.promised_to.map(|p| p.key()), line, at, json!({ "goal": id, "source": g.source }));
        self.out.push(Request::Witness { cid: g.owner, text: memory, importance: if g.promised_to.is_some() { 0.6 } else { 0.4 } });
        let Some(p) = g.promised_to else { return };
        let r = self.social.rel_mut(p, me);
        if done {
            r.trust = (r.trust + 0.1).min(1.0);
            r.affection = (r.affection + 0.05).min(1.0);
        } else {
            r.trust = (r.trust - 0.2).max(-1.0);
            r.affection = (r.affection - 0.05).max(-1.0);
        }
        self.social.dirty = true;
        if let ActorId::Npc(pc) = p {
            let text = if done { format!("{name} kept their promise to me: {}.", g.text) } else { format!("{name} promised me {} and didn't do it.", g.text) };
            self.out.push(Request::Witness { cid: pc, text, importance: 0.5 });
        }
    }

    /// A planner's answer about a goal in words that came due.
    pub fn goal_judged(&mut self, cid: i64, kept: bool) {
        let t = self.t;
        let Some(id) = self.goals.list.iter().find(|g| g.owner == cid && g.open() && g.judging.is_some_and(|u| t <= u + 1.0)).map(|g| g.id) else { return };
        self.close_goal(id, kept, if kept { "" } else { "it wasn't done" });
    }

    /// What a decision says about goals: a promise made (or making something
    /// and handing it to someone, which is one, said or not), an aim taken
    /// on, or whether a goal that came due was met.
    pub fn decision_goals(&mut self, cid: i64, d: &crate::world::characters::Decision, steps: &[super::Action]) {
        use super::{Action, Target};
        if let Some(k) = d.kept {
            self.goal_judged(cid, k);
        }
        let me = ActorId::Npc(cid);
        let at = self.actor(me).map(|a| a.pos).unwrap_or_default();
        let pledge = |v: &Option<Value>| v.as_ref().and_then(|v| serde_json::from_value::<Pledge>(v.clone()).ok()).filter(|p| !p.text.trim().is_empty());
        // A favour asked of them is promised when they agree (see `needs`).
        let asked = self.cast.get(cid).is_some_and(|n| n.asked_by.is_some());
        if let Some(p) = pledge(&d.promise) {
            let to = p.to.as_deref().and_then(|n| match self.find_named(n, at, me) {
                Some(Target::Actor(a)) if a != me => Some(a),
                _ => None,
            });
            if let Some(to) = to {
                self.promise(cid, to, &p.text, p.what.as_deref(), p.within_hours);
            }
        } else if !asked {
            let give = steps.iter().enumerate().find_map(|(i, a)| match a {
                Action::Give { to } => Some((i, to.clone())),
                _ => None,
            });
            if let Some((i, to)) = give {
                let who = self.resolve(&to, me).map(|r| r.target);
                let made = steps[..i].iter().rev().find_map(|a| match a {
                    Action::Do { text, .. } => Some(text.clone()),
                    _ => None,
                });
                if let (Some(Target::Actor(who)), Some(text)) = (who, made) {
                    if who != me {
                        let what = text.split_once(' ').map(|(_, r)| r.to_string()).unwrap_or_else(|| text.clone());
                        let words = format!("{text} for {}", self.actor_name(who));
                        self.promise(cid, who, &words, Some(&what), None);
                    }
                }
            }
        }
        if let Some(a) = pledge(&d.aim) {
            let hours = a.within_hours.filter(|h| h.is_finite() && *h > 0.0).unwrap_or(24.0).clamp(1.0, 96.0) as f64;
            let deadline = Some(self.t + hours * super::headless::hour_s());
            let n_plan = self.goals.list.iter().filter(|g| g.owner == cid && g.open() && g.source == Source::Plan).count();
            if n_plan < 3 {
                self.add_goal(cid, Want::Words { text: a.text.clone() }, &a.text, "my own idea", 0.5, deadline, None, Source::Plan);
            }
        }
    }

    /// Their goals in a line, for their planner.
    pub fn goals_line(&self, cid: i64) -> Option<String> {
        let t = self.t;
        let hour = super::headless::hour_s();
        let parts: Vec<String> = self
            .goals_of(cid)
            .into_iter()
            .take(4)
            .map(|g| {
                let mut s = g.text.clone();
                let mut tail = Vec::new();
                if let Some(p) = g.promised_to {
                    tail.push(format!("promised to {}", self.actor_name(p)));
                }
                if let Some(d) = g.deadline {
                    let h = ((d - t) / hour).max(0.0);
                    tail.push(if h < 1.0 { "due now".to_string() } else { format!("due in about {h:.0} hours") });
                }
                if g.source == Source::Persona {
                    tail.push("your own aim".into());
                }
                if !tail.is_empty() {
                    s = format!("{s} ({})", tail.join(", "));
                }
                s
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join("; "))
    }

    /// Goals closed lately, for their planner ("you kept your promise to Ola").
    pub fn goals_done_line(&self, cid: i64) -> Option<String> {
        let t = self.t;
        let parts: Vec<String> = self.goals.list.iter().filter(|g| g.owner == cid && g.closed.is_some_and(|c| t - c < 300.0)).map(|g| format!("{} ({})", g.text, if g.status == Status::Done { "done" } else { "given up" })).collect();
        (!parts.is_empty()).then(|| parts.join("; "))
    }
}
