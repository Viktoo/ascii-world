//! Needs: when a character's deed can't be done as things are, the world
//! names one need (a place, a thing, someone, a time) and the character sets
//! out to meet it: use what is there, make it, ask people who could, wait,
//! or give up and say why. Nothing here knows about grills or burritos; the
//! interpreter brings the meaning, this only carries the mission.
//!
//! Limits keep it bounded: one need per mission (making the need can't need
//! more), asking is one hop (whoever is asked never asks on), and a mission
//! asks a few people at most.

use super::actions::Action;
use super::{ActorId, Note, Sim, Target};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// People a mission may ask, one after another.
const MAX_ASKED: usize = 3;
/// How long one person has to come through.
const ASK_SECS: f64 = 900.0;
/// How long a character may take to decide how to meet a need.
const CHOOSE_SECS: f64 = 180.0;

/// What a deed was missing.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Need {
    /// "place", "thing", "someone" or "time".
    #[serde(default)]
    pub kind: String,
    /// Its name from what is around, or what it is in a few words.
    #[serde(default)]
    pub what: String,
    /// For a time: the hour (0–23) it can be done at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hour: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stage {
    /// Walking to what it needs, or waiting for the hour.
    OnTheWay,
    /// Deciding (the planner) how to meet it.
    Choosing,
    /// Making the need themselves.
    Making,
    /// Waiting on someone who said they'd ask… or agreed to help.
    Asking(ActorId),
    /// Trying the deed again: what happens now is the end of it.
    Resumed,
}

/// A deed a character set out to do, and what it is waiting on.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mission {
    /// The deed, in words (empty when it is only an ask, with nothing to resume).
    pub text: String,
    /// Who the result was for (the `deliver` it had), handed on when it is done.
    pub deliver: Option<(ActorId, f64)>,
    pub need: Need,
    pub stage: Stage,
    /// Who to ask next, and who was asked already.
    pub candidates: Vec<ActorId>,
    pub asked: Vec<ActorId>,
    /// Helping someone else: asking on is not allowed (one hop).
    pub no_ask: bool,
    /// When the current stage gives out.
    pub until: f64,
}

