//! The living world: actors, live things, properties and rules, behaviour
//! code, characters' minds and their life together. Rendering, terminals and
//! LLM calls live elsewhere; the simulation talks to them through requests
//! (`out`) and notes (`notes`), so the same `Sim` runs inside the game, in
//! `pocket sim` (headless, fast-forward, seeded) and under `pocket act`.
//!
//! Determinism: everything iterates in id order and draws randomness from one
//! seeded generator, so a seed and a scripted LLM replay the same history.

pub mod actions;
pub mod actor;
pub mod behavior;
pub mod catchup;
pub mod config;
pub mod env;
pub mod headless;
pub mod inspect;
pub mod interp;
pub mod npc;
pub mod persist;
pub mod physics;
pub mod pick;
pub mod props;
pub mod render;
pub mod rules;
pub mod shape;
pub mod social;
pub mod things;

use crate::db::Db;
use crate::render::sky;
use crate::world::collide::{Obstacles, move_body};
use crate::world::describe::View;
use crate::world::scatter::ScatterCache;
use crate::world::{Solid, TypeEntry, WorldSnapshot};
use actor::Actor;
use config::SimConfig;
use glam::Vec3;
use npc::Cast;
use props::{Props, TypeProps, Vocab};
use rules::{Rule, RuleSpec};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use social::Social;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write;
use std::sync::Arc;
use things::{ThingId, Things};

pub use actions::{Action, ActErr};

/// Who acts: the player or a character (by character id).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActorId {
    Player,
    Npc(i64),
}

impl ActorId {
    /// A stable number: 0 for the player, the character id otherwise.
    pub fn code(self) -> i64 {
        match self {
            ActorId::Player => 0,
            ActorId::Npc(i) => i,
        }
    }
    pub fn from_code(c: i64) -> ActorId {
        if c == 0 { ActorId::Player } else { ActorId::Npc(c) }
    }
    pub fn key(self) -> String {
        match self {
            ActorId::Player => "player".into(),
            ActorId::Npc(i) => format!("npc:{i}"),
        }
    }
    pub fn parse(s: &str) -> Option<ActorId> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("player") || s == "you" || s == "0" {
            return Some(ActorId::Player);
        }
        s.trim_start_matches("npc:").parse::<i64>().ok().filter(|i| *i > 0).map(ActorId::Npc)
    }
}

impl Serialize for ActorId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ActorId::Player => s.serialize_str("player"),
            ActorId::Npc(i) => s.serialize_i64(*i),
        }
    }
}

impl<'de> Deserialize<'de> for ActorId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        match &v {
            Value::String(s) => ActorId::parse(s).ok_or_else(|| serde::de::Error::custom(format!("not an actor: {s}"))),
            Value::Number(n) => n.as_i64().map(ActorId::from_code).ok_or_else(|| serde::de::Error::custom("bad actor id")),
            _ => Err(serde::de::Error::custom("an actor is \"player\" or a character id")),
        }
    }
}

/// Anything an action can point at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// A live thing.
    Thing(ThingId),
    /// The player or a character.
    Actor(ActorId),
    /// A placed object of the static world (joins the live layer when touched).
    Instance(i64),
    /// The procedural item in a 4 m scatter cell (a stick, a tree, a tuft…).
    Cell([i32; 2]),
    /// A point in the world.
    Point([f32; 3]),
    /// The nearest thing or person with this name (for plans and agents).
    Name(String),
}

/// Requests for the slow world (LLM). Answers come back through the `on_*` methods.
#[derive(Clone, Debug)]
pub enum Request {
    /// A character decides what to do about an event.
    Decide { cid: i64, event: String, context: String },
    /// A character remembers something.
    Witness { cid: i64, text: String, importance: f32 },
    /// Something happened that no rule or behaviour covers: what does it do?
    Interpret { id: u64, actor: ActorId, text: String, context: String },
    /// Someone makes something new.
    Create { id: u64, by: ActorId, text: String, target: Vec3, yaw: f32, view: Box<View> },
    /// Two characters talk; the player is close enough to overhear.
    Chat { a: i64, b: i64, context: String },
    /// Write a new object type (for spawn() of an unknown name).
    BuildType { id: u64, name: String, description: String, size: [f32; 3], props: Vec<(String, f32)> },
    /// The player says something to a character (outside the talk screen).
    Talk { cid: i64, text: String, context: String },
    /// Write the pose keyframes of a gesture nobody knows yet.
    BuildGesture { id: u64, name: String },
    /// Rewrite a thing's shape code: `source` changed as `change` says, near
    /// `spot` (JSON, the type's own coordinates), with `cuts` baked in and
    /// maybe another thing worked in (name, source, its size relative to this one).
    EditType { id: u64, name: String, source: String, change: String, spot: String, cuts: Vec<[f32; 4]>, with: Option<(String, String, f32)> },
}

