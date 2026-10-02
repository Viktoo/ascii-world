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
    /// A short line the actor says.
    #[serde(default)]
    pub say: Option<String>,
    /// false for one-off results that should not be reused.
    #[serde(default = "yes")]
    pub cache: bool,
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
    /// Things waiting for this type: (thing, transform it rather than spawn next to it).
    pub then: Vec<(ThingId, bool)>,
    /// Or a place to put a new one (and who to hand it to).
    pub place: Option<(glam::Vec3, Option<ActorId>, Option<ThingId>)>,
    /// Things being reshaped into it (by whom, and what is worked into
    /// them, used up when it is done), and what the change was.
    pub reshape: Vec<(ThingId, ActorId, Option<ThingId>)>,
    pub change: String,
}

#[derive(Default)]
pub struct Interp {
    pub pending: BTreeMap<u64, PendingInterp>,
    /// Creations in progress: request id → (who, when).
    pub creating: HashMap<u64, (ActorId, f64)>,
    pub building: HashMap<u64, PendingBuild>,
    pub building_names: HashMap<String, u64>,
    /// Gestures being written: request id → (who, toward whom, name).
    pub gestures: HashMap<u64, (ActorId, Option<Target>, String)>,
    /// Species' own versions of gestures already asked for (name@species).
    pub variants_asked: std::collections::HashSet<String>,
    pub hits: u64,
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

/// The cache key: the words, normalised, and the kinds of things involved.
fn cache_key(text: &str, held: &str, target: &str) -> String {
    let t: String = text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty() && !matches!(*w, "the" | "a" | "an" | "my" | "i" | "to")).collect::<Vec<_>>().join(" ");
    format!("{t}|{}|{}", held.to_lowercase(), target.to_lowercase())
}

impl Sim {
    /// Ask the world what an action does (cached, else the LLM).
    pub fn interpret(&mut self, who: ActorId, text: &str, target: Option<Resolved>, hit: Option<glam::Vec3>) -> Result<Outcome, ActErr> {
        let held = self.actor(who).and_then(|a| a.held);
        let held_name = held.map(|h| self.thing_name(h)).unwrap_or_default();
        let target_name = target.as_ref().map(|r| r.name.clone()).unwrap_or_default();
        let key = cache_key(text, &held_name, &target_name);
        let name = self.actor_name(who);
        if let Some(fx) = super::persist::cached_interp(&self.db, &key) {
            self.interp.hits += 1;
            let p = PendingInterp { actor: who, text: text.to_string(), held, target: target.map(|r| r.target), hit, key, at: self.t };
            let msg = self.apply_interp(&p, &fx);
            self.event("interpreted", Some(who), None, format!("{name}: {text} → {msg}"), self.actor(who).map(|a| a.pos), json!({ "cached": true }));
            return Ok(Outcome::ok(msg));
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
            "time": crate::render::sky::time_label(self.t),
            "held": held.map(|h| self.describe_thing(h)),
        });
        if let Some(r) = target {
            let tv = match &r.target {
                Target::Actor(a) => json!({ "person": self.actor_name(*a) }),
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
        }
        v["properties"] = json!(self.vocab.names);
        v.to_string()
    }

    /// The LLM's answer arrived.
    pub fn on_interpreted(&mut self, id: u64, result: Result<Value, String>) {
        let Some(p) = self.interp.pending.remove(&id) else { return };
        let name = self.actor_name(p.actor);
        match result.and_then(|v| serde_json::from_value::<InterpEffect>(v).map_err(|e| e.to_string())) {
            Ok(fx) => {
                let msg = self.apply_interp(&p, &fx);
                if fx.cache && pieces_without_change(&fx, &p) {
                    // A piece came off the target, but its shape stayed: that
                    // answer would show nothing, so don't make it the rule.
                    crate::log::info(format!("not caching '{}': it makes new things from the target but leaves the target's shape as it was", p.text));
                } else if fx.cache {
                    super::persist::cache_interp(&self.db, &p.key, &fx);
                }
                let at = self.actor(p.actor).map(|a| a.pos);
                self.event("interpreted", Some(p.actor), None, format!("{name}: {} → {msg}", p.text), at, json!({ "effect": fx }));
            }
            Err(e) => {
                crate::log::error(format!("interpretation failed: {e}"));
                self.note_near(self.actor(p.actor).map(|a| a.pos).unwrap_or_default(), 30.0, Note::Info("Nothing seems to happen.".into()));
            }
        }
    }

