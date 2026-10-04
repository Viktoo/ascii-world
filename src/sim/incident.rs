//! Incidents: many small happenings with one cause, told as one story.
//!
//! When a property spreads (heat to the next tuft, a curse by touch), the one
//! it spread to remembers where it came from. Threshold crossings with a
//! shared cause gather into one incident: a fire that ate 300 tufts started
//! at one kiln. The log keeps one line per incident and updates it; the event
//! log keeps the first crossing and the end; people remember what started it
//! and treat it as a problem to deal with together.
//!
//! Nothing here knows fire: a property spreads as incidents when its
//! vocabulary entry says so (`PropMeta::incident`), with the words to tell it.

use super::props::Words;
use super::{Note, Sim};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Quiet this long (no crossings, nothing still going), an incident is over.
const QUIET: f64 = 20.0;
/// Quiet this long, it's over even if parts of it froze out of reach.
const STALE: f64 = 120.0;
/// Catching this close to a going incident (metres) is part of it.
const NEAR: f32 = 6.0;
/// The log line is redrawn at most this often.
const TELL_EVERY: f64 = 2.0;
/// Ended incidents are kept this long (seconds of game time), for minds and the log.
const KEEP_ENDED: f64 = 2.0 * crate::render::sky::DAY_SECONDS;

/// A threshold crossing, as the rules pass saw it.
#[derive(Clone, Debug)]
pub struct Crossed {
    /// The event kind ("ignited").
    pub kind: String,
    /// The property that crossed.
    pub prop: usize,
    /// Up past its level (true), or back down.
    pub rose: bool,
    /// Stopped before running its course.
    pub saved: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    /// A part catches: it joins its cause's incident, or starts one.
    Start,
    /// A part stops (ran its course, or was stopped).
    End,
    /// Only counted, if the part already belongs to one.
    Join,
}

fn fire_words() -> Words {
    Words { noun: "fire".into(), big: "wildfire".into(), active: "burning".into(), spent: "burnt".into(), ended: "burnt itself out".into(), stopped: "put out".into() }
}

fn fire() -> String {
    "fire".into()
}

/// What started an incident: the thing it spread from, and who, if anyone.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Cause {
    /// "thing:3", "instance:12", "cell:4,5" (empty: it started by itself).
    pub subject: String,
    /// "the root kiln".
    pub name: String,
    /// Someone behind it ("the traveler", "Pell"): held it, threw it, did a deed to it.
    #[serde(default)]
    pub by: Option<String>,
}

