//! The interpreter: when an action falls outside the rules and the things'
//! own code ("I rub the stone on the lantern"), the LLM decides what happens.
//! It must answer as primitives (property changes, state, new things,
//! removals), never only words, so its inventions obey the same rules as
//! everything else. Answers are cached by (what was done, to what, with
//! what): the same cause gives the same effect, and the world is learnable.

use super::actions::{ActErr, Outcome, Resolved};
use super::props::*;
use super::things::{Origin, ThingId};
use super::{ActorId, Note, Request, Sim, Target};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Change {
    /// "held", "target" or "actor".
    pub target: String,
    #[serde(default)]
    pub props: BTreeMap<String, f32>,
    #[serde(default)]
    pub state: BTreeMap<String, f32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Make {
    pub name: String,
    /// Replace the "held" or "target" thing with the new one.
    #[serde(default)]
    pub replace: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub size_m: Option<[f32; 3]>,
    #[serde(default)]
    pub props: BTreeMap<String, f32>,
}

/// What a deed does to a being: what it wears, its look sliders, its needs
/// and feelings change, it can learn a trick, its body can be reshaped (its
/// own from then on), and (where the world has its own forces) it can
/// become another species.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct BeingFx {
    /// Added to its needs (hunger, fatigue, social, fun, curiosity).
    #[serde(default)]
    pub needs: BTreeMap<String, f32>,
    /// Added to how it feels about the actor (affection, trust).
    #[serde(default)]
    pub feel: BTreeMap<String, f32>,
    /// Look sliders set (within the body's ranges).
    #[serde(default)]
    pub look: BTreeMap<String, f32>,
    /// Its size changes by this factor (2 = twice as big).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grow: Option<f32>,
    /// Layers put on it (made if nobody has made one yet).
    #[serde(default)]
    pub wear: Vec<Make>,
    /// Layers taken off it, by name.
    #[serde(default)]
    pub take_off: Vec<String>,
    /// A gesture it learns (a trick), done now and when greeting.
    #[serde(default)]
    pub learn: Option<String>,
    /// Another species it turns into.
    #[serde(default, rename = "become")]
    pub turn_into: Option<String>,
    /// What changes about its body's shape (a poofy tail): its body is
    /// rewritten, for this being only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reshape: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NewBeing {
    pub species: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Its look sliders (its species' body's), from the description.
    #[serde(default)]
    pub look: BTreeMap<String, f32>,
    /// Its size against its species' (1 = usual).
    #[serde(default)]
    pub size: Option<f32>,
}

/// Make something new in the world (a well, a lighthouse on that hill, a
/// stool): handed to the builder, which places it in view.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MakeFx {
    /// What to make and where, in words ("a stone well by the path").
    pub text: String,
}

/// Cut a piece out of a thing at the spot that was touched.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CutFx {
    #[serde(default = "target_role")]
    pub target: String,
    /// Radius (or half-width) in metres.
    #[serde(default = "cut_size")]
    pub size_m: f32,
    /// "round" (default) or "square".
    #[serde(default)]
    pub shape: String,
}

/// Rewrite a thing's shape code, maybe working another thing into it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ReshapeFx {
    #[serde(default = "target_role")]
    pub target: String,
    /// The changed thing's name ("hut with a stick on the wall").
    pub name: String,
    /// What changes about its shape, for the builder.
    pub change: String,
    /// "held": the held thing becomes part of it (and is used up).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub with: Option<String>,
}

fn target_role() -> String {
    "target".into()
}
fn cut_size() -> f32 {
    0.25
}

/// What the interpreter may answer.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct InterpEffect {
    #[serde(default)]
    pub narration: String,
    #[serde(default)]
    pub changes: Vec<Change>,
    #[serde(default)]
    pub create: Vec<Make>,
    #[serde(default)]
    pub remove: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub make: Vec<MakeFx>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cut: Vec<CutFx>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reshape: Vec<ReshapeFx>,
    /// What it does to a person or creature (the target).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub being: Option<BeingFx>,
    /// New beings brought into the world (only where the world allows it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub beings: Vec<NewBeing>,
    /// A short line the actor says.
    #[serde(default)]
    pub say: Option<String>,
    /// A character couldn't do it as things are: the one thing it needs
    /// (a place, a thing, someone, a time), for them to go and meet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<super::needs::Need>,
    /// false for one-off results that should not be reused.
    #[serde(default = "yes")]
    pub cache: bool,
}

impl InterpEffect {
    /// Something is still to be built before it all shows (a new thing, a reshape).
    fn builds(&self) -> bool {
        !self.make.is_empty() || !self.create.is_empty() || !self.reshape.is_empty()
    }

    /// Nothing in the world changes: a refusal or a "nothing happens".
    fn changes_nothing(&self) -> bool {
        self.changes.is_empty() && self.create.is_empty() && self.remove.is_empty() && self.make.is_empty() && self.cut.is_empty() && self.reshape.is_empty() && self.being.is_none() && self.beings.is_empty()
    }
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug)]
pub struct PendingInterp {
    pub actor: ActorId,
    pub text: String,
    pub held: Option<ThingId>,
    pub target: Option<Target>,
    /// The spot on the target that was touched (world).
    pub hit: Option<glam::Vec3>,
    pub key: String,
    pub at: f64,
}

#[derive(Clone, Debug, Default)]
pub struct PendingBuild {
    pub name: String,
    /// Whose deed it is (None: the world's own, like a rule's new thing).
    pub by: Option<ActorId>,
    /// Things waiting for this type: (thing, transform it rather than spawn next to it).
    pub then: Vec<(ThingId, bool)>,
    /// Or a place to put a new one (and who to hand it to).
    pub place: Option<(glam::Vec3, Option<ActorId>, Option<ThingId>)>,
    /// Things being reshaped into it (by whom, and what is worked into
    /// them, used up when it is done), and what the change was.
    pub reshape: Vec<(ThingId, ActorId, Option<ThingId>)>,
    pub change: String,
    /// A layer to put on someone when it exists: (wearer, by whom).
    pub wear_on: Option<(ActorId, ActorId)>,
    /// A character whose body this is, reshaped (see `reshape_being`).
    pub body_of: Option<i64>,
}