    /// Apply an interpreter answer. Returns the narration.
    pub fn apply_interp(&mut self, p: &PendingInterp, fx: &InterpEffect) -> String {
        let pos = self.actor(p.actor).map(|a| a.pos).unwrap_or_default();
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
        for c in &fx.changes {
            let Some(id) = resolve(self, &c.target) else { continue };
            let vocab = self.vocab.clone();
            if let Some(t) = self.things.get_mut(id) {
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
                        self.apply_make_props(nid, m);
                        if let Some(h) = was_held_by {
                            self.hand_to(h, nid);
                        }
                    }
                }
                None => {
                    let id = self.next_id();
                    let key = m.name.trim().to_lowercase();
                    self.interp.building_names.insert(key, id);
                    self.interp.building.insert(id, PendingBuild { name: m.name.clone(), place: Some((at, was_held_by, None)), ..Default::default() });
                    let props: Vec<(String, f32)> = m.props.iter().map(|(k, v)| (k.clone(), *v)).collect();
                    let desc = if m.description.is_empty() { format!("{} (made from {})", m.name, made_from.join(" and ")) } else { m.description.clone() };
                    self.request_now(Request::BuildType { id, name: m.name.clone(), description: desc, size: m.size_m.unwrap_or([0.5, 0.5, 0.5]), props });
                }
            }
        }
        // Shape changes, at the spot that was touched.
        let mut why: Vec<String> = Vec::new();
        for c in fx.cut.iter().take(2) {
            if let Some(id) = resolve(self, &c.target) {
                let square = c.shape.trim().eq_ignore_ascii_case("square") || c.shape.trim().eq_ignore_ascii_case("box");
                if let Err(e) = self.cut(p.actor, id, p.hit, c.size_m, square) {
                    why.push(e);
                }
            }
        }
        for r in fx.reshape.iter().take(1) {
            if let Some(id) = resolve(self, &r.target) {
                let with = r.with.as_deref().and_then(|w| resolve(self, w));
                if let Err(e) = self.reshape(p.actor, id, &r.name, &r.change, with, p.hit) {
                    why.push(e);
                }
            }
        }
        for m in fx.make.iter().take(1) {
            if !m.text.trim().is_empty() {
                if let Err(e) = self.act(p.actor, super::Action::Create { text: m.text.trim().to_string() }) {
                    why.push(e.to_string());
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
        if let Some(line) = &fx.say {
            if !line.trim().is_empty() {
                self.say(p.actor, line.trim(), None);
            }
        }
        let msg = if fx.narration.trim().is_empty() { format!("{} {}.", super::physics::cap(&self.actor_name(p.actor)), p.text) } else { fx.narration.trim().to_string() };
        self.note_near(pos, 30.0, Note::Info(msg.clone()));
        self.witness(pos, 15.0, &format!("I saw {}: {msg}", self.actor_name(p.actor)), 0.35, &[p.actor]);
        msg
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

    /// A type the world asked for was written (or failed).
    pub fn on_type_built(&mut self, id: u64, type_id: Option<u32>) {
        let Some(b) = self.interp.building.remove(&id) else { return };
        self.interp.building_names.remove(&b.name.trim().to_lowercase());
        let Some(tid) = type_id else {
            crate::log::info(format!("no type for '{}'", b.name));
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
            self.spawn_or_transform(thing, tid, transform, &from);
        }
        for (thing, who, with) in b.reshape {
            let by = self.actor_name(who);
            self.set_shape_type(thing, tid, &by, &b.change, true);
            if let Some(w) = with {
                self.use_up(w);
            }
        }
        if let Some((at, holder, _)) = b.place {
            let origin = Origin { made_by: holder.map(|h| self.actor_name(h)), ..Default::default() };
            if let Some(nid) = self.spawn_thing(tid, at, 0.0, 1.0, origin, holder.is_none()) {
                let name = self.thing_name(nid);
                if let Some(h) = holder {
                    self.hand_to(h, nid);
                }
                self.event("made", holder, Some(format!("thing:{nid}")), format!("{} {name} came into being", super::physics::cap(super::article(&name))), Some(at), json!({}));
                self.note_near(at, 30.0, Note::Info(format!("{} {name} appears.", super::physics::cap(super::article(&name)))));
            }
        }
    }

    /// A creation request finished: `instances` are the new placed objects.
    pub fn on_created(&mut self, id: u64, instances: &[i64]) {
        let Some((who, _)) = self.interp.creating.remove(&id) else { return };
        let maker = self.actor_name(who);
        for i in instances {
            super::persist::set_made_by(&self.db, *i, &maker);
            if who != ActorId::Player {
                self.made.insert(*i);
            }
        }
        let Some(first) = instances.first().copied() else {
            self.event("create_failed", Some(who), None, format!("{maker} couldn't make it"), None, json!({}));
            return;
        };
        let (name, pos) = self.snap.instances.iter().find(|p| p.id == first).and_then(|p| self.snap.type_of(p.type_id).map(|t| (t.name().to_string(), p.pos))).unwrap_or(("thing".into(), self.player.pos));
        self.event("made", Some(who), Some(format!("instance:{first}")), format!("{maker} made {} {name}", super::article(&name)), Some(pos), json!({ "instances": instances }));
        if who != ActorId::Player {
            self.note_near(pos, 40.0, Note::Info(format!("{} made {} {name}.", super::physics::cap(&maker), super::article(&name))));
            self.witness(pos, 40.0, &format!("{maker} made {} {name}.", super::article(&name)), 0.5, &[who]);
            // Small things go straight into the maker's hands.
            if let Some(tid) = self.promote_instance(first) {
                if self.things.get(tid).is_some_and(|t| t.liftable(1)) && self.actor(who).is_some_and(|a| (a.pos - pos).length() < 8.0) {
                    self.hand_to(who, tid);
                }
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
                self.note_near(self.actor(who).map(|a| a.pos).unwrap_or_default(), 20.0, Note::Info(format!("(nobody quite knows how to {name})")));
            }
        }
    }

    /// Requests whose answers never came (stale) are dropped.
    pub fn expire_pending(&mut self) {
        let now = self.t;
        self.interp.pending.retain(|_, p| now - p.at < 300.0);
        self.interp.creating.retain(|_, c| now - c.1 < 600.0);
    }
}