impl Cause {
    /// "the root kiln", "a lantern the traveler threw".
    pub fn told(&self) -> String {
        match &self.by {
            Some(b) if !self.name.is_empty() => format!("{} ({b})", self.name),
            Some(b) => b.clone(),
            None if self.name.is_empty() => "nobody knows what".into(),
            None => self.name.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Incident {
    pub id: u32,
    /// What it is called ("fire").
    pub kind: String,
    /// The property that spreads ("fire", "cursed").
    #[serde(default = "fire")]
    pub prop: String,
    /// How it is told.
    #[serde(default = "fire_words")]
    pub words: Words,
    /// Parts it ran its course on.
    #[serde(default)]
    pub spent: u32,
    pub cause: Cause,
    /// The place's name where it began, if it has one.
    #[serde(default)]
    pub place: String,
    pub at: [f32; 3],
    pub started: f64,
    pub last: f64,
    /// Crossings by kind ("ignited": 340, "burnt_out": 300, "doused": 4).
    pub counts: BTreeMap<String, u32>,
    /// Parts still going (burning).
    pub live: BTreeSet<String>,
    /// How far it has reached (x, z box).
    pub lo: [f32; 2],
    pub hi: [f32; 2],
    #[serde(default)]
    pub ended: Option<f64>,
    /// Who fought it.
    #[serde(default)]
    pub fought_by: BTreeSet<String>,
    /// Parts put out (and not burnt since): what was saved.
    #[serde(default)]
    pub saved: BTreeSet<String>,
    #[serde(skip)]
    told_at: f64,
    /// Who was stirred to think about it already.
    #[serde(skip)]
    roused: BTreeSet<i64>,
}

impl Incident {
    /// Crossings of one kind ("ignited").
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn count(&self, kind: &str) -> u32 {
        self.counts.get(kind).copied().unwrap_or(0)
    }

    pub fn going(&self) -> bool {
        self.ended.is_none()
    }

    pub fn centre(&self) -> Vec3 {
        Vec3::new((self.lo[0] + self.hi[0]) * 0.5, self.at[1], (self.lo[1] + self.hi[1]) * 0.5)
    }

    /// Across, in metres.
    pub fn extent(&self) -> f32 {
        (self.hi[0] - self.lo[0]).max(self.hi[1] - self.lo[1])
    }

    /// Grown big ("wildfire" rather than "fire").
    fn big(&self) -> bool {
        self.count_rises() >= 12 || self.extent() > 25.0
    }

    fn count_rises(&self) -> u32 {
        self.spent + self.saved.len() as u32 + self.live.len() as u32
    }

    /// The incident in one line: what, from what, how big, how it stands.
    pub fn line(&self) -> String {
        let w = &self.words;
        let what = super::physics::cap(&if self.big() { w.big() } else { w.noun.clone() });
        let from = if self.cause.subject.is_empty() && self.cause.by.is_none() { String::new() } else { format!(" from {}", self.cause.told()) };
        let place = if self.place.is_empty() { String::new() } else { format!(" near {}", self.place) };
        let mut parts = Vec::new();
        if self.spent > 0 {
            parts.push(format!("{} {}", self.spent, w.spent()));
        }
        if !self.saved.is_empty() {
            parts.push(format!("{} saved", self.saved.len()));
        }
        if self.extent() >= 10.0 {
            parts.push(format!("{} m across", (self.extent() / 5.0).round() as i32 * 5));
        }
        let state = match self.ended {
            Some(_) => self.end_state(),
            None if self.live.is_empty() => "dying down".into(),
            None => format!("{} {}", self.live.len(), w.active()),
        };
        parts.push(state);
        format!("{what}{from}{place}: {}.", parts.join(", "))
    }

    /// How it ended: "put out", "burnt itself out", "burnt itself out though 3 fought it".
    fn end_state(&self) -> String {
        let w = &self.words;
        match self.fought_by.len() {
            0 => w.ended(),
            _ if self.saved.len() * 2 >= self.spent as usize => w.stopped(),
            n => format!("{} though {n} fought it", w.ended()),
        }
    }

    /// As news, for the event log and people's talk: "a fire broke out near
    /// Brask, started by the root kiln"; "the fire from the root kiln: put out".
    fn story(&self) -> String {
        let what = &self.words.noun;
        let from = if self.cause.subject.is_empty() && self.cause.by.is_none() { String::new() } else { format!(" from {}", self.cause.told()) };
        match self.ended {
            None => format!("{} {what} broke out{}{}", super::article(what), self.place_suffix(), if from.is_empty() { String::new() } else { format!(", started by {}", self.cause.told()) }),
            Some(_) => format!("the {what}{from} is over: {}", self.end_state()),
        }
    }

    /// How a witness remembers it.
    fn memory(&self) -> String {
        let what = format!("{} {}", super::article(&self.words.noun), self.words.noun);
        match (&self.cause.subject.is_empty(), &self.cause.by) {
            (true, None) => format!("I saw {what} break out{}; nobody knows what started it.", self.place_suffix()),
            _ => format!("I saw {what} break out{}. It started from {}.", self.place_suffix(), self.cause.told()),
        }
    }

    fn place_suffix(&self) -> String {
        if self.place.is_empty() { String::new() } else { format!(" near {}", self.place) }
    }
}

/// All incidents, and which part belongs to which.
#[derive(Default, Serialize, Deserialize)]
pub struct Incidents {
    pub list: Vec<Incident>,
    /// Part ("cell:4,5") → incident, while the incident is going.
    #[serde(default)]
    pub of: HashMap<String, u32>,
    #[serde(default)]
    pub next: u32,
}

impl Incidents {
    pub fn get(&self, id: u32) -> Option<&Incident> {
        self.list.iter().find(|i| i.id == id)
    }

    fn get_mut(&mut self, id: u32) -> Option<&mut Incident> {
        self.list.iter_mut().find(|i| i.id == id)
    }

    /// The going incident a part belongs to.
    pub fn part_of(&self, subject: &str) -> Option<u32> {
        self.of.get(subject).copied().filter(|id| self.get(*id).is_some_and(|i| i.going()))
    }

    /// Going incidents.
    pub fn going(&self) -> impl Iterator<Item = &Incident> {
        self.list.iter().filter(|i| i.going())
    }
}

/// A crossing's place in an incident.
pub enum Joined {
    /// It began one.
    Began(u32),
    /// It is one more part of one.
    Part(u32),
}

impl Sim {
    /// File a crossing (on `subject`, which spread from `cause`): into its
    /// incident, a new one, or none.
    pub fn file_incident(&mut self, subject: &str, c: &Crossed, pos: Vec3, cause: Option<Cause>) -> Option<Joined> {
        let vocab = self.vocab.clone();
        let words = vocab.meta.get(c.prop).and_then(|m| m.incident.clone());
        let role = match (&words, c.rose) {
            (Some(_), true) => Role::Start,
            (Some(_), false) => Role::End,
            (None, _) => Role::Join,
        };
        let prop = vocab.names.get(c.prop).cloned().unwrap_or_default();
        let fam = words.as_ref().map(|w| w.noun.clone()).unwrap_or_default();
        let fam = fam.as_str();
        let kind = c.kind.as_str();
        let t = self.t;
        if role == Role::Join && self.incidents.part_of(subject).is_none() {
            return None;
        }
        let mine = self.incidents.part_of(subject);
        let from = cause.as_ref().and_then(|c| self.incidents.part_of(&c.subject));
        // Catching right by a going one is the same one (pieces of one smashed lamp).
        let by = || self.incidents.going().find(|i| i.kind == fam && pos.x > i.lo[0] - NEAR && pos.x < i.hi[0] + NEAR && pos.z > i.lo[1] - NEAR && pos.z < i.hi[1] + NEAR).map(|i| i.id);
        let (id, began) = match (role, mine.or(from).or_else(|| if role == Role::Start { by() } else { None })) {
            (_, Some(id)) => (id, false),
            (Role::Start, None) => {
                let id = self.incidents.next.max(1);
                self.incidents.next = id + 1;
                let cause = cause.clone().unwrap_or_else(|| self.blame(subject));
                let place = self.snap.region_name(crate::world::region_of(pos.x, pos.z)).unwrap_or_default().to_string();
                self.incidents.list.push(Incident {
                    id,
                    kind: fam.into(),
                    prop: prop.clone(),
                    words: words.clone().unwrap_or_else(fire_words),
                    spent: 0,
                    cause,
                    place,
                    at: pos.to_array(),
                    started: t,
                    last: t,
                    counts: BTreeMap::new(),
                    live: BTreeSet::new(),
                    lo: [pos.x, pos.z],
                    hi: [pos.x, pos.z],
                    ended: None,
                    fought_by: BTreeSet::new(),
                    saved: BTreeSet::new(),
                    told_at: f64::MIN,
                    roused: BTreeSet::new(),
                });
                (id, true)
            }
            _ => return None,
        };
        // What it spread from is part of it too (a fire already burning when
        // the world was loaded, the lantern that lit it).
        if began {
            if let Some(c) = cause.as_ref().filter(|c| !c.subject.is_empty()) {
                self.incidents.of.entry(c.subject.clone()).or_insert(id);
            }
        }
        // Two fires that grow into each other are one, told as the older.
        let first = id;
        let id = if role == Role::Start { self.merge_near(id, fam, pos) } else { id };
        let began = began && id == first;
        let inc = self.incidents.get_mut(id)?;
        inc.last = t;
        *inc.counts.entry(kind.to_string()).or_default() += 1;
        inc.lo = [inc.lo[0].min(pos.x), inc.lo[1].min(pos.z)];
        inc.hi = [inc.hi[0].max(pos.x), inc.hi[1].max(pos.z)];
        match role {
            Role::Start => {
                inc.live.insert(subject.to_string());
            }
            Role::End => {
                inc.live.remove(subject);
                if c.saved {
                    inc.saved.insert(subject.to_string());
                } else {
                    inc.saved.remove(subject);
                    inc.spent += 1;
                }
            }
            Role::Join => {}
        }
        if role == Role::Start {
            self.incidents.of.insert(subject.to_string(), id);
        }
        Some(if began { Joined::Began(id) } else { Joined::Part(id) })
    }

    /// Fold going incidents of a family that reach `pos` into the oldest of
    /// them (with `id`). Returns the one that is left.
    fn merge_near(&mut self, id: u32, fam: &str, pos: Vec3) -> u32 {
        let near = |i: &Incident| i.going() && i.kind == fam && pos.x > i.lo[0] - NEAR && pos.x < i.hi[0] + NEAR && pos.z > i.lo[1] - NEAR && pos.z < i.hi[1] + NEAR;
        let mut ids: Vec<(f64, u32)> = self.incidents.list.iter().filter(|i| i.id == id || near(i)).map(|i| (i.started, i.id)).collect();
        if ids.len() < 2 {
            return id;
        }
        ids.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let keep = ids[0].1;
        for (_, gone) in ids.into_iter().skip(1) {
            let Some(ix) = self.incidents.list.iter().position(|i| i.id == gone) else { continue };
            let g = self.incidents.list.remove(ix);
            for v in self.incidents.of.values_mut().filter(|v| **v == gone) {
                *v = keep;
            }
            if let Some(k) = self.incidents.get_mut(keep) {
                for (kind, n) in g.counts {
                    *k.counts.entry(kind).or_default() += n;
                }
                k.live.extend(g.live);
                k.spent += g.spent;
                k.saved.extend(g.saved);
                k.fought_by.extend(g.fought_by);
                k.roused.extend(g.roused);
                k.lo = [k.lo[0].min(g.lo[0]), k.lo[1].min(g.lo[1])];
                k.hi = [k.hi[0].max(g.hi[0]), k.hi[1].max(g.hi[1])];
                k.last = k.last.max(g.last);
            }
            // Its line goes; the one it joined tells it all.
            self.notes.push(Note::Incident { id: gone, text: String::new(), at: g.at, near: false });
        }
        self.tell_incident(keep, true);
        keep
    }

    /// Who is behind a part that caught by itself: whoever holds it, threw
    /// it, or did something to it just now.
    fn blame(&self, subject: &str) -> Cause {
        self.cause_named(subject, self.subject_name(subject))
    }

    /// A cause: the part (by name) and whoever is behind it.
    pub fn cause_named(&self, subject: &str, name: String) -> Cause {
        let t = self.t;
        let mut by = None;
        if let Some(id) = subject.strip_prefix("thing:").and_then(|s| s.parse::<i64>().ok()) {
            if let Some(th) = self.things.get(id) {
                by = th.holder.map(|h| self.actor_name(h)).or_else(|| th.thrown_by.filter(|(_, at)| t - at < 60.0).map(|(a, _)| self.actor_name(a)));
            }
        }
        if by.is_none() {
            by = self.log.recent.iter().rev().take(200).filter(|e| t - e.t < 20.0).find(|e| e.actor.is_some() && e.subject.as_deref() == Some(subject)).and_then(|e| e.actor).map(|a| self.actor_name(a));
        }
        Cause { subject: subject.to_string(), name, by }
    }

    /// A live part by its key ("thing:3", "cell:4,5"), and where it is.
    pub fn part_at(&self, key: &str) -> Option<(super::Target, Vec3)> {
        if let Some(id) = key.strip_prefix("thing:").and_then(|s| s.parse::<i64>().ok()) {
            return self.things.get(id).filter(|t| !t.removed).map(|t| (super::Target::Thing(id), t.pos));
        }
        let (x, z) = key.strip_prefix("cell:")?.split_once(',')?;
        let c = (x.parse().ok()?, z.parse().ok()?);
        self.field.cells.get(&c).map(|cell| (super::Target::Cell([c.0, c.1]), cell.pos))
    }

    /// "the grass tuft", "the lantern" (for causes).
    pub fn subject_name(&self, subject: &str) -> String {
        let name = if let Some(id) = subject.strip_prefix("thing:").and_then(|s| s.parse::<i64>().ok()) {
            self.things.get(id).map(|t| self.thing_name(t.id))
        } else if let Some(i) = subject.strip_prefix("instance:").and_then(|s| s.parse::<i64>().ok()) {
            self.snap.instances.iter().find(|p| p.id == i).and_then(|p| self.snap.type_of(p.type_id)).map(|t| t.name().to_string())
        } else if let Some((x, z)) = subject.strip_prefix("cell:").and_then(|s| s.split_once(',')) {
            let c = (x.parse().unwrap_or(0), z.parse().unwrap_or(0));
            self.field.cells.get(&c).and_then(|c| self.snap.type_of(c.type_id)).map(|t| t.name().to_string())
        } else if let Some(a) = subject.starts_with("npc:").then(|| super::ActorId::parse(subject)).flatten() {
            // A being goes by its own name.
            return self.actor_name(a);
        } else {
            None
        };
        name.map(|n| format!("the {n}")).unwrap_or_default()
    }

    /// Tell the log about an incident: a new line when it begins, the same
    /// line updated after (at most every few seconds, and when it ends).
    fn tell_incident(&mut self, id: u32, force: bool) {
        let t = self.t;
        let Some(inc) = self.incidents.get(id) else { return };
        if !force && t - inc.told_at < TELL_EVERY {
            return;
        }
        let at = inc.centre();
        let note = Note::Incident { id, text: inc.line(), at: at.to_array(), near: self.dist_to_player(at) <= 60.0 + inc.extent() * 0.5 };
        if let Some(inc) = self.incidents.get_mut(id) {
            inc.told_at = t;
        }
        self.notes.push(note);
    }

    /// A crossing that belongs to an incident: count it, tell it as one
    /// story, remember it once.
    /// A crossing joined an incident (`text`: what happened to the part;
    /// `from`: what it spread from).
    pub fn incident_crossing(&mut self, joined: Joined, c: &Crossed, pos: Vec3, text: &str, from: Option<Cause>) {
        use super::beliefs::{Claim, Source};
        match joined {
            Joined::Began(id) => {
                let Some(inc) = self.incidents.get(id).cloned() else { return };
                self.rouse(id, pos, 60.0);
                self.event("incident", None, Some(inc.cause.subject.clone()).filter(|s| !s.is_empty()), inc.story(), Some(pos), json!({ "incident": id, "kind": inc.kind, "cause": inc.cause }));
                self.tell_incident(id, true);
                // Who saw it break out saw what started it.
                let claim = Claim::Cause { incident: id, noun: inc.words.noun.clone(), what: if inc.cause.name.is_empty() { "nobody knows what".into() } else { inc.cause.name.clone() }, by: inc.cause.by.clone() };
                self.witness_claim(pos, 60.0, &inc.memory(), 0.8, &[], &claim, Source::Saw, 0.9);
            }
            Joined::Part(id) => {
                // Who sees it reach them sees where it came at them, and
                // guesses that is where it came from.
                if c.rose {
                    let near_new = self.cast.npcs.iter().any(|n| n.here() && (n.a.pos - pos).length() < 30.0);
                    if near_new {
                        self.rouse(id, pos, 30.0);
                        if let Some(inc) = self.incidents.get(id).cloned() {
                            let noun = inc.words.noun.clone();
                            let src = from.filter(|f| !f.name.is_empty() || f.by.is_some()).unwrap_or_else(|| inc.cause.clone());
                            let memory = format!("I saw {}; the {noun} came at it from {}.", text.trim_end_matches('.'), src.told());
                            let claim = Claim::Cause { incident: id, noun, what: if src.name.is_empty() { "nobody knows what".into() } else { src.name.clone() }, by: src.by.clone() };
                            self.witness_claim(pos, 45.0, &memory, 0.7, &[], &claim, Source::Guessed, 0.5);
                        }
                    }
                }
                self.tell_incident(id, false);
            }
        }
    }

    /// Those awake near a new trouble (or near where it reached) think
    /// again now, instead of finishing what they were about.
    /// Once per incident each.
    fn rouse(&mut self, id: u32, pos: Vec3, r: f32) {
        let t = self.t;
        let Some(inc) = self.incidents.list.iter_mut().find(|i| i.id == id) else { return };
        for n in self.cast.npcs.iter_mut().filter(|n| n.here() && !n.a.asleep && (n.a.pos - pos).length() < r) {
            if inc.roused.insert(n.def.id) {
                n.think_at = n.think_at.min(t);
                n.plan.clear();
                n.a.task = None;
            }
        }
    }

    /// End what has gone quiet; forget what ended long ago.
    pub fn step_incidents(&mut self) {
        let t = self.t;
        let done: Vec<u32> = self.incidents.going().filter(|i| (i.live.is_empty() && t - i.last > QUIET) || t - i.last > STALE).map(|i| i.id).collect();
        for id in done {
            let Some(inc) = self.incidents.get_mut(id) else { continue };
            inc.ended = Some(t);
            inc.live.clear();
            let inc = inc.clone();
            self.incidents.of.retain(|_, v| *v != id);
            self.event("incident_end", None, Some(inc.cause.subject.clone()).filter(|s| !s.is_empty()), inc.story(), Some(inc.centre()), json!({ "incident": id, "kind": inc.kind, "counts": inc.counts, "fought_by": inc.fought_by }));
            self.tell_incident(id, true);
            let m = format!("The {} from {} is over: {}.", inc.words.noun, inc.cause.told(), inc.end_state());
            self.witness(inc.centre(), 60.0 + inc.extent() * 0.5, &m, 0.5, &[]);
        }
        self.incidents.list.retain(|i| i.ended.is_none_or(|e| t - e < KEEP_ENDED));
    }

    /// What is going on (or went on lately) around a point, for minds:
    /// "Trouble: Wildfire from the root kiln near Brask: 40 burnt, 6 burning (120 m north)."
    pub fn trouble_line(&self, p: Vec3) -> Option<String> {
        let t = self.t;
        let mut v = Vec::new();
        for i in self.incidents.list.iter().rev() {
            let c = i.centre();
            let d = Vec3::new(c.x - p.x, 0.0, c.z - p.z);
            let far = (d.length() - i.extent() * 0.5).max(0.0);
            let recent = i.ended.is_none_or(|e| t - e < crate::render::sky::DAY_SECONDS);
            if far > 300.0 || !recent {
                continue;
            }
            let way = if far < 15.0 { "right here".to_string() } else { format!("{} m {}", (far / 10.0).round() as i32 * 10, super::compass(d)) };
            let fought = if i.fought_by.is_empty() { String::new() } else { format!(" Fought by {}.", i.fought_by.iter().take(4).cloned().collect::<Vec<_>>().join(", ")) };
            v.push(format!("{} ({way}).{fought}", i.line().trim_end_matches('.')));
            if v.len() >= 3 {
                break;
            }
        }
        (!v.is_empty()).then(|| format!("Trouble nearby: {}", v.join(" ")))
    }

    /// Live parts of going incidents within `r` of a point, nearest first.
    pub fn trouble_parts(&self, p: Vec3, r: f32) -> Vec<(super::Target, Vec3)> {
        let mut v: Vec<(super::Target, Vec3)> = self.incidents.going().flat_map(|i| i.live.iter()).filter_map(|k| self.part_at(k)).filter(|(_, at)| (*at - p).length() <= r).collect();
        v.dedup_by(|a, b| a.0 == b.0);
        v.sort_by(|a, b| (a.1 - p).length().total_cmp(&(b.1 - p).length()));
        v
    }

    /// Going incidents within `r` of a point (nearest first), for minds.
    pub fn incidents_near(&self, p: Vec3, r: f32) -> Vec<&Incident> {
        let mut v: Vec<(&Incident, f32)> = self
            .incidents
            .going()
            .map(|i| {
                let c = i.centre();
                let d = (Vec3::new(c.x, 0.0, c.z) - Vec3::new(p.x, 0.0, p.z)).length() - i.extent() * 0.5;
                (i, d.max(0.0))
            })
            .filter(|(_, d)| *d <= r)
            .collect();
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        v.into_iter().map(|(i, _)| i).collect()
    }
}