#[derive(Default)]
pub struct Interp {
    pub pending: BTreeMap<u64, PendingInterp>,
    /// Creations in progress: request id → (who, when, what).
    pub creating: HashMap<u64, (ActorId, f64, String)>,
    pub building: HashMap<u64, PendingBuild>,
    pub building_names: HashMap<String, u64>,
    /// What a deed's story says, held back until the build it waits on is
    /// done (build or creation id → (who, line)), so it isn't told too soon.
    /// With it, the player's deed's effect line (see `effect_line`).
    pub stories: HashMap<u64, Vec<(ActorId, String, Option<String>)>>,
    /// Deeds' "interpreted" events, held back the same way: (who, text, data).
    pub held_events: HashMap<u64, Vec<(ActorId, String, Value)>>,
    /// Gestures being written: request id → (who, toward whom, name).
    pub gestures: HashMap<u64, (ActorId, Option<Target>, String)>,
    /// Species' own versions of gestures already asked for (name@species).
    pub variants_asked: std::collections::HashSet<String>,
    /// Species whose own body was asked for (see `ask_species_bodies`).
    pub bodies_asked: std::collections::HashSet<String>,
    /// The trip (by its deadline) for which a character last asked its mount.
    pub mount_asked: HashMap<ActorId, u64>,
    pub hits: u64,
}

/// Slow work in progress, one kind per pending map in `Interp`: what the
/// game shows as being worked on. A new kind of slow request gets a kind here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorkKind {
    /// A deed the world is deciding the outcome of.
    Doing,
    /// Something new being made in the world (/ or a character's make).
    Conjuring,
    /// A new kind of thing being written.
    Making,
    /// A thing's shape being rewritten.
    Reshaping,
    /// A gesture nobody knew, being worked out.
    Learning,
}

impl WorkKind {
    pub fn verb(self) -> &'static str {
        match self {
            WorkKind::Doing => "doing",
            WorkKind::Conjuring => "conjuring",
            WorkKind::Making => "making",
            WorkKind::Reshaping => "reshaping",
            WorkKind::Learning => "learning",
        }
    }
}

/// One piece of slow work: its request id, kind, whose it is and what it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Work {
    pub id: u64,
    pub kind: WorkKind,
    pub who: Option<ActorId>,
    pub what: String,
}

/// New loose things appear next to a target whose shape doesn't change (no
/// cut, reshape, removal or replacement of it): likely "a piece came off" told
/// only in words.
fn pieces_without_change(fx: &InterpEffect, p: &PendingInterp) -> bool {
    let on_thing = matches!(p.target, Some(Target::Thing(_) | Target::Instance(_) | Target::Cell(_)));
    let is_target = |r: &str| matches!(r.trim().to_lowercase().as_str(), "target" | "object" | "other");
    let loose = fx.create.iter().any(|m| m.replace.is_none());
    let reshaped = fx.cut.iter().any(|c| is_target(&c.target)) || fx.reshape.iter().any(|r| is_target(&r.target)) || fx.remove.iter().any(|r| is_target(r)) || fx.create.iter().any(|m| m.replace.as_deref().is_some_and(is_target));
    on_thing && loose && !reshaped
}

/// The cache key: who acts (the traveler makes outright, characters by the
/// world's laws), the words, normalised, and the kinds of things involved.
fn cache_key(who: ActorId, text: &str, held: &str, target: &str) -> String {
    let t: String = text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty() && !matches!(*w, "the" | "a" | "an" | "my" | "i" | "to")).collect::<Vec<_>>().join(" ");
    let by = if who == ActorId::Player { "traveler" } else { "character" };
    format!("{by}|{t}|{}|{}", held.to_lowercase(), target.to_lowercase())
}

/// "heat ↑" for a change worth telling.
fn shift(name: &str, d: f32) -> Option<String> {
    (d.is_finite() && d.abs() >= 0.02).then(|| format!("{} {}", name.replace('_', " "), if d > 0.0 { '↑' } else { '↓' }))
}

