//! Beliefs: what each character holds true, as rows that can be asked. A
//! belief is a claim (what started a fire, where a thing lies, who made
//! something), where it came from (they saw it, someone told them, they
//! guessed), how sure they are and when. Memory text stays the story;
//! beliefs are what the planner, talk and the scorer look up.
//!
//! Seeing makes beliefs, and seeing different parts of something makes
//! different ones: whoever saw a fire break out knows what started it;
//! whoever only saw it reach them guesses it came from where it came at
//! them. Gossip passes beliefs on, marked as told and a little less sure
//! each hop. Honest people pass on what they believe, which can be stale or
//! a guess: rumours emerge without lies. Only those who mean harm lie (the
//! night's phantoms): such a teller twists a claim (blames the wrong one),
//! marked in the row; those who saw it themselves don't take it up.
//!
//! Each mind keeps at most `beliefs_per_mind`: the least important, least
//! sure and oldest fade first, and a repeat raises sureness instead of
//! adding a row, so a world's file stays roughly flat however long it runs.

use super::things::ThingId;
use super::{ActorId, Sim};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Each hop of telling keeps this much of the teller's sureness.
const HOP: f32 = 0.7;
/// Beliefs fade over about this long (s) when nothing renews them.
const FADE_SECS: f64 = crate::render::sky::DAY_SECONDS * 3.0;

/// What a belief says, structured where it can be.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Claim {
    /// What started an incident (a fire, a curse): the part, by name, and
    /// who was behind it.
    Cause { incident: u32, noun: String, what: String, by: Option<String> },
    /// Where a thing lies (or lay when last seen).
    Where { thing: ThingId, name: String, at: [f32; 3] },
    /// Who made a thing.
    Made { name: String, by: String },
}

impl Claim {
    /// One row per this: a repeat of it merges.
    pub fn key(&self) -> String {
        match self {
            Claim::Cause { incident, .. } => format!("cause:{incident}"),
            Claim::Where { thing, .. } => format!("where:{thing}"),
            Claim::Made { name, .. } => format!("made:{}", name.to_lowercase()),
        }
    }

    /// The same claim (a thing a little moved is still where it was).
    fn agrees(&self, o: &Claim) -> bool {
        match (self, o) {
            (Claim::Where { at: a, .. }, Claim::Where { at: b, .. }) => (Vec3::from(*a) - Vec3::from(*b)).length() < 3.0,
            (Claim::Cause { what: a, by: x, .. }, Claim::Cause { what: b, by: y, .. }) => a == b && x == y,
            _ => self == o,
        }
    }