impl Request {
    fn costs_llm(&self) -> bool {
        !matches!(self, Request::Witness { .. })
    }
}

/// Something for the player's chat log.
#[derive(Clone, Debug, PartialEq)]
pub enum Note {
    /// Someone says something the player hears.
    Line { who: String, text: String },
    /// Something the player sees happen.
    Info(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SimEvent {
    pub t: f64,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<ActorId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_pos")]
    pub pos: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
}

/// Positions to a tenth of a metre, without f32 noise in the JSON.
fn ser_pos<S: Serializer>(p: &Option<[f32; 3]>, s: S) -> Result<S::Ok, S::Error> {
    match p {
        Some(v) => v.map(|x| (x as f64 * 10.0).round() / 10.0).serialize(s),
        None => s.serialize_none(),
    }
}

/// Everything that happened, newest last.
#[derive(Default)]
pub struct EventLog {
    pub recent: VecDeque<SimEvent>,
    pub sink: Option<Box<dyn Write + Send>>,
    pub total: u64,
    /// Events not yet saved to the database.
    pub unsaved: Vec<SimEvent>,
    pub counts: BTreeMap<String, u64>,
}

impl EventLog {
    pub fn push(&mut self, e: SimEvent) {
        if let Some(w) = self.sink.as_mut() {
            if let Ok(line) = serde_json::to_string(&e) {
                let _ = writeln!(w, "{line}");
            }
        }
        *self.counts.entry(e.kind.clone()).or_default() += 1;
        self.total += 1;
        self.unsaved.push(e.clone());
        if self.unsaved.len() > 4000 {
            self.unsaved.drain(..1000);
        }
        self.recent.push_back(e);
        while self.recent.len() > 3000 {
            self.recent.pop_front();
        }
    }

    /// Recent events about a subject ("thing:3", "npc:2", …).
    pub fn about(&self, subject: &str, actor: Option<ActorId>, n: usize) -> Vec<SimEvent> {
        let mut v: Vec<SimEvent> = self
            .recent
            .iter()
            .rev()
            .filter(|e| e.subject.as_deref() == Some(subject) || (actor.is_some() && e.actor == actor))
            .take(n)
            .cloned()
            .collect();
        v.reverse();
        v
    }
}

/// A queued LLM request waiting for budget.
struct Queued {
    req: Request,
    dist: f32,
    at: f64,
}

pub struct Sim {
    pub cfg: SimConfig,
    pub db: Arc<Db>,
    pub snap: Arc<WorldSnapshot>,
    pub cache: ScatterCache,
    pub seed: u64,
    rng: u64,
    /// Game time in seconds (one day = sky::DAY_SECONDS).
    pub t: f64,
    pub player: Actor,
    pub cast: Cast,
    pub things: Things,
    pub field: env::Field,
    pub vocab: Arc<Vocab>,
    pub rules: Arc<Vec<Rule>>,
    pub universe_rules: Vec<RuleSpec>,
    pub type_props: TypeProps,
    pub social: Social,
    pub log: EventLog,
    /// Requests ready for the brain.
    pub out: Vec<Request>,
    queue: Vec<Queued>,
    budget: f32,
    pub notes: Vec<Note>,
    pub has_llm: bool,
    /// How far up or down the player looks (radians): where "that hill" is.
    pub look_pitch: f32,
    /// The character the player is talking to (they stand still and face the player).
    pub talking_to: Option<i64>,
    pub interp: interp::Interp,
    acc_rules: f32,
    acc_behavior: f32,
    acc_regions: f32,
    /// Highlighted (hovered) target, drawn brighter.
    pub hover: Option<Target>,
    pub region_seen: HashMap<(i32, i32), f64>,
    next_req: u64,
    /// The things the player recently did (for dialogue context).
    pub recent_player: VecDeque<(f64, String)>,
    /// Actions queued for the player by an agent (`pocket act`).
    pub player_plan: VecDeque<actions::Action>,
    /// Placed objects that appeared in the last flips: (id, when).
    fresh: Vec<(i64, f64)>,
    /// Placed objects someone is known to have made (told by `on_created`).
    pub made: std::collections::HashSet<i64>,
}

impl Sim {
    pub fn new(db: Arc<Db>, snap: Arc<WorldSnapshot>, player_pos: Vec3, player_yaw: f32, t: f64, seed: u64) -> Sim {
        let cfg = SimConfig::load(|k| db.kv_get(k));
        let mut sim = Sim {
            cfg,
            db: db.clone(),
            snap: snap.clone(),
            cache: ScatterCache::default(),
            seed,
            rng: seed ^ 0x9E37_79B9_7F4A_7C15,
            t,
            player: Actor::new(player_pos, player_yaw),
            cast: Cast::default(),
            things: Things::default(),
            field: env::Field::default(),
            vocab: Arc::new(Vocab::builtin()),
            rules: Arc::new(Vec::new()),
            universe_rules: Vec::new(),
            type_props: TypeProps::default(),
            social: Social::default(),
            log: EventLog::default(),
            out: Vec::new(),
            queue: Vec::new(),
            budget: 2.0,
            notes: Vec::new(),
            has_llm: false,
            look_pitch: -0.12,
            talking_to: None,
            interp: interp::Interp::default(),
            acc_rules: 0.0,
            acc_behavior: 0.0,
            acc_regions: 1.0,
            hover: None,
            region_seen: HashMap::new(),
            next_req: 1,
            recent_player: VecDeque::new(),
            player_plan: VecDeque::new(),
            fresh: Vec::new(),
            made: std::collections::HashSet::new(),
        };
        sim.load_universe_rules();
        persist::load_gestures(&sim.db);
        sim.cast.sync(&snap, seed);
        persist::load(&mut sim);
        sim.social.seed_from_personas(&sim.cast);
        sim.sync_overlay();
        sim
    }

    // ------------------------------------------------------------ basics

    /// Uniform random in [0, 1) from the simulation's seeded generator.
    pub fn rand(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.rng >> 40) as u32) as f32 / (1u32 << 24) as f32
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_req;
        self.next_req += 1;
        id
    }