/// The player's deed in one short line: what it changed that the eye may
/// miss (a property, a feeling, a need, a small cut), e.g. "the lantern-moth:
/// trust ↓, affection ↓". None when there is nothing like that. Every part of
/// an answer is named here, so a new kind of effect decides whether it shows.
fn effect_line(fx: &InterpEffect, moved: Vec<(String, Vec<String>)>) -> Option<String> {
    let InterpEffect { narration: _, changes: _, cut: _, create: _, remove: _, make: _, reshape: _, beings: _, say: _, needs: _, cache: _, being } = fx;
    // `changes`, `cut` and the being's needs and feelings are in `moved`.
    // New, removed and reshaped things, new beings and words show by themselves.
    if let Some(b) = being {
        let BeingFx { needs: _, feel: _, look: _, grow: _, wear: _, take_off: _, learn: _, turn_into: _, reshape: _ } = b;
        // Its look, size, layers, a trick, a new shape and a new species show by themselves.
    }
    let parts: Vec<String> = moved.into_iter().map(|(what, shifts)| format!("{what}: {}", shifts.join(", "))).collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

impl Sim {
    /// Ask the world what an action does (cached, else the LLM).
    pub fn interpret(&mut self, who: ActorId, text: &str, target: Option<Resolved>, hit: Option<glam::Vec3>) -> Result<Outcome, ActErr> {
        let held = self.actor(who).and_then(|a| a.held);
        let held_name = held.map(|h| self.thing_name(h)).unwrap_or_default();
        let target_name = target.as_ref().map(|r| r.name.clone()).unwrap_or_default();
        let key = cache_key(who, text, &held_name, &target_name);
        let name = self.actor_name(who);
        // The traveler's deeds in words draw on their charges.
        if who == ActorId::Player && self.has_llm && !self.spend_charge() {
            self.notes.push(super::Note::Info("Your power is spent until dawn (✦ 0). Kindness and getting through the night bring it back.".into()));
            return Ok(Outcome { ok: false, msg: "Your power is spent until dawn.".into(), pending: None, thing: None });
        }
        self.deed_started(who, text, target.as_ref().map(|r| r.target.clone()));
        if let Some(fx) = super::persist::cached_interp(&self.db, &key) {
            self.interp.hits += 1;
            let p = PendingInterp { actor: who, text: text.to_string(), held, target: target.map(|r| r.target), hit, key, at: self.t };
            let (msg, wait) = self.apply_interp(&p, &fx);
            if !fx.builds() {
                self.deed_landed(who);
            }
            self.deed_done(who, text);
            self.deed_event(who, format!("{name}: {text} → {msg}"), json!({ "cached": true }), wait);
            // The player hears the story from the notes (as with a fresh answer),
            // so the outcome only says what was tried, not the story again.
            let told = wait.is_some() || who == ActorId::Player;
            return Ok(if told { Outcome { ok: true, msg: format!("{name} tries to {text}…"), pending: None, thing: None } } else { Outcome::ok(msg) });
        }
        if !self.has_llm {
            let at = self.actor(who).map(|a| a.pos);
            self.event("nothing", Some(who), None, format!("{name} tried to {text}; nothing happened"), at, json!({}));
            return Ok(Outcome { ok: false, msg: "Nothing happens.".into(), pending: None, thing: None });
        }
        let id = self.next_id();
        let context = self.interp_context(who, held, target.as_ref(), hit);
        let req = Request::Interpret { id, actor: who, text: text.to_string(), context };
        if who == ActorId::Player {
            self.request_now(req);
        } else {
            let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
            self.request(req, at);
        }
        self.interp.pending.insert(id, PendingInterp { actor: who, text: text.to_string(), held, target: target.map(|r| r.target), hit, key, at: self.t });
        Ok(Outcome { ok: true, msg: format!("{name} tries to {text}…"), pending: Some(id), thing: None })
    }

    /// A person or creature as the interpreter sees it.
    fn describe_being(&self, a: ActorId, by: ActorId) -> Value {
        let wears: Vec<String> = self.worn_by(a).into_iter().map(|id| self.thing_name(id)).collect();
        let mut v = json!({ "person": self.actor_name(a), "wears": wears, "body": self.body_name(a) });
        if let ActorId::Npc(c) = a {
            if let Some(n) = self.cast.get(c) {
                let body = self.snap.type_of(n.body_ty).and_then(|t| t.ct.meta.body.clone());
                let look: serde_json::Map<String, Value> = body.map(|b| b.look.iter().enumerate().map(|(i, (name, lo, hi))| (name.clone(), json!({ "now": n.sliders.get(i).copied().unwrap_or(0.5), "range": [lo, hi] }))).collect()).unwrap_or_default();
                v["species"] = json!(n.species.name);
                v["mind"] = json!(n.species.mind);
                v["speech"] = json!(n.species.speech);
                v["look_sliders"] = Value::Object(look);
                v["tricks"] = json!(n.tricks);
                v["needs"] = json!({ "hunger": n.needs.hunger, "fatigue": n.needs.fatigue, "social": n.needs.social, "fun": n.needs.fun });
                v["feels_about_actor"] = json!(self.social.rel(a, by).map(|r| r.describe()).unwrap_or_else(|| "a stranger".into()));
                v["distance_m"] = json!(self.actor(by).map(|x| ((x.pos - n.a.pos).length() * 10.0).round() / 10.0));
            }
        }
        v["world_has_magic"] = json!(self.world_has_magic());
        v["beings_can_be_made"] = json!(self.cfg.create_beings);
        v["species_here"] = json!(self.snap.species.list.iter().map(|s| s.name.clone()).collect::<Vec<_>>());
        v
    }

    /// Each species with its body's look sliders and their usual ranges, for
    /// making a new being that looks as described.
    fn species_looks(&self) -> Value {
        let mut out = serde_json::Map::new();
        for sp in &self.snap.species.list {
            let body = self.snap.body_type(&sp.body).and_then(|b| b.ct.meta.body.clone());
            let look: serde_json::Map<String, Value> = body.map(|b| b.look.iter().map(|(name, lo, hi)| (name.clone(), json!({ "range": [lo, hi], "usual": sp.look.get(name) }))).collect()).unwrap_or_default();
            out.insert(sp.name.clone(), json!({ "body": sp.body, "look": look }));
        }
        Value::Object(out)
    }

    fn describe_thing(&self, id: ThingId) -> Value {
        let Some(t) = self.things.get(id) else { return Value::Null };
        let ty = self.snap.type_of(t.type_id);
        let base = ty.map(|ty| scaled(type_props(&self.vocab, ty), t.scale));
        json!({
            "name": ty.map(|t| t.name()).unwrap_or("thing"),
            "tags": ty.map(|t| t.ct.meta.tags.clone()).unwrap_or_default(),
            "size_m": ty.map(|ty| { let b = ty.ct.meta.bounds; [b[0] * 2.0 * t.scale, b[1] * 2.0 * t.scale, b[2] * 2.0 * t.scale] }),
            "props": named(&self.vocab, &t.props),
            "changed_from_new": base.map(|b| diff(&self.vocab, &t.props, &b)),
            "state": t.state,
            "made_by": t.origin.made_by,
            "cuts": t.shape.cuts.len(),
            "history": t.shape.edits.iter().rev().take(5).map(|e| format!("{} ({})", e.what, e.by)).collect::<Vec<_>>(),
        })
    }

    fn interp_context(&mut self, who: ActorId, held: Option<ThingId>, target: Option<&Resolved>, hit: Option<glam::Vec3>) -> String {
        let me = self.actor(who).cloned();
        let mut v = json!({
            "actor": self.actor_name(who),
            "actor_is": if who == ActorId::Player { "the traveler" } else { "a character" },
            "time": crate::render::sky::time_label(self.t),
            "held": held.map(|h| self.describe_thing(h)),
        });
        if let Some(r) = target {
            let tv = match &r.target {
                Target::Actor(a) => self.describe_being(*a, who),
                Target::Point(p) => json!({ "ground_or_far_point": p, "distance_m": (glam::Vec3::from(*p) - self.actor(who).map(|a| a.pos).unwrap_or_default()).length().round() }),
                other => match self.liven(other) {
                    Some(id) => {
                        let mut d = self.describe_thing(id);
                        if let (Some(t), Some(a)) = (self.things.get(id), self.actor(who)) {
                            d["distance_m"] = json!(((t.pos - a.pos).length() * 10.0).round() / 10.0);
                        }
                        if let Some(spot) = self.touch_spot(id, hit, who).and_then(|p| self.describe_spot(id, p)) {
                            d["touched_at"] = spot;
                        }
                        d
                    }
                    None => json!({ "name": r.name }),
                },
            };
            v["target"] = tv;
        }
        if let Some(m) = me {
            let mut near: Vec<String> = Vec::new();
            for id in self.things.near(m.pos, 8.0).into_iter().take(10) {
                near.push(self.thing_name(id));
            }
            v["nearby"] = json!(near);
            let beings: Vec<Value> = self.cast.npcs.iter().filter(|n| n.here() && ActorId::Npc(n.def.id) != who && (n.a.pos - m.pos).length() < 15.0).take(8).map(|n| json!({ "name": n.def.persona.name, "species": n.species.name })).collect();
            if !beings.is_empty() {
                v["beings_nearby"] = json!(beings);
            }
            if let ActorId::Npc(c) = who {
                if let Some(n) = self.cast.get(c) {
                    let pe = &n.def.persona;
                    let about: String = format!("{} {}", pe.personality, pe.goals).chars().take(300).collect();
                    v["actor_about"] = json!({ "species": n.species.name, "who_they_are": about.trim() });
                }
                v["around"] = self.around(m.pos);
            }
        }
        if self.cfg.create_beings {
            v["beings_can_be_made"] = json!(true);
            v["species_looks"] = self.species_looks();
        }
        v["properties"] = json!(self.vocab.names);
        if let Some(l) = self.twist_line(who) {
            v["darkness"] = json!(l);
        }
        v.to_string()
    }

    /// The LLM's answer arrived.
    pub fn on_interpreted(&mut self, id: u64, result: Result<Value, String>) {
        let Some(p) = self.interp.pending.remove(&id) else { return };
        let name = self.actor_name(p.actor);
        match result.and_then(|v| serde_json::from_value::<InterpEffect>(v).map_err(|e| e.to_string())) {
            Ok(fx) => {
                let (msg, wait) = self.apply_interp(&p, &fx);
                if !fx.builds() {
                    self.deed_landed(p.actor);
                }
                if fx.changes_nothing() {
                    // Nothing came of it: the charge isn't spent.
                    if p.actor == ActorId::Player {
                        self.refund_charge();
                    }
                    // A "no" depends on the moment; never make it the rule.
                    match &fx.needs {
                        Some(need) => self.on_need(p.actor, &p.text, need, &fx.narration),
                        None => self.deed_fell_through(p.actor, &p.text, &fx.narration),
                    }
                } else {
                    self.deed_done(p.actor, &p.text);
                    if fx.cache && pieces_without_change(&fx, &p) {
                        // A piece came off the target, but its shape stayed: that
                        // answer would show nothing, so don't make it the rule.
                        crate::log::info(format!("not caching '{}': it makes new things from the target but leaves the target's shape as it was", p.text));
                    } else if fx.cache {
                        super::persist::cache_interp(&self.db, &p.key, &fx);
                    }
                }
                self.deed_event(p.actor, format!("{name}: {} → {msg}", p.text), json!({ "effect": fx }), wait);
            }
            Err(e) => {
                self.deed_landed(p.actor);
                if p.actor == ActorId::Player {
                    self.refund_charge();
                }
                crate::log::error(format!("interpretation failed: {e}"));
                self.note_near(self.actor(p.actor).map(|a| a.pos).unwrap_or_default(), 30.0, Note::seen("Nothing seems to happen.".into(), p.actor == ActorId::Player));
            }
        }
    }

    /// What is around a character, by name, for the interpreter to pick a
    /// need from: places and big things, loose things, and people.
    fn around(&self, at: glam::Vec3) -> Value {
        let mut places: Vec<(f32, String)> = Vec::new();
        for i in &self.snap.instances {
            let d = (i.pos - at).length();
            if d < 150.0 {
                if let Some(t) = self.snap.type_of(i.type_id) {
                    let tags = t.ct.meta.tags.join(", ");
                    places.push((d, if tags.is_empty() { t.name().to_string() } else { format!("{} ({tags})", t.name()) }));
                }
            }
        }
        places.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut seen = std::collections::HashSet::new();
        let places: Vec<String> = places.into_iter().filter(|(_, n)| seen.insert(n.clone())).take(24).map(|(d, n)| format!("{n}, {d:.0} m")).collect();
        let mut things: Vec<(f32, String)> = self.things.live().filter(|t| t.holder.is_none()).filter_map(|t| {
            let d = (t.pos - at).length();
            (d < 40.0).then(|| (d, self.snap.type_of(t.type_id).map(|ty| ty.name().to_string()).unwrap_or_default()))
        }).collect();
        things.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut seen = std::collections::HashSet::new();
        let things: Vec<String> = things.into_iter().filter(|(_, n)| !n.is_empty() && seen.insert(n.clone())).take(12).map(|(d, n)| format!("{n}, {d:.0} m")).collect();
        let mut people: Vec<(f32, String)> = self.cast.npcs.iter().filter(|n| n.here()).map(|n| {
            let d = (n.a.pos - at).length();
            let about: String = n.def.persona.personality.chars().take(80).collect();
            (d, format!("{} ({}, {d:.0} m): {}", n.def.persona.name, n.species.name, about.trim()))
        }).filter(|(d, _)| *d > 0.5 && *d < 300.0).collect();
        people.sort_by(|a, b| a.0.total_cmp(&b.0));
        let people: Vec<String> = people.into_iter().take(10).map(|(_, s)| s).collect();
        json!({ "places": places, "things": things, "people": people })
    }

    /// Apply an interpreter answer. Returns the narration, and the build it
    /// is held back for (if any).
    pub fn apply_interp(&mut self, p: &PendingInterp, fx: &InterpEffect) -> (String, Option<u64>) {
        let pos = self.actor(p.actor).map(|a| a.pos).unwrap_or_default();
        // Changing someone takes their say-so; a refused change changes nothing.
        let being = match &p.target {
            Some(Target::Actor(a)) => Some(*a),
            _ => None,
        };
        if let (Some(b), Some(bf)) = (being, &fx.being) {
            if let Err(why) = self.being_consents(p.actor, b, bf) {
                if p.actor == ActorId::Player {
                    self.notes.push(Note::Info(why.clone()));
                }
                return (why, None);
            }
        }
        // Builds this deed waits on: its story is told when the first is done.
        let mut waits: Vec<u64> = Vec::new();
        let target_thing = p.target.as_ref().and_then(|t| match t {
            Target::Actor(_) | Target::Point(_) | Target::Name(_) => None,
            other => self.liven(other),
        });
        let resolve = |s: &Sim, role: &str| -> Option<ThingId> {
            match role.trim().to_lowercase().as_str() {
                "held" | "tool" | "it" => p.held.filter(|h| s.things.get(*h).is_some()),
                "target" | "object" | "other" => target_thing,
                _ => None,
            }
        };
        // What moved, for the effect line: (what, "prop ↑").
        let mut moved: Vec<(String, Vec<String>)> = Vec::new();
        for c in &fx.changes {
            let Some(id) = resolve(self, &c.target) else { continue };
            let vocab = self.vocab.clone();
            let mut shifts = Vec::new();
            if let Some(t) = self.things.get_mut(id) {
                let (props0, state0) = (t.props.clone(), t.state);
                for (k, v) in &c.props {
                    if let Some(i) = vocab.id(k) {
                        if v.is_finite() {
                            t.props[i] = *v;
                        }
                    }
                }
                for (k, v) in &c.state {
                    if let Some(i) = crate::lang::ir::state_field(k) {
                        if v.is_finite() {
                            t.state[i as usize] = *v;
                        }
                    }
                }
                sanitize(&mut t.props);
                t.dirty = true;
                t.asleep = false;
                for k in c.props.keys() {
                    if let Some(i) = vocab.id(k) {
                        shifts.extend(shift(k, t.props[i] - props0[i]));
                    }
                }
                for k in c.state.keys() {
                    if let Some(i) = crate::lang::ir::state_field(k) {
                        shifts.extend(shift(k, t.state[i as usize] - state0[i as usize]));
                    }
                }
            }
            if !shifts.is_empty() {
                moved.push((format!("the {}", self.thing_name(id)), shifts));
            }
        }
        for m in fx.create.iter().take(3) {
            let replace = m.replace.as_deref().and_then(|r| resolve(self, r));
            let at = replace.and_then(|r| self.things.get(r).map(|t| t.pos)).or_else(|| target_thing.and_then(|t| self.things.get(t).map(|x| x.pos))).unwrap_or(pos + glam::Vec3::Y * 0.0);
            let was_held_by = replace.and_then(|r| self.things.get(r).and_then(|t| t.holder));
            let made_from: Vec<String> = [p.held, target_thing].into_iter().flatten().map(|i| self.thing_name(i)).collect();
            let origin = Origin { made_by: Some(self.actor_name(p.actor)), made_from: made_from.clone(), ..Default::default() };
            if let Some(r) = replace {
                self.release(r);
                self.things.remove(r);
            }
            match self.type_by_name(&m.name) {
                Some(ty) => {
                    if let Some(nid) = self.spawn_thing(ty.id, at, 0.0, 1.0, origin, was_held_by.is_none()) {
                        self.record_creation(ty.id, "made", Some(p.actor), &made_from.join(" and "), at);
                        self.apply_make_props(nid, m);
                        if let Some(h) = was_held_by {
                            self.hand_to(h, nid);
                        } else if p.actor != ActorId::Player && self.things.get(nid).is_some_and(|t| t.liftable(1)) {
                            self.made_into_hands(p.actor, nid, true);
                        }
                    }
                }
                None => {
                    let id = self.next_id();
                    let key = m.name.trim().to_lowercase();
                    self.interp.building_names.insert(key, id);
                    self.interp.building.insert(id, PendingBuild { name: m.name.clone(), by: Some(p.actor), place: Some((at, was_held_by, None)), ..Default::default() });
                    let props: Vec<(String, f32)> = m.props.iter().map(|(k, v)| (k.clone(), *v)).collect();
                    let desc = if m.description.is_empty() { format!("{} (made from {})", m.name, made_from.join(" and ")) } else { m.description.clone() };
                    self.request_now(Request::BuildType { id, name: m.name.clone(), description: desc, size: m.size_m.unwrap_or([0.5, 0.5, 0.5]), props, fits: None });
                    waits.push(id);
                }
            }
        }
        // Shape changes, at the spot that was touched.
        let mut why: Vec<String> = Vec::new();
        for c in fx.cut.iter().take(2) {
            if let Some(id) = resolve(self, &c.target) {
                let square = c.shape.trim().eq_ignore_ascii_case("square") || c.shape.trim().eq_ignore_ascii_case("box");
                let name = self.thing_name(id);
                match self.cut(p.actor, id, p.hit, c.size_m, square) {
                    Ok(_) => moved.push((format!("the {name}"), vec![format!("cut {:.0} cm", (c.size_m * 100.0).max(1.0))])),
                    Err(e) => why.push(e),
                }
            }
        }
        for r in fx.reshape.iter().take(1) {
            if let Some(id) = resolve(self, &r.target) {
                let with = r.with.as_deref().and_then(|w| resolve(self, w));
                match self.reshape(p.actor, id, &r.name, &r.change, with, p.hit) {
                    Ok(Some(rid)) => waits.push(rid),
                    Ok(None) => {}
                    Err(e) => why.push(e),
                }
            }
        }
        for m in fx.make.iter().take(1) {
            if !m.text.trim().is_empty() {
                match self.act(p.actor, super::Action::Create { text: m.text.trim().to_string() }) {
                    Ok(o) => waits.extend(o.pending),
                    Err(e) => why.push(e.to_string()),
                }
            }
        }
        if !why.is_empty() {
            crate::log::info(format!("interpretation partly failed: {}", why.join("; ")));
        }
        for r in &fx.remove {
            if let Some(id) = resolve(self, r) {
                self.release(id);
                self.things.remove(id);
            }
        }
        if let (Some(b), Some(bf)) = (being, &fx.being) {
            if let Some(change) = bf.reshape.as_deref() {
                match self.reshape_being(p.actor, b, change) {
                    Ok(Some(rid)) => waits.push(rid),
                    Ok(None) => {}
                    Err(e) => crate::log::info(format!("reshaping a being failed: {e}")),
                }
            }
            self.apply_being(p.actor, b, bf);
            let feel = bf.feel.iter().filter(|(k, _)| b != p.actor && matches!(k.as_str(), "affection" | "love" | "liking" | "trust" | "rivalry" | "anger"));
            let shifts: Vec<String> = feel.chain(&bf.needs).filter_map(|(k, v)| shift(k, *v)).collect();
            if !shifts.is_empty() {
                let name = if b == ActorId::Player { "you".to_string() } else { self.actor_name(b).trim_end_matches(|c: char| c.is_ascii_digit()).trim_end().to_string() };
                moved.push((name, shifts));
            }
        }
        for nb in fx.beings.iter().take(2) {
            if let Err(e) = self.make_being(p.actor, nb) {
                why.push(e);
            }
        }
        if let Some(line) = &fx.say {
            if !line.trim().is_empty() {
                self.say(p.actor, line.trim(), None);
            }
        }
        let msg = if fx.narration.trim().is_empty() { format!("{} {}.", super::physics::cap(&self.actor_name(p.actor)), p.text) } else { fx.narration.trim().to_string() };
        let line = (p.actor == ActorId::Player).then(|| effect_line(fx, moved)).flatten();
        if let Some(first) = waits.first() {
            self.interp.stories.entry(*first).or_default().push((p.actor, msg.clone(), line));
            return (msg, Some(*first));
        }
        self.tell_story(pos, p.actor, &msg, line);
        (msg, None)
    }

    /// A deed's "interpreted" event: now, or once the build it waits on is
    /// done, so nothing (achievements included) counts it before it shows.
    fn deed_event(&mut self, who: ActorId, text: String, data: Value, wait: Option<u64>) {
        match wait {
            Some(id) => self.interp.held_events.entry(id).or_default().push((who, text, data)),
            None => {
                let at = self.actor(who).map(|a| a.pos);
                self.event("interpreted", Some(who), None, text, at, data);
            }
        }
    }

    /// The deeds waiting on build `id` are done, or came to nothing.
    pub(super) fn release_deeds(&mut self, id: u64, ok: bool) {
        for (who, text, mut data) in self.interp.held_events.remove(&id).unwrap_or_default() {
            if !ok {
                data["came_to_nothing"] = json!(true);
            }
            self.deed_event(who, text, data, None);
        }
    }

    /// Everything slow in progress, oldest first.
    pub fn work(&self) -> Vec<Work> {
        let i = &self.interp;
        let mut out: Vec<Work> = Vec::new();
        out.extend(i.pending.iter().map(|(id, p)| Work { id: *id, kind: WorkKind::Doing, who: Some(p.actor), what: p.text.clone() }));
        out.extend(i.creating.iter().map(|(id, c)| Work { id: *id, kind: WorkKind::Conjuring, who: Some(c.0), what: c.2.clone() }));
        out.extend(i.building.iter().map(|(id, b)| {
            let kind = if b.reshape.is_empty() && b.body_of.is_none() { WorkKind::Making } else { WorkKind::Reshaping };
            let what = match (b.reshape.first(), b.body_of) {
                (Some((thing, _, _)), _) if !b.change.trim().is_empty() => format!("the {}: {}", self.thing_name(*thing), b.change.trim()),
                (_, Some(c)) => format!("{}: {}", self.actor_name(ActorId::Npc(c)), b.change.trim()),
                _ => b.name.clone(),
            };
            Work { id: *id, kind, who: b.by, what }
        }));
        out.extend(i.gestures.iter().map(|(id, g)| {
            let name = g.2.split('@').next().unwrap_or("").replace('_', " ");
            Work { id: *id, kind: WorkKind::Learning, who: Some(g.0), what: format!("how to {name}") }
        }));
        out.sort_by_key(|w| w.id);
        out
    }

    /// A deed's story, told to those near and remembered by who saw it.
    fn tell_story(&mut self, pos: glam::Vec3, who: ActorId, msg: &str, effect: Option<String>) {
        // Your own deeds are answers to you; anyone else's change the world.
        let n = if who == ActorId::Player { Note::Info(msg.to_string()) } else { Note::Notable(msg.to_string()) };
        self.note_near(pos, 30.0, n);
        if let Some(e) = effect {
            self.note_near(pos, 30.0, Note::Effect(e));
        }
        self.witness(pos, 15.0, &format!("I saw {}: {msg}", self.actor_name(who)), 0.35, &[who]);
    }

    /// Tell the stories held back for a build that is done (at `at`), if
    /// any. Returns whether there were any.
    pub(super) fn tell_held_stories(&mut self, id: u64, at: glam::Vec3) -> bool {
        self.release_deeds(id, true);
        let Some(stories) = self.interp.stories.remove(&id) else { return false };
        let any = !stories.is_empty();
        for (who, msg, effect) in stories {
            self.tell_story(at, who, &msg, effect);
        }
        any
    }

    fn apply_make_props(&mut self, id: ThingId, m: &Make) {
        let vocab = self.vocab.clone();
        if let Some(t) = self.things.get_mut(id) {
            for (k, v) in &m.props {
                if let (Some(i), true) = (vocab.id(k), v.is_finite()) {
                    t.props[i] = *v;
                }
            }
            sanitize(&mut t.props);
        }
    }

    /// Put a thing in someone's hands (if they are free).
    pub fn hand_to(&mut self, who: ActorId, id: ThingId) {
        if self.actor(who).is_some_and(|a| a.held.is_some()) {
            return;
        }
        let strength = self.strength(who);
        if let Some(t) = self.things.get_mut(id) {
            if t.anchored || t.mass() > strength {
                return;
            }
            t.holder = Some(who);
            t.asleep = false;
        }
        if let Some(a) = self.actor_mut(who) {
            a.held = Some(id);
        }
    }

    /// A character's own small make goes into their hands (when it appeared
    /// `near` them), and on to whoever they made it for: fetched first if it
    /// appeared further off.
    fn made_into_hands(&mut self, who: ActorId, id: ThingId, near: bool) {
        if near {
            self.hand_to(who, id);
        }
        let ActorId::Npc(c) = who else { return };
        self.need_made(who, Target::Thing(id), true);
        let t = self.t;
        let Some(n) = self.cast.get_mut(c) else { return };
        let Some((to, until)) = n.deliver.take() else { return };
        if t > until {
            return;
        }
        let give = super::Action::Give { to: Target::Actor(to) };
        let steps = match n.a.held {
            Some(h) if h == id => vec![give],
            Some(_) => vec![super::Action::Drop, super::Action::Hold { target: Target::Thing(id) }, give],
            None => vec![super::Action::Hold { target: Target::Thing(id) }, give],
        };
        self.plan(who, steps, "hand it over", true);
    }

    /// A type the world asked for was written (or failed).
    pub fn on_type_built(&mut self, id: u64, type_id: Option<u32>) {
        let Some(b) = self.interp.building.remove(&id) else { return };
        if let Some(c) = b.body_of {
            self.on_body_reshaped(id, c, b, type_id);
            return;
        }
        for who in b.place.iter().filter_map(|p| p.1).chain(b.reshape.iter().map(|r| r.1)) {
            self.deed_landed(who);
        }
        self.interp.building_names.remove(&b.name.trim().to_lowercase());
        let Some(tid) = type_id else {
            crate::log::info(format!("no type for '{}'", b.name));
            self.release_deeds(id, false);
            if self.interp.stories.remove(&id).is_some() && b.reshape.is_empty() {
                if let Some((at, _, _)) = b.place {
                    self.note_near(at, 30.0, Note::Info("Nothing comes of it.".into()));
                }
            }
            for (thing, _, _) in &b.reshape {
                if let Some(at) = self.things.get(*thing).map(|t| t.pos) {
                    let name = self.thing_name(*thing);
                    self.note_near(at, 30.0, Note::Info(format!("The {name} stays as it was.")));
                }
            }
            return;
        };
        for (thing, transform) in b.then {
            let from = self.thing_name(thing);
            let at = self.things.get(thing).map(|t| t.pos).unwrap_or_default();
            self.record_creation(tid, "changed", b.by, &from, at);
            self.spawn_or_transform(thing, tid, transform, &from);
        }
        let mut told = false;
        for (thing, who, with) in b.reshape {
            let at = self.things.get(thing).map(|t| t.pos).unwrap_or_default();
            let from = self.thing_name(thing);
            self.record_creation(tid, "reshaped", Some(who), &from, at);
            told = told || self.tell_held_stories(id, at);
            self.set_shape_type(thing, tid, who, &b.change, true, !told);
            if let Some(w) = with {
                self.use_up(w);
            }
        }
        if let Some((wearer, by)) = b.wear_on {
            let at = self.actor(wearer).map(|a| a.pos).unwrap_or_default();
            self.record_creation(tid, "made", Some(by), "", at);
            let origin = Origin { made_by: Some(self.actor_name(by)), ..Default::default() };
            if let Some(nid) = self.spawn_thing(tid, at, 0.0, 1.0, origin, false) {
                if let Err(e) = self.wear(by, wearer, nid) {
                    crate::log::info(format!("couldn't put it on: {e:?}"));
                    self.things.remove(nid);
                }
            }
        }
        if let Some((at, holder, _)) = b.place {
            // Credit whose deed it was, not only who ends up holding it.
            let maker = b.by.or(holder);
            let origin = Origin { made_by: maker.map(|h| self.actor_name(h)), ..Default::default() };
            if let Some(nid) = self.spawn_thing(tid, at, 0.0, 1.0, origin, holder.is_none()) {
                self.record_creation(tid, if maker.is_some() { "made" } else { "changed" }, maker, "", at);
                let name = self.thing_name(nid);
                if let Some(h) = holder {
                    self.hand_to(h, nid);
                }
                self.event("made", maker, Some(format!("thing:{nid}")), format!("{} {name} came into being", super::physics::cap(super::article(&name))), Some(at), json!({}));
                if !self.tell_held_stories(id, at) {
                    self.note_near(at, 30.0, Note::Notable(format!("{} {name} appears.", super::physics::cap(super::article(&name)))));
                }
            }
        }
    }

    /// Count a kind of thing made in the world's record of creations.
    pub fn record_creation(&mut self, tid: u32, kind: &str, by: Option<ActorId>, from: &str, at: glam::Vec3) {
        let name = self.snap.type_of(tid).map(|t| t.name().to_string()).unwrap_or_default();
        self.note_creation(&format!("type:{tid}"), kind, &name, by, from, at);
    }

    /// Count something made, by key (`type:<id>`, `being:<id>`).
    pub fn note_creation(&mut self, key: &str, kind: &str, name: &str, by: Option<ActorId>, from: &str, at: glam::Vec3) {
        if matches!(by, Some(ActorId::Npc(_))) {
            self.cast.last_made = self.t;
        }
        let maker = by.map(|b| self.actor_name(b)).unwrap_or_else(|| "the world".into());
        let t = self.t;
        let _ = self.db.with(|c| crate::db::note_creation(c, key, kind, name, &maker, from, t, at.x, at.z));
    }

    /// A creation request finished: `instances` are the new placed objects.
    pub fn on_created(&mut self, id: u64, instances: &[i64]) {
        let Some((who, _, _)) = self.interp.creating.remove(&id) else { return };
        self.deed_landed(who);
        let maker = self.actor_name(who);
        for i in instances {
            super::persist::set_made_by(&self.db, *i, &maker);
            self.made.insert(*i, who);
        }
        let Some(first) = instances.first().copied() else {
            self.release_deeds(id, false);
            if self.interp.stories.remove(&id).is_some() {
                let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
                self.note_near(at, 30.0, Note::seen("Nothing comes of it.".into(), who == ActorId::Player));
            }
            self.event("create_failed", Some(who), None, format!("{maker} couldn't make it"), None, json!({}));
            return;
        };
        let placed = self.snap.instances.iter().find(|p| p.id == first).and_then(|p| self.snap.type_of(p.type_id).map(|t| (t.clone(), p.pos, p.scale)));
        let (name, pos) = placed.as_ref().map(|(t, p, _)| (t.name().to_string(), *p)).unwrap_or(("thing".into(), self.player.pos));
        // The traveler's things appear out of nowhere; a character's are made by hand.
        let how = if who == ActorId::Player { super::surprise::Arrival::FromNowhere } else { super::surprise::Arrival::Seen };
        let sight = match &placed {
            Some((ty, _, scale)) => self.sight_of(ty, *scale, how),
            None => super::surprise::Sight { how, extent: 1.0, strange: 0.0 },
        };
        let surprise = (self.plain_surprise(sight) * 100.0).round() / 100.0;
        if let Some(tid) = self.snap.instances.iter().find(|p| p.id == first).map(|p| p.type_id) {
            self.record_creation(tid, "built", Some(who), "", pos);
        }
        self.event("made", Some(who), Some(format!("instance:{first}")), format!("{maker} made {} {name}", super::article(&name)), Some(pos), json!({ "instances": instances, "surprise": surprise }));
        let told = self.tell_held_stories(id, pos);
        if who != ActorId::Player {
            if !told {
                self.note_near(pos, 40.0, Note::Notable(format!("{} made {} {name}.", super::physics::cap(&maker), super::article(&name))));
            }
            let memory = format!("{maker} made {} {name}.", super::article(&name));
            let tell = format!("{} just made {} {name} near you.", super::physics::cap(&maker), super::article(&name));
            let what = format!("the {name}");
            let news = super::surprise::News { at: pos, sight, memory: &memory, importance: 0.5, event: "something_made", tell: &tell, what: &what };
            self.startle(40.0, &news, &[who]);
            // Small things go straight into the maker's hands.
            match self.promote_instance(first) {
                Some(tid) if self.things.get(tid).is_some_and(|t| t.liftable(1)) => {
                    let near = self.actor(who).is_some_and(|a| (a.pos - pos).length() < 8.0);
                    self.made_into_hands(who, tid, near);
                }
                Some(tid) => self.need_made(who, Target::Thing(tid), false),
                None => self.need_made(who, Target::Instance(first), false),
            }
        }
    }

    /// A gesture's keyframes arrived: learn it, keep it, and do it.
    pub fn on_gesture_built(&mut self, id: u64, result: Result<Value, String>) {
        let Some((who, to, name)) = self.interp.gestures.remove(&id) else { return };
        let g = result.and_then(|v| {
            let mut g: super::actor::CustomGesture = serde_json::from_value(v).map_err(|e| e.to_string())?;
            g.name = name.clone();
            Ok(g)
        });
        match g.and_then(|g| super::actor::register_gesture(g.clone()).map(|k| (k, g))) {
            Ok((k, g)) => {
                super::persist::save_gesture(&self.db, &g);
                // A species' own version is used from now on; the shared one already played.
                if !g.name.contains('@') {
                    let _ = self.act(who, super::Action::Gesture { kind: k.name(), to });
                }
            }
            Err(e) => {
                crate::log::info(format!("gesture '{name}' not learned: {e}"));
                self.note_near(self.actor(who).map(|a| a.pos).unwrap_or_default(), 20.0, Note::seen(format!("(nobody quite knows how to {name})"), who == ActorId::Player));
            }
        }
    }

    /// Requests whose answers never came (stale) are dropped.
    pub fn expire_pending(&mut self) {
        let now = self.t;
        self.interp.pending.retain(|_, p| now - p.at < 300.0);
        self.interp.creating.retain(|_, c| now - c.1 < 600.0);
        let Interp { stories, held_events, creating, building, .. } = &mut self.interp;
        stories.retain(|id, _| creating.contains_key(id) || building.contains_key(id));
        held_events.retain(|id, _| creating.contains_key(id) || building.contains_key(id));
    }
}