    /// In words.
    pub fn words(&self) -> String {
        match self {
            Claim::Cause { noun, what, by, .. } => match by {
                Some(b) => format!("the {noun} was started by {what} ({b})"),
                None => format!("the {noun} started from {what}"),
            },
            Claim::Where { name, .. } => format!("the {name} is where you last saw it"),
            Claim::Made { name, by } => format!("{by} made the {name}"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum Source {
    Saw,
    Guessed,
    Told { by: String },
}

impl Source {
    fn rank(&self) -> u8 {
        match self {
            Source::Saw => 2,
            Source::Guessed => 1,
            Source::Told { .. } => 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Belief {
    pub claim: Claim,
    pub source: Source,
    pub sure: f32,
    pub importance: f32,
    pub t: f64,
    /// A corrupted teller twisted it on the way (for inspection; nobody in
    /// the world can tell).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub twisted: bool,
}

impl Belief {
    /// What decides whether it stays: importance × sureness × recency.
    fn keep(&self, now: f64) -> f32 {
        let age = ((now - self.t).max(0.0) / FADE_SECS) as f32;
        self.importance.max(0.05) * self.sure.max(0.05) * (-age).exp()
    }
}

#[derive(Default)]
pub struct Beliefs {
    pub by_holder: HashMap<i64, Vec<Belief>>,
    /// Holders whose rows changed since the last save.
    pub dirty: BTreeSet<i64>,
}

impl Beliefs {
    pub fn of(&self, holder: i64) -> &[Belief] {
        self.by_holder.get(&holder).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn get(&self, holder: i64, key: &str) -> Option<&Belief> {
        self.of(holder).iter().find(|b| b.claim.key() == key)
    }

    /// How many hold a belief about each key, and how many different ones:
    /// keys where people disagree.
    pub fn disagreements(&self) -> usize {
        let mut by_key: BTreeMap<String, Vec<&Claim>> = BTreeMap::new();
        for bs in self.by_holder.values() {
            for b in bs.iter().filter(|b| matches!(b.claim, Claim::Cause { .. })) {
                by_key.entry(b.claim.key()).or_default().push(&b.claim);
            }
        }
        by_key.values().filter(|cs| cs.iter().any(|c| !c.agrees(cs[0]))).count()
    }

    pub fn total(&self) -> usize {
        self.by_holder.values().map(|v| v.len()).sum()
    }

    pub fn most(&self) -> usize {
        self.by_holder.values().map(|v| v.len()).max().unwrap_or(0)
    }
}

impl Sim {
    /// Someone comes to hold a claim. A repeat raises sureness; a different
    /// claim about the same thing replaces it when it is better founded (what
    /// one saw beats what one was told; a newer sight of where a thing is
    /// beats an older one). Returns whether anything changed.
    pub fn believe(&mut self, holder: i64, claim: Claim, source: Source, sure: f32, importance: f32) -> bool {
        self.believe_twisted(holder, claim, source, sure, importance, false)
    }

    fn believe_twisted(&mut self, holder: i64, claim: Claim, source: Source, sure: f32, importance: f32, twisted: bool) -> bool {
        let t = self.t;
        let cap = self.cfg.beliefs_per_mind.max(1);
        let key = claim.key();
        let rows = self.beliefs.by_holder.entry(holder).or_default();
        let changed = match rows.iter_mut().find(|b| b.claim.key() == key) {
            Some(b) if b.claim.agrees(&claim) => {
                b.sure = (b.sure + (1.0 - b.sure) * sure * 0.5).min(1.0);
                b.importance = b.importance.max(importance);
                b.t = t;
                if source.rank() > b.source.rank() {
                    b.source = source;
                    b.twisted = false;
                }
                if let (Claim::Where { at: a, .. }, Claim::Where { at: n, .. }) = (&mut b.claim, &claim) {
                    *a = *n;
                }
                true
            }
            Some(b) => {
                // A fresh sight of where a thing lies always wins; what one
                // saw is never talked away; otherwise the surer claim wins.
                let better = match (&source, &b.source) {
                    (Source::Saw, _) => true,
                    (_, Source::Saw) => false,
                    _ => sure >= b.sure,
                };
                if better {
                    *b = Belief { claim, source, sure, importance, t, twisted };
                }
                better
            }
            None => {
                rows.push(Belief { claim, source, sure: sure.clamp(0.0, 1.0), importance, t, twisted });
                // Too many: the least worth keeping fades.
                if rows.len() > cap {
                    if let Some(i) = rows.iter().enumerate().min_by(|a, b| a.1.keep(t).total_cmp(&b.1.keep(t))).map(|(i, _)| i) {
                        rows.remove(i);
                    }
                }
                true
            }
        };
        if changed {
            self.beliefs.dirty.insert(holder);
        }
        changed
    }

    /// Forget a belief (what it said turned out not to be so).
    pub fn unbelieve(&mut self, holder: i64, key: &str) {
        if let Some(rows) = self.beliefs.by_holder.get_mut(&holder) {
            let n = rows.len();
            rows.retain(|b| b.claim.key() != key);
            if rows.len() != n {
                self.beliefs.dirty.insert(holder);
            }
        }
    }

    /// Everyone awake within `range` of `at` sees it and comes to hold the
    /// claim (with the memory text, as `witness` does).
    #[allow(clippy::too_many_arguments)]
    pub fn witness_claim(&mut self, at: Vec3, range: f32, text: &str, importance: f32, except: &[ActorId], claim: &Claim, source: Source, sure: f32) {
        let ids: Vec<i64> = self.cast.npcs.iter().filter(|n| n.here() && (n.a.pos - at).length() < range && !n.a.asleep && !except.contains(&ActorId::Npc(n.def.id)) && self.speaks_or_thinks(n.def.id)).map(|n| n.def.id).collect();
        self.witness(at, range, text, importance, except);
        for cid in ids {
            self.believe(cid, claim.clone(), source.clone(), sure, importance);
        }
    }

    /// Everyone awake within `range` comes to hold the claim (no memory
    /// text: what they saw is remembered elsewhere).
    #[allow(clippy::too_many_arguments)]
    pub fn notice_claim(&mut self, at: Vec3, range: f32, except: &[ActorId], claim: &Claim, source: Source, sure: f32, importance: f32) {
        let ids: Vec<i64> = self.cast.npcs.iter().filter(|n| n.here() && (n.a.pos - at).length() < range && !n.a.asleep && !except.contains(&ActorId::Npc(n.def.id)) && self.speaks_or_thinks(n.def.id)).map(|n| n.def.id).collect();
        for cid in ids {
            self.believe(cid, claim.clone(), source.clone(), sure, importance);
        }
    }

    /// Minds that can hold a claim worth asking about (people, not beasts).
    fn speaks_or_thinks(&self, cid: i64) -> bool {
        self.mind_of(ActorId::Npc(cid)) == crate::world::species::Mind::Sapient
    }

    /// Where someone sees a thing lie: kept, or put right.
    pub fn see_thing(&mut self, holder: i64, thing: ThingId) {
        if !self.speaks_or_thinks(holder) {
            return;
        }
        let Some(t) = self.things.get(thing) else { return };
        if t.held() || t.worn.is_some() {
            return;
        }
        let at = t.pos.to_array();
        let name = self.thing_name(thing);
        self.believe(holder, Claim::Where { thing, name, at }, Source::Saw, 1.0, 0.3);
    }

    /// Looking about where they believed things lay: what isn't there any
    /// more is forgotten (it moved, someone took it).
    pub fn look_about(&mut self, holder: i64, pos: Vec3) {
        let stale: Vec<String> = self
            .beliefs
            .of(holder)
            .iter()
            .filter_map(|b| match &b.claim {
                Claim::Where { thing, at, .. } if (Vec3::from(*at) - pos).length() < 8.0 => {
                    let there = self.things.get(*thing).is_some_and(|t| !t.held() && (t.pos - Vec3::from(*at)).length() < 3.0);
                    (!there).then(|| b.claim.key())
                }
                _ => None,
            })
            .collect();
        for k in stale {
            self.unbelieve(holder, &k);
        }
    }

    /// Where someone believes a thing like this lies (nearest first), with
    /// how sure they are: what they go to when it isn't in sight.
    pub fn believed_where(&self, holder: i64, fits: impl Fn(&str) -> bool, from: Vec3) -> Option<(ThingId, Vec3, f32)> {
        self.beliefs
            .of(holder)
            .iter()
            .filter_map(|b| match &b.claim {
                Claim::Where { thing, name, at } if fits(name) => Some((*thing, Vec3::from(*at), b.sure)),
                _ => None,
            })
            .min_by(|a, b| (a.1 - from).length().total_cmp(&(b.1 - from).length()))
    }

    /// One of the teller's beliefs passes to the listener: the most
    /// important one they don't hold the same way. A teller who means harm
    /// twists it (blame someone else); a listener who saw it themselves
    /// keeps what they saw. Returns the claim passed, if any.
    pub fn pass_belief(&mut self, teller: i64, listener: i64) -> Option<Claim> {
        if !self.speaks_or_thinks(teller) || !self.speaks_or_thinks(listener) {
            return None;
        }
        let tname = self.actor_name(ActorId::Npc(teller));
        let theirs: Vec<Belief> = self.beliefs.of(listener).to_vec();
        let pick = self
            .beliefs
            .of(teller)
            .iter()
            .filter(|b| !matches!(b.claim, Claim::Where { .. }) || b.importance >= 0.3)
            .filter(|b| theirs.iter().find(|x| x.claim.key() == b.claim.key()).is_none_or(|x| !x.claim.agrees(&b.claim)))
            .max_by(|a, b| (a.importance * a.sure).total_cmp(&(b.importance * b.sure)))
            .cloned()?;
        let mut claim = pick.claim.clone();
        let mut twisted = pick.twisted;
        // Only those who mean harm lie: blame someone else.
        if self.cast.get(teller).is_some_and(|n| n.species.hostile) {
            if let Claim::Cause { by, .. } = &mut claim {
                let scapegoat = self.scapegoat(teller, by.as_deref());
                if scapegoat.is_some() && scapegoat != *by {
                    *by = scapegoat;
                    twisted = true;
                }
            }
        }
        // What one saw for oneself isn't talked out of.
        if self.beliefs.get(listener, &claim.key()).is_some_and(|x| x.source == Source::Saw) {
            return None;
        }
        let sure = pick.sure * HOP;
        let changed = self.believe_twisted(listener, claim.clone(), Source::Told { by: tname }, sure, pick.importance * 0.9, twisted);
        if changed && twisted {
            let (a, b) = (self.actor_name(ActorId::Npc(teller)), self.actor_name(ActorId::Npc(listener)));
            let at = self.actor(ActorId::Npc(teller)).map(|x| x.pos);
            self.event("rumour", Some(ActorId::Npc(teller)), Some(ActorId::Npc(listener).key()), format!("{a} told {b} {}", claim.words()), at, serde_json::json!({ "belief": claim.key(), "twisted": true }));
        }
        changed.then_some(claim)
    }

    /// Whom a twisted mind blames: the one it likes least among those about
    /// (never whoever is truly behind it).
    fn scapegoat(&self, teller: i64, truly: Option<&str>) -> Option<String> {
        let me = ActorId::Npc(teller);
        let pos = self.actor(me)?.pos;
        self.cast
            .npcs
            .iter()
            .filter(|n| n.def.id != teller && n.here() && (n.a.pos - pos).length() < 120.0 && self.mind_of(ActorId::Npc(n.def.id)) == crate::world::species::Mind::Sapient)
            .map(|n| (self.social.affection(me, ActorId::Npc(n.def.id)), n.name().to_string()))
            .filter(|(_, name)| Some(name.as_str()) != truly)
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, name)| name)
    }

    /// What they believe that matters now, in words, for their planner and
    /// their talk: causes of trouble, who made what, where things lie that
    /// their goals want. Marked with how they know.
    pub fn beliefs_line(&self, cid: i64) -> Option<String> {
        let goals: Vec<String> = self.goals_of(cid).iter().map(|g| g.text.to_lowercase()).collect();
        let mut rows: Vec<&Belief> = self
            .beliefs
            .of(cid)
            .iter()
            .filter(|b| match &b.claim {
                Claim::Where { name, .. } => goals.iter().any(|g| super::goals::fits(name, g)) || b.importance >= 0.5,
                _ => true,
            })
            .collect();
        rows.sort_by(|a, b| (b.importance * b.sure).total_cmp(&(a.importance * a.sure)));
        let parts: Vec<String> = rows
            .into_iter()
            .take(4)
            .map(|b| {
                let how = match &b.source {
                    Source::Saw => "you saw it".to_string(),
                    Source::Guessed => "your guess from what you saw".to_string(),
                    Source::Told { by } => format!("{by} told you"),
                };
                let sure = if b.sure >= 0.75 { "" } else if b.sure >= 0.4 { ", fairly sure" } else { ", not sure" };
                let words = match &b.claim {
                    Claim::Where { name, at, .. } => {
                        let me = self.actor(ActorId::Npc(cid)).map(|a| a.pos).unwrap_or_default();
                        let d = Vec3::from(*at) - me;
                        format!("the {name} lies {:.0} m {}", d.length(), super::compass(d))
                    }
                    c => c.words(),
                };
                format!("{words} ({how}{sure})")
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join("; "))
    }
}