    pub fn hour(&self) -> f32 {
        sky::day_phase(self.t) * 24.0
    }

    pub fn night(&self) -> bool {
        sky::is_night(self.t)
    }

    /// Types used by live things and cells (reported to the model so it can
    /// leave unused ones out of the shader).
    pub fn types_in_use(&self) -> std::collections::HashSet<u32> {
        self.things.live().map(|t| t.type_id).chain(self.field.cells.values().map(|c| c.type_id)).collect()
    }

    pub fn type_entry(&self, id: u32) -> Option<Arc<TypeEntry>> {
        self.snap.type_of(id).cloned()
    }

    /// A type and its initial properties at scale 1.
    pub fn type_info(&mut self, id: u32) -> Option<(Arc<TypeEntry>, Arc<Props>)> {
        let ty = self.snap.type_of(id)?.clone();
        let base = self.type_props.get(&self.vocab, &ty);
        Some((ty, base))
    }

    /// A type by name (case-insensitive), newest first.
    pub fn type_by_name(&self, name: &str) -> Option<Arc<TypeEntry>> {
        let n = name.trim().to_lowercase();
        let mut best: Option<&Arc<TypeEntry>> = None;
        for t in self.snap.scene.types.values() {
            if t.name().to_lowercase() == n && best.is_none_or(|b| t.id > b.id) {
                best = Some(t);
            }
        }
        best.cloned()
    }

    pub fn actor(&self, id: ActorId) -> Option<&Actor> {
        match id {
            ActorId::Player => Some(&self.player),
            ActorId::Npc(c) => self.cast.get(c).map(|n| &n.a),
        }
    }

    pub fn actor_mut(&mut self, id: ActorId) -> Option<&mut Actor> {
        match id {
            ActorId::Player => Some(&mut self.player),
            ActorId::Npc(c) => self.cast.get_mut(c).map(|n| &mut n.a),
        }
    }

    pub fn actor_ids(&self) -> Vec<ActorId> {
        let mut v = vec![ActorId::Player];
        v.extend(self.cast.npcs.iter().map(|n| ActorId::Npc(n.def.id)));
        v
    }