/// What a character is in the middle of, kept between sessions.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Work {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plan: Vec<Action>,
    #[serde(default)]
    pub plan_from_llm: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission: Option<Mission>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver: Option<(ActorId, f64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asked_by: Option<(ActorId, f64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub helping: Option<(ActorId, f64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deed: Option<(String, Option<Target>, f64)>,
}

/// A deed still unfinished this long after it started is let go.
const DEED_SECS: f64 = 900.0;

impl super::npc::Npc {
    /// Their work under way, or nothing when there is none.
    pub fn work(&self) -> Option<serde_json::Value> {
        let w = Work {
            plan: self.plan.iter().cloned().collect(),
            plan_from_llm: self.plan_from_llm,
            mission: self.mission.clone(),
            deliver: self.deliver,
            asked_by: self.asked_by,
            helping: self.helping,
            deed: self.deed.clone(),
        };
        let empty = w.plan.is_empty() && w.mission.is_none() && w.deliver.is_none() && w.asked_by.is_none() && w.helping.is_none() && w.deed.is_none();
        if empty { None } else { serde_json::to_value(&w).ok() }
    }

    pub fn restore_work(&mut self, v: &serde_json::Value) {
        let Ok(w) = serde_json::from_value::<Work>(v.clone()) else { return };
        self.plan = w.plan.into();
        self.plan_from_llm = w.plan_from_llm;
        self.mission = w.mission;
        self.deliver = w.deliver;
        self.asked_by = w.asked_by;
        self.helping = w.helping;
        self.deed = w.deed;
        self.restored = true;
    }
}

impl Sim {
    /// A character's deed in words started (it may take a while to land).
    pub fn deed_started(&mut self, who: ActorId, text: &str, on: Option<Target>) {
        let t = self.t;
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get_mut(c) {
                n.deed = Some((text.to_string(), on, t));
            }
        }
    }

    /// It landed (or came to nothing): nothing to do again after a reload.
    pub fn deed_landed(&mut self, who: ActorId) {
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get_mut(c) {
                n.deed = None;
            }
        }
    }

    /// First tick after loading: whatever was waiting on an answer that the
    /// last session never got is asked again.
    pub fn after_load(&mut self, cid: i64) {
        let me = ActorId::Npc(cid);
        let t = self.t;
        let Some(n) = self.cast.get_mut(cid) else { return };
        n.restored = false;
        // The deed under way: do it again, first thing.
        if let Some((text, on, at)) = n.deed.take() {
            if t - at < DEED_SECS {
                n.plan.push_front(Action::Do { text, on, at: None });
            }
        }
        // A favour asked of them that they never answered: asked again by the asker.
        n.asked_by = None;
        let busy = !n.plan.is_empty();
        let Some(m) = n.mission.clone() else { return };
        match m.stage {
            Stage::Choosing => self.choose_how(me),
            Stage::Asking(other) => {
                let agreed = matches!(other, ActorId::Npc(oc) if self.cast.get(oc).is_some_and(|o| o.helping.is_some_and(|(a, _)| a == me)));
                let pleading = self.cast.get(cid).is_some_and(|n| n.plan.iter().any(|a| matches!(a, Action::Plea { .. })));
                if !agreed && !pleading {
                    if let Some(m) = self.mission_mut(me) {
                        m.until = t + ASK_SECS;
                    }
                    self.plan(me, vec![Action::Goto { target: Target::Actor(other), run: false }, Action::Plea { to: Target::Actor(other) }], &format!("ask for {}", m.need.what), true);
                }
            }
            Stage::OnTheWay if !busy => self.meet_need(me),
            Stage::Making if !busy => self.choose_how(me),
            Stage::Resumed if !busy && !m.text.is_empty() => {
                self.plan(me, vec![Action::Do { text: m.text.clone(), on: None, at: None }], "back to it", true);
            }
            _ => {}
        }
    }

    fn mission(&self, who: ActorId) -> Option<&Mission> {
        let ActorId::Npc(c) = who else { return None };
        self.cast.get(c).and_then(|n| n.mission.as_ref())
    }

    fn mission_mut(&mut self, who: ActorId) -> Option<&mut Mission> {
        let ActorId::Npc(c) = who else { return None };
        self.cast.get_mut(c).and_then(|n| n.mission.as_mut())
    }

    /// A character's deed came back as "not as things are". Set out to meet
    /// the need, or give up when it can't go further.
    pub fn on_need(&mut self, who: ActorId, text: &str, need: &Need, narration: &str) {
        let ActorId::Npc(c) = who else { return };
        if let Some(m) = self.mission(who) {
            // Making the need, or the deed itself once more, still came up short.
            let what = if m.text.is_empty() { need.what.clone() } else { m.text.clone() };
            self.give_up(who, &format!("couldn't {what}: {narration}"));
            return;
        }
        let t = self.t;
        let Some(n) = self.cast.get_mut(c) else { return };
        let deliver = n.deliver.take();
        let no_ask = n.helping.or(n.asked_by).is_some_and(|(_, until)| t <= until);
        n.mission = Some(Mission { text: text.to_string(), deliver, need: need.clone(), stage: Stage::OnTheWay, candidates: vec![], asked: vec![], no_ask, until: t + ASK_SECS });
        self.meet_need(who);
    }

    /// The direct ways first (it is right there, the hour will come, the one
    /// person who can); otherwise the character decides.
    fn meet_need(&mut self, who: ActorId) {
        let Some(m) = self.mission(who).cloned() else { return };
        let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
        let kind = m.need.kind.trim().to_lowercase();
        if kind == "time" {
            if let Some(h) = m.need.hour.filter(|h| h.is_finite()) {
                self.plan(who, vec![Action::WaitUntil { hour: h }, Action::Resume { on: None }], &format!("wait to {}", m.text), true);
                return;
            }
        }
        let found = if m.need.what.trim().is_empty() { None } else { self.find_named(&m.need.what, at, who) };
        match found {
            Some(Target::Actor(a)) if a != ActorId::Player && kind == "someone" && !m.no_ask => {
                if let Some(m) = self.mission_mut(who) {
                    m.candidates = vec![a];
                }
                self.next_ask(who);
            }
            Some(t) if kind != "someone" && !matches!(t, Target::Actor(_) | Target::Point(_)) => {
                let small = self.liven_peek_liftable(&t);
                let steps = if small {
                    vec![Action::Hold { target: t }, Action::Resume { on: None }]
                } else {
                    vec![Action::Goto { target: t.clone(), run: false }, Action::Resume { on: Some(t) }]
                };
                self.plan(who, steps, &format!("go get {}", m.need.what), true);
            }
            _ => self.choose_how(who),
        }
    }

    /// Whether a target is a loose thing someone could pick up.
    fn liven_peek_liftable(&self, t: &Target) -> bool {
        match t {
            Target::Thing(id) => self.things.get(*id).is_some_and(|x| x.liftable(1)),
            _ => false,
        }
    }

    /// Nothing at hand meets it: the character decides to make it, ask
    /// someone who could, or give up.
    fn choose_how(&mut self, who: ActorId) {
        let ActorId::Npc(c) = who else { return };
        let Some(m) = self.mission(who).cloned() else { return };
        let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
        let mut people: Vec<(f32, String)> = Vec::new();
        if !m.no_ask {
            for n in &self.cast.npcs {
                if n.def.id == c || !n.here() || self.mind_of(ActorId::Npc(n.def.id)) != crate::world::species::Mind::Sapient {
                    continue;
                }
                let d = (n.a.pos - at).length();
                let pe = &n.def.persona;
                let about: String = format!("{} {}", pe.personality, pe.goals).chars().take(140).collect();
                let rel = self.social.rel(who, ActorId::Npc(n.def.id)).map(|r| r.describe()).unwrap_or_else(|| "a stranger".into());
                people.push((d, format!("{} ({}, {d:.0} m, {rel}): {}", pe.name, n.species.name, about.trim())));
            }
            people.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        let people: Vec<String> = people.into_iter().take(10).map(|(_, s)| s).collect();
        let ask = if m.no_ask {
            "You are doing this as a favour, so you can't ask anyone else.".to_string()
        } else if people.is_empty() {
            "Nobody else is around to ask.".to_string()
        } else {
            format!("People you could ask:\n- {}", people.join("\n- "))
        };
        let what = format!(
            "You set out to {} but it needs {} ({}), and there is none at hand. Decide, as yourself, how to get it:\n- make it yourself, if someone like you could: a \"do\" step that makes it (e.g. \"make {}\")\n- ask people who could make it or have it, best first: one \"ask\" step, {{\"do\": \"ask\", \"who\": [\"Name\", …], \"for\": \"{}\"}} (up to {MAX_ASKED})\n- or give up: no steps, and say why\n{ask}",
            m.text, m.need.what, m.need.kind, m.need.what, m.need.what
        );
        let until = self.t + CHOOSE_SECS;
        if let Some(m) = self.mission_mut(who) {
            m.stage = Stage::Choosing;
            m.until = until;
        }
        if !self.ask_planner_now(c, "need", &what) {
            self.give_up(who, &format!("no way to get {}", m.need.what));
        }
    }

    /// A decision arrived while a mission waits on it, or for someone who
    /// was asked a favour. Returns the steps to carry out.
    pub fn need_decision(&mut self, cid: i64, mut steps: Vec<Action>, said: bool) -> Vec<Action> {
        let me = ActorId::Npc(cid);
        let t = self.t;
        // Asked a favour: yes is a plan that makes or gets it, no is none.
        let asked = self.cast.get_mut(cid).and_then(|n| n.asked_by.take()).filter(|(_, until)| t <= *until);
        if let Some((asker, _)) = asked {
            // One hop: whoever is asked never asks on.
            steps.retain(|a| !matches!(a, Action::Ask { .. }));
            if steps.is_empty() {
                self.refused(asker, me);
            } else {
                let makes = steps.iter().any(|a| matches!(a, Action::Do { .. }));
                if let Some(n) = self.cast.get_mut(cid) {
                    if makes && n.deliver.is_none() {
                        n.deliver = Some((asker, t + ASK_SECS));
                    }
                    n.helping = Some((asker, t + ASK_SECS));
                }
                // Handing over something already held is a give step to them.
                if !makes && !steps.iter().any(|a| matches!(a, Action::Give { .. })) {
                    steps.push(Action::Give { to: Target::Actor(asker) });
                }
                if let Some(m) = self.mission_mut(asker) {
                    m.until = t + ASK_SECS;
                }
                let (a, b) = (self.actor_name(me), self.actor_name(asker));
                let at = self.actor(me).map(|x| x.pos);
                self.event("agreed", Some(me), Some(asker.key()), format!("{a} agreed to help {b}"), at, json!({}));
            }
            return steps;
        }
        let Some(m) = self.mission(me).cloned() else { return steps };
        if m.stage != Stage::Choosing {
            return steps;
        }
        if m.no_ask {
            steps.retain(|a| !matches!(a, Action::Ask { .. }));
        }
        if steps.iter().any(|a| matches!(a, Action::Ask { .. })) {
            // The ask step runs as planned and picks who to go to.
        } else if steps.iter().any(|a| matches!(a, Action::Do { .. })) {
            if let Some(m) = self.mission_mut(me) {
                m.stage = Stage::Making;
                m.until = t + ASK_SECS;
            }
        } else {
            let why = if said { "they gave up".to_string() } else { format!("no way to get {}", m.need.what) };
            self.give_up_quietly(me, &why);
        }
        steps
    }

    /// The `ask` step: go to the next person and ask them.
    pub fn ask_step(&mut self, who: ActorId, names: &[Target], what: &str) -> Result<super::actions::Outcome, super::actions::ActErr> {
        let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
        if self.mission(who).is_none() {
            // Asking on its own: a mission with nothing to resume.
            let ActorId::Npc(c) = who else { return Err(super::actions::ActErr::Fail("the traveler asks in words".into())) };
            let t = self.t;
            if let Some(n) = self.cast.get_mut(c) {
                n.mission = Some(Mission { text: String::new(), deliver: None, need: Need { kind: "thing".into(), what: what.to_string(), hour: None }, stage: Stage::OnTheWay, candidates: vec![], asked: vec![], no_ask: false, until: t + ASK_SECS });
            }
        }
        let asked = self.mission(who).map(|m| m.asked.clone()).unwrap_or_default();
        if self.mission(who).is_some_and(|m| m.candidates.is_empty()) {
            let mut list = Vec::new();
            for t in names {
                if let Some(Target::Actor(a @ ActorId::Npc(_))) = self.resolve(t, who).map(|r| r.target).or_else(|| match t {
                    Target::Name(n) => self.find_named(n, at, who),
                    _ => None,
                }) {
                    if a != who && !asked.contains(&a) && !list.contains(&a) {
                        list.push(a);
                    }
                }
            }
            list.truncate(MAX_ASKED.saturating_sub(asked.len()));
            if let Some(m) = self.mission_mut(who) {
                m.candidates = list;
                if m.need.what.is_empty() {
                    m.need.what = what.to_string();
                }
            }
        }
        self.next_ask(who);
        Ok(super::actions::Outcome::ok(format!("{} goes to ask for {what}", self.actor_name(who))))
    }

    /// Ask the next person on the list (walking up to them first), or give up.
    fn next_ask(&mut self, who: ActorId) {
        let Some(m) = self.mission(who).cloned() else { return };
        let Some(&other) = m.candidates.first() else {
            let what = m.need.what.clone();
            self.give_up(who, &format!("nobody could help with {what}"));
            return;
        };
        let until = self.t + ASK_SECS;
        if let Some(m) = self.mission_mut(who) {
            m.candidates.remove(0);
            m.asked.push(other);
            m.stage = Stage::Asking(other);
            m.until = until;
        }
        self.plan(who, vec![Action::Goto { target: Target::Actor(other), run: false }, Action::Plea { to: Target::Actor(other) }], &format!("ask {} for {}", self.actor_name(other), m.need.what), true);
    }

    /// Standing by them: ask, in words, and let them decide.
    pub fn plea_step(&mut self, who: ActorId, to: &Target) -> Result<super::actions::Outcome, super::actions::ActErr> {
        let Some(m) = self.mission(who).cloned() else { return Err(super::actions::ActErr::Fail("nothing to ask for".into())) };
        let Some(Target::Actor(other)) = self.resolve(to, who).map(|r| r.target) else { return Err(super::actions::ActErr::Fail("ask whom?".into())) };
        let ActorId::Npc(oc) = other else { return Err(super::actions::ActErr::Fail("ask whom?".into())) };
        let (me, them) = (self.actor_name(who), self.actor_name(other));
        let what = &m.need.what;
        let why = if m.text.is_empty() { String::new() } else { format!(" so they can {}", m.text) };
        self.say(who, &format!("{them}, could you help me with {what}?"), Some(Target::Actor(other)));
        let t = self.t;
        if let Some(n) = self.cast.get_mut(oc) {
            n.asked_by = Some((who, t + CHOOSE_SECS));
        }
        // An answer comes soon or not at all; a yes gives them longer.
        if let Some(m) = self.mission_mut(who) {
            m.until = t + CHOOSE_SECS;
        }
        let text = format!(
            "{me} asks you for {what}{why}. Decide as yourself, by your own needs, what you are doing and how you feel about {me}: you have your own life, so say no when it doesn't suit you. If yes: steps that make it (a \"do\" step) or get it, then a \"give\" step to {me}; if it is big (a structure), make it near {me}. If no: no steps. Set \"say\" to what you answer {me}."
        );
        if !self.ask_planner_now(oc, "asked_by", &text) {
            // They can't think it over now (far from anyone watching): no.
            if let Some(n) = self.cast.get_mut(oc) {
                n.asked_by = None;
            }
            self.refused(who, other);
        }
        Ok(super::actions::Outcome::ok(format!("{me} asks {them} for {what}")))
    }

    /// Someone said no: on to the next one.
    fn refused(&mut self, asker: ActorId, by: ActorId) {
        let Some(m) = self.mission(asker).cloned() else { return };
        if m.stage != Stage::Asking(by) {
            return;
        }
        let (a, b) = (self.actor_name(asker), self.actor_name(by));
        let at = self.actor(by).map(|x| x.pos).unwrap_or_default();
        self.event("refused", Some(by), Some(asker.key()), format!("{b} wouldn't help {a} with {}", m.need.what), Some(at), json!({}));
        self.witness(at, 15.0, &format!("{b} wouldn't help {a} with {}.", m.need.what), 0.3, &[]);
        self.next_ask(asker);
    }

    /// The `resume` step: the need is met; try the deed again.
    pub fn resume_step(&mut self, who: ActorId, on: Option<Target>) -> Result<super::actions::Outcome, super::actions::ActErr> {
        let ActorId::Npc(c) = who else { return Err(super::actions::ActErr::Fail("nothing to go back to".into())) };
        let Some(n) = self.cast.get_mut(c) else { return Err(super::actions::ActErr::Fail("nothing to go back to".into())) };
        let Some(m) = n.mission.as_mut() else { return Err(super::actions::ActErr::Fail("nothing to go back to".into())) };
        if m.text.is_empty() {
            // Only an ask: having it is the end of it.
            n.mission = None;
            return Ok(super::actions::Outcome::ok("done"));
        }
        m.stage = Stage::Resumed;
        let text = m.text.clone();
        n.deliver = m.deliver.take();
        self.act(who, Action::Do { text, on, at: None })
    }

    /// What a mission waits on was made: by the character, or by whoever
    /// agreed to help. A loose thing made for someone is handed over first.
    pub fn need_made(&mut self, maker: ActorId, made: Target, loose: bool) {
        if self.mission(maker).is_some_and(|m| m.stage == Stage::Making) {
            let on = if loose { None } else { Some(made.clone()) };
            let steps = match &on {
                Some(t) => vec![Action::Goto { target: t.clone(), run: false }, Action::Resume { on }],
                None => vec![Action::Resume { on: None }],
            };
            self.plan(maker, steps, "back to it", true);
            return;
        }
        if loose {
            return;
        }
        let waiting: Vec<ActorId> = self.cast.npcs.iter().filter(|n| n.mission.as_ref().is_some_and(|m| m.stage == Stage::Asking(maker))).map(|n| ActorId::Npc(n.def.id)).collect();
        for w in waiting {
            self.plan(w, vec![Action::Goto { target: made.clone(), run: false }, Action::Resume { on: Some(made.clone()) }], "back to it", true);
        }
        self.done_helping(maker);
    }

    /// Something was handed over: the help someone was waiting for.
    pub fn need_given(&mut self, from: ActorId, to: ActorId) {
        if self.mission(to).is_some_and(|m| m.stage == Stage::Asking(from)) {
            self.plan(to, vec![Action::Resume { on: None }], "back to it", true);
            self.done_helping(from);
        }
    }

    fn done_helping(&mut self, helper: ActorId) {
        if let ActorId::Npc(c) = helper {
            if let Some(n) = self.cast.get_mut(c) {
                n.helping = None;
            }
        }
    }

    /// The deed was tried again (or an ask came to its end): the mission is over.
    pub fn deed_done(&mut self, who: ActorId, text: &str) {
        let ActorId::Npc(c) = who else { return };
        if let Some(n) = self.cast.get_mut(c) {
            if n.mission.as_ref().is_some_and(|m| m.stage == Stage::Resumed && m.text == text) {
                n.mission = None;
            }
        }
    }

    /// The deed changed nothing and named no need: if it was the mission's
    /// last try, or the making of its need, that is the end of it.
    pub fn deed_fell_through(&mut self, who: ActorId, text: &str, narration: &str) {
        let Some(m) = self.mission(who) else { return };
        if (m.stage == Stage::Resumed && m.text == text) || m.stage == Stage::Making {
            let why = narration.trim().trim_end_matches('.').to_string();
            self.give_up(who, if why.is_empty() { "it didn't work" } else { &why });
        }
    }

    /// Give up the mission and say so, where people can hear.
    pub fn give_up(&mut self, who: ActorId, why: &str) {
        let Some(m) = self.mission(who).cloned() else { return };
        self.give_up_quietly(who, why);
        let name = self.actor_name(who);
        let goal = if m.text.is_empty() { format!("get {}", m.need.what) } else { m.text.clone() };
        if let Some(at) = self.actor(who).map(|a| a.pos) {
            self.note_near(at, 20.0, Note::Ambient(format!("{} gives up trying to {goal}: {why}.", super::physics::cap(&name))));
        }
    }

    fn give_up_quietly(&mut self, who: ActorId, why: &str) {
        let ActorId::Npc(c) = who else { return };
        let Some(n) = self.cast.get_mut(c) else { return };
        let helping = n.helping.take();
        let Some(m) = n.mission.take() else { return };
        let name = self.actor_name(who);
        let goal = if m.text.is_empty() { format!("get {}", m.need.what) } else { m.text.clone() };
        let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
        self.event("gave_up", Some(who), None, format!("{name} gave up trying to {goal}: {why}"), Some(at), json!({ "need": m.need }));
        self.witness(at, 15.0, &format!("{name} gave up trying to {goal}: {why}."), 0.4, &[]);
        // Whoever they were helping hears it couldn't be done.
        if let Some((asker, _)) = helping {
            self.refused(asker, who);
        }
    }

    /// Stages run out: an ask nobody answered is a no; anything else, give up.
    pub fn need_tick(&mut self, cid: i64) {
        let me = ActorId::Npc(cid);
        let Some(m) = self.mission(me).cloned() else { return };
        if self.t <= m.until {
            return;
        }
        match m.stage {
            Stage::Asking(other) => {
                if let ActorId::Npc(oc) = other {
                    if let Some(n) = self.cast.get_mut(oc) {
                        n.asked_by = None;
                    }
                }
                self.refused(me, other);
            }
            _ => self.give_up(me, "it took too long"),
        }
    }
}