    pub fn actor_name(&self, id: ActorId) -> String {
        match id {
            ActorId::Player => "the traveller".into(),
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.name().to_string()).unwrap_or_else(|| "someone".into()),
        }
    }

    /// Body height of an actor (m).
    pub fn actor_height(&self, id: ActorId) -> f32 {
        match id {
            ActorId::Player => 1.75,
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.def.persona.look.height).unwrap_or(1.75),
        }
    }

    pub fn thing_name(&self, id: ThingId) -> String {
        self.things.get(id).and_then(|t| self.snap.type_of(t.type_id)).map(|t| t.name().to_string()).unwrap_or_else(|| "thing".into())
    }

    pub fn dist_to_player(&self, p: Vec3) -> f32 {
        let d = p - self.player.pos;
        (d.x * d.x + d.z * d.z).sqrt()
    }

    // ------------------------------------------------------------ events

    #[allow(clippy::too_many_arguments)]
    pub fn event(&mut self, kind: &str, actor: Option<ActorId>, subject: Option<String>, text: impl Into<String>, pos: Option<Vec3>, data: Value) {
        let e = SimEvent { t: (self.t * 1000.0).round() / 1000.0, kind: kind.into(), actor, subject, text: text.into(), pos: pos.map(|p| [(p.x * 10.0).round() / 10.0, (p.y * 10.0).round() / 10.0, (p.z * 10.0).round() / 10.0]), data };
        self.log.push(e);
    }

    /// Tell the player, if they are close enough to notice.
    pub fn note_near(&mut self, at: Vec3, range: f32, n: Note) {
        if self.dist_to_player(at) <= range {
            self.notes.push(n);
        }
    }

    /// Characters within `range` of `at` who can see it remember it.
    pub fn witness(&mut self, at: Vec3, range: f32, text: &str, importance: f32, except: &[ActorId]) {
        let ids: Vec<i64> = self.cast.npcs.iter().filter(|n| (n.a.pos - at).length() < range && !n.a.asleep && !except.contains(&ActorId::Npc(n.def.id))).map(|n| n.def.id).collect();
        for cid in ids {
            if let Some(n) = self.cast.get_mut(cid) {
                if n.recently_witnessed(text, self.t) {
                    continue;
                }
                n.curiosity_bump(importance);
            }
            self.out.push(Request::Witness { cid, text: text.to_string(), importance });
        }
    }

    // ------------------------------------------------------------ LLM budget

    /// Queue a request that costs LLM budget; nearer ones go first.
    pub fn request(&mut self, req: Request, at: Vec3) {
        if !self.has_llm {
            return;
        }
        if !req.costs_llm() {
            self.out.push(req);
            return;
        }
        let dist = self.dist_to_player(at);
        self.queue.push(Queued { req, dist, at: self.t });
    }

    /// Player-initiated requests skip the queue.
    pub fn request_now(&mut self, req: Request) {
        if self.has_llm {
            self.out.push(req);
        }
    }

    fn pump_requests(&mut self, dt: f32) {
        let per_s = self.cfg.llm_per_min / 60.0;
        self.budget = (self.budget + per_s * dt).min((self.cfg.llm_per_min / 6.0).max(1.0));
        let now = self.t;
        self.queue.retain(|q| now - q.at < 20.0);
        self.queue.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        while self.budget >= 1.0 && !self.queue.is_empty() {
            let q = self.queue.remove(0);
            self.budget -= 1.0;
            self.out.push(q.req);
        }
    }

    /// Requests still waiting for budget.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    // ------------------------------------------------------------ world

    /// Solid things near `p`: static solids (scatter and placed objects not in
    /// the live layer) plus anchored or large live things.
    pub fn solids_near(&mut self, p: Vec3, r: f32) -> Vec<Solid> {
        let snap = self.snap.clone();
        let mut out = self.cache.solids_near(&snap, p, r);
        for id in self.things.near(p, r + 12.0) {
            let Some(t) = self.things.get(id) else { continue };
            if t.held() {
                continue;
            }
            let Some(ty) = snap.type_of(t.type_id) else { continue };
            if !ty.solid || t.props[props::P_SOLID] <= 0.0 {
                continue;
            }
            let big = t.anchored || ty.radius() * t.scale > 0.45;
            if !big {
                continue;
            }
            let gi = render::thing_inst(t, ty, [0.0; 4]);
            if (gi.center() - p).length() - gi.radius() > r {
                continue;
            }
            out.push(Solid { inst: gi, ty: ty.clone() });
        }
        out
    }

    /// Move an actor by `delta` with collisions (terrain, solids, other bodies).
    pub fn walk(&mut self, id: ActorId, delta: Vec3) {
        let Some(from) = self.actor(id).map(|a| a.pos) else { return };
        let solids = self.solids_near(from, 4.0);
        let bodies: Vec<Vec3> = self.actor_ids().into_iter().filter(|o| *o != id).filter_map(|o| self.actor(o).map(|a| a.pos)).filter(|p| (*p - from).length() < 4.0).collect();
        let obs = Obstacles { solids: &solids, bodies: &bodies };
        let r = if id == ActorId::Player { crate::world::collide::PLAYER_RADIUS } else { crate::world::collide::NPC_RADIUS };
        let to = move_body(&self.snap.terrain, &obs, from, delta, r);
        if let Some(a) = self.actor_mut(id) {
            a.moved = (to - from).length();
            a.pos = to;
        }
    }

    /// A new world version arrived (creation, region, undo).
    pub fn flip(&mut self, snap: Arc<WorldSnapshot>) {
        let alive: std::collections::HashSet<i64> = snap.instances.iter().map(|p| p.id).collect();
        let before: std::collections::HashSet<i64> = self.snap.instances.iter().map(|p| p.id).collect();
        // (A first full world after a quick start is not news.)
        if !before.is_empty() {
            for p in &snap.instances {
                if !before.contains(&p.id) {
                    self.fresh.push((p.id, self.t));
                }
            }
        }
        // Things whose source was undone go away with it.
        let gone: Vec<ThingId> = self.things.live().filter(|t| t.origin.instance.is_some_and(|i| !alive.contains(&i)) || snap.type_of(t.type_id).is_none()).map(|t| t.id).collect();
        for id in gone {
            self.release(id);
            self.things.forget(id);
        }
        self.snap = snap.clone();
        self.type_props.clear();
        self.cast.sync(&snap, self.seed);
        self.social.seed_from_personas(&self.cast);
        self.sync_overlay();
    }

    /// Detach a thing from whoever holds it.
    pub fn release(&mut self, id: ThingId) {
        let holders: Vec<ActorId> = self.things.get(id).map(|t| [t.holder, t.co_holder].into_iter().flatten().collect()).unwrap_or_default();
        for h in holders {
            if let Some(a) = self.actor_mut(h) {
                if a.held == Some(id) {
                    a.held = None;
                }
            }
        }
        if let Some(t) = self.things.get_mut(id) {
            t.holder = None;
            t.co_holder = None;
        }
    }

    /// Keep the static layer's view (hidden instances, taken scatter, cell
    /// looks) in step with the live layer.
    pub fn sync_overlay(&mut self) {
        let o = &mut self.cache.overlay;
        o.hidden = self.things.by_instance.keys().copied().collect();
        o.taken = self.things.taken.keys().copied().collect();
        o.cell_fx.clear();
        o.cell_gone.clear();
        for (c, cell) in &self.field.cells {
            let p = &cell.props;
            if p[props::P_FUEL] <= 0.0 && cell.small {
                o.cell_gone.insert(*c);
                continue;
            }
            let mut fx = [0.0; 4];
            fx[crate::render::FX_CHAR] = p[props::P_CHAR];
            fx[crate::render::FX_WET] = p[props::P_WET].min(1.0) * 0.6;
            if fx.iter().any(|v| *v > 0.02) {
                o.cell_fx.insert(*c, fx);
            }
        }
        o.version = o.version.wrapping_add(1);
    }

    // ------------------------------------------------------------ the tick

    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.25);
        self.t += dt as f64;
        self.step_actors(dt);
        self.step_social(dt);
        self.step_physics(dt);
        self.acc_behavior += dt;
        let bstep = 1.0 / self.cfg.behavior_hz;
        if self.acc_behavior >= bstep {
            let d = self.acc_behavior.min(1.0);
            self.acc_behavior = 0.0;
            self.step_behavior(d);
        }
        self.acc_rules += dt;
        let rstep = 1.0 / self.cfg.rules_hz;
        if self.acc_rules >= rstep {
            let d = self.acc_rules.min(1.0);
            self.acc_rules = 0.0;
            self.step_rules(d);
        }
        self.acc_regions += dt;
        if self.acc_regions >= 1.0 {
            self.acc_regions = 0.0;
            self.step_regions();
        }
        self.pump_requests(dt);
    }

    /// Take everything ready for the brain.
    pub fn drain_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.out)
    }

    pub fn drain_notes(&mut self) -> Vec<Note> {
        std::mem::take(&mut self.notes)
    }

    /// People near something that just appeared notice it (unless someone
    /// they saw made it: that is told by `on_created`).
    pub fn notice_fresh(&mut self) {
        let t = self.t;
        let (due, wait): (Vec<(i64, f64)>, Vec<(i64, f64)>) = self.fresh.drain(..).partition(|(_, at)| t - at >= 0.5);
        self.fresh = wait;
        for (id, _) in due {
            if self.made.contains(&id) {
                continue;
            }
            let Some(p) = self.snap.instances.iter().find(|p| p.id == id).cloned() else { continue };
            if self.dist_to_player(p.pos) > 70.0 {
                continue;
            }
            let Some(name) = self.snap.type_of(p.type_id).map(|t| t.name().to_string()) else { continue };
            let near: Vec<(i64, Vec3)> = self.cast.npcs.iter().filter(|n| (n.a.pos - p.pos).length() < 60.0).map(|n| (n.def.id, n.a.pos)).collect();
            for (cid, at) in near {
                let dir = compass(p.pos - at);
                self.out.push(Request::Witness { cid, text: format!("A {name} appeared {dir} of me, out of nowhere, after the traveller arrived."), importance: 0.6 });
                if let Some(n) = self.cast.get_mut(cid) {
                    n.curiosity_bump(0.5);
                }
                self.ask(cid, "new_building", &format!("A {name} just appeared {dir} of you, out of nowhere."));
            }
        }
    }

    /// Things that must always hold. Returns what is wrong (empty: all well).
    pub fn check_invariants(&self) -> Vec<String> {
        let mut bad = Vec::new();
        for t in self.things.live() {
            let Some(ty) = self.snap.type_of(t.type_id) else {
                bad.push(format!("thing {} has no type", t.id));
                continue;
            };
            if !t.pos.is_finite() || t.props.iter().any(|v| !v.is_finite()) {
                bad.push(format!("thing {} has a non-finite position or property", t.id));
            }
            if !t.held() && !t.anchored {
                let g = self.snap.terrain.height(t.pos.x, t.pos.z);
                let lowest = t.pos.y + ty.bottom * t.scale;
                if lowest < g - 0.5 - ty.radius() * t.scale && g > crate::terrain::WATER_LEVEL {
                    bad.push(format!("thing {} ({}) sank {:.1} m into the ground", t.id, ty.name(), g - lowest));
                }
            }
            for h in [t.holder, t.co_holder].into_iter().flatten() {
                match self.actor(h) {
                    Some(a) if a.held == Some(t.id) => {}
                    _ => bad.push(format!("thing {} says {} holds it, but they don't", t.id, h.key())),
                }
            }
            if t.holder.is_some() && t.holder == t.co_holder {
                bad.push(format!("thing {} is held twice by the same person", t.id));
            }
        }
        for a in self.actor_ids() {
            if let Some(id) = self.actor(a).and_then(|x| x.held) {
                match self.things.get(id) {
                    Some(t) if t.holder == Some(a) || t.co_holder == Some(a) => {}
                    _ => bad.push(format!("{} holds thing {id}, which doesn't know it", a.key())),
                }
            }
        }
        if self.things.len() > self.cfg.max_things {
            bad.push(format!("{} live things, over the cap of {}", self.things.len(), self.cfg.max_things));
        }
        bad
    }

    /// The player did something worth remembering in conversations.
    pub fn player_did(&mut self, what: String) {
        self.recent_player.push_back((self.t, what));
        while self.recent_player.len() > 4 {
            self.recent_player.pop_front();
        }
    }
}

/// Short compass word for a direction.
pub fn compass(d: Vec3) -> &'static str {
    let a = d.x.atan2(d.z).to_degrees().rem_euclid(360.0);
    ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"][((a + 22.5) / 45.0) as usize % 8]
}

/// "a" or "an".
pub fn article(name: &str) -> &'static str {
    if name.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) { "an" } else { "a" }
}

#[cfg(test)]
mod tests;
