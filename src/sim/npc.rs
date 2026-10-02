//! Characters' minds. Cheap code runs every tick: needs drift, and when a
//! character has nothing to do it scores a few options (eat, sleep, play,
//! seek company, look at something new, flee a fire, wander) and turns the
//! best into a short plan of shared actions. The LLM is a rare planner: for
//! big moments it writes a goal and steps, and the same executor carries
//! them out.

use super::actions::{Action, ActErr};
use super::actor::{Actor, GestureKind, NPC_RUN, NPC_WALK, PLAYER_SPEED, Task};
use super::props::*;
use super::{ActorId, Request, Sim, Target};
use crate::render::GpuInst;
use crate::world::characters::{Decision, Needs, SavedState};
use crate::world::species::{Dims, Species};
use crate::world::{CharacterDef, TypeEntry, WorldSnapshot};
use glam::Vec3;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Personality as numbers (0..1), read from the persona's words.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Traits {
    pub sociable: f32,
    pub playful: f32,
    pub curious: f32,
    pub brave: f32,
    pub generous: f32,
    pub crafty: f32,
}

impl Traits {
    pub fn from_persona(p: &crate::world::Persona, seed: u32) -> Traits {
        let text = format!("{} {} {} {}", p.personality, p.goals, p.appearance, p.voice).to_lowercase();
        let has = |ws: &[&str]| ws.iter().filter(|w| text.contains(*w)).count() as f32;
        let jitter = |k: u32| (crate::noise::u2f(crate::noise::pcg(seed ^ k.wrapping_mul(0x9E37))) - 0.5) * 0.2;
        let clamp = |v: f32| v.clamp(0.05, 0.95);
        let young = p.age > 0 && p.age < 16;
        Traits {
            sociable: clamp(0.5 + 0.15 * has(&["friendly", "warm", "cheerful", "talkative", "gregarious", "kind", "jovial", "chatty", "outgoing"]) - 0.15 * has(&["shy", "gruff", "quiet", "reserved", "withdrawn", "solitary", "aloof", "suspicious"]) + jitter(1)),
            playful: clamp(0.4 + 0.15 * has(&["playful", "mischievous", "lively", "joyful", "fun", "games", "restless", "adventurous"]) + if young { 0.35 } else { 0.0 } - 0.1 * has(&["stern", "serious", "grim", "weary"]) + jitter(2)),
            curious: clamp(0.45 + 0.15 * has(&["curious", "inquisitive", "scholar", "inventor", "nosy", "explorer", "questions"]) + jitter(3)),
            brave: clamp(0.5 + 0.15 * has(&["brave", "bold", "fearless", "soldier", "guard", "stubborn"]) - 0.15 * has(&["timid", "nervous", "anxious", "fearful", "cautious"]) + jitter(4)),
            generous: clamp(0.5 + 0.15 * has(&["generous", "kind", "giving", "caring", "gentle"]) - 0.2 * has(&["greedy", "stingy", "selfish", "miser"]) + jitter(5)),
            crafty: clamp(0.35 + 0.2 * has(&["smith", "carpenter", "maker", "tinker", "builder", "artist", "craft", "inventor", "potter", "weaver", "mender"]) + jitter(6)),
        }
    }
}

impl Traits {
    /// A species' temper sets the starting point; the persona's words still
    /// nudge it (a timid dragon is possible).
    pub fn with_temper(self, sp: &Species, t: crate::world::species::Temper) -> Traits {
        use crate::world::species::{Mind, Social};
        let blend = |own: f32, base: f32| (own - 0.5) * 0.5 + base;
        let sociable = match sp.social {
            Social::Solitary => 0.2,
            Social::Pair => 0.45,
            Social::Pack | Social::Herd => 0.65,
            Social::Village => 0.6,
        };
        let sapient = sp.mind == Mind::Sapient;
        Traits {
            sociable: blend(self.sociable, sociable).clamp(0.0, 1.0),
            playful: blend(self.playful, t.playful).clamp(0.0, 1.0),
            curious: self.curious,
            brave: blend(self.brave, t.bold).clamp(0.0, 1.0),
            generous: if sapient { self.generous } else { 0.2 },
            crafty: if sapient { self.crafty } else { 0.0 },
        }
    }
}

pub struct Npc {
    pub def: Arc<CharacterDef>,
    pub species: Arc<Species>,
    /// Its own temper (its species', or its line's).
    pub temper: crate::world::species::Temper,
    /// The body type it is drawn with.
    pub body_ty: u32,
    /// Look sliders (k.a … k.e).
    pub sliders: [f32; 5],
    pub a: Actor,
    pub needs: Needs,
    pub traits: Traits,
    /// What they are doing right now, in a few words.
    pub doing: String,
    pub goal: String,
    pub plan: VecDeque<Action>,
    pub plan_from_llm: bool,
    pub think_at: f64,
    pub next_llm: f64,
    pub rng: u32,
    witnessed: VecDeque<(u64, f64)>,
    pub last_player_near: f64,
    acc: f32,
    pub last_sim: f64,
    pub decisions: VecDeque<(f64, String)>,
    pub bored_since: f64,
    pub last_line: f64,
    pub last_work: f64,
    /// When an animal last greeted its person.
    pub last_greet: f64,
    /// Someone to hand the next thing they make to, until this game time.
    pub deliver: Option<(ActorId, f64)>,
    /// A deed they set out to do and what it waits on (see `needs`).
    pub mission: Option<super::needs::Mission>,
    /// Someone who asked them a favour and awaits the answer, until this game time.
    pub asked_by: Option<(ActorId, f64)>,
    /// Someone they agreed to help, until this game time.
    pub helping: Option<(ActorId, f64)>,
    /// The deed in words they are in the middle of (asked, or being made),
    /// so it can be done again if the session ends before it lands.
    pub deed: Option<(String, Option<Target>, f64)>,
    /// Loaded with work under way: fix up what was waiting on an answer.
    pub restored: bool,
    pub dead: bool,
    pub dressed: bool,
    /// Gestures it was taught (an animal's tricks), done when greeting.
    pub tricks: Vec<String>,
    /// Game time it was born (for the young), or 0.
    pub born: f64,
    pub parents: Vec<i64>,
    pub lineage: i64,
    pub last_birth: f64,
    pub frights: u32,
    /// Grown-up fraction (0.3 newborn … 1 adult).
    pub growth: f32,
}

impl Npc {
    pub fn name(&self) -> &str {
        &self.def.persona.name
    }

    pub fn rand(&mut self) -> f32 {
        self.rng = crate::noise::pcg(self.rng);
        crate::noise::u2f(self.rng)
    }

    pub fn saved(&self, t: f64) -> SavedState {
        SavedState { x: self.a.pos.x, z: self.a.pos.z, yaw: self.a.yaw, asleep: self.a.asleep, needs: Some(self.needs), held: self.a.held, goal: self.goal.clone(), t, dead: self.dead, dressed: self.dressed, tricks: self.tricks.clone(), born: self.born, parents: self.parents.clone(), lineage: self.lineage, last_birth: self.last_birth, frights: self.frights, work: self.work() }
    }

    pub fn gpu(&self, body: &TypeEntry) -> GpuInst {
        let rot = self.a.yaw;
        let d = &self.a.dims;
        // Bodies that draw their own height (people) are scaled only by the
        // world's size; others by their whole size.
        let own_height = body.ct.meta.body.as_ref().is_some_and(|b| b.slider("height").is_some());
        let r = if own_height { d.ratio } else { d.scale };
        let mut g = GpuInst {
            pos_scale: [self.a.pos.x, self.a.pos.y, self.a.pos.z, d.scale],
            rot: [rot.cos(), rot.sin(), body.sphere_r * r.max(d.scale), 1.0],
            k0: [self.def.id as f32 % 97.0, d.scale, self.sliders[0], self.sliders[1]],
            k1: [self.sliders[2], self.sliders[3], self.sliders[4], self.a.phase],
            info: body.gpu_info(),
            ..Default::default()
        };
        g.set_state(&self.a.pose);
        g
    }

    /// Avoid remembering the same thing over and over.
    pub fn recently_witnessed(&mut self, text: &str, t: f64) -> bool {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        let k = h.finish();
        self.witnessed.retain(|(_, at)| t - at < 120.0);
        if self.witnessed.iter().any(|(x, _)| *x == k) {
            return true;
        }
        self.witnessed.push_back((k, t));
        false
    }

    pub fn curiosity_bump(&mut self, amount: f32) {
        self.needs.curiosity = (self.needs.curiosity + amount * self.traits.curious).min(1.0);
    }

    fn log_decision(&mut self, t: f64, s: String) {
        self.decisions.push_back((t, s));
        while self.decisions.len() > 12 {
            self.decisions.pop_front();
        }
    }
}

#[derive(Default)]
pub struct Cast {
    pub npcs: Vec<Npc>,
    index: HashMap<i64, usize>,
    /// When any character last turned to their craft (see `SimConfig::work_gap_secs`).
    pub last_work: f64,
    /// The species book the cast was last fitted to.
    book: Option<Arc<crate::world::species::SpeciesBook>>,
}

impl Cast {
    /// Add characters that appeared in a new snapshot; when the species
    /// changed (one was written or replaced), everyone takes theirs anew.
    pub fn sync(&mut self, snap: &WorldSnapshot, seed: u64) {
        let fresh_book = !self.book.as_ref().is_some_and(|b| Arc::ptr_eq(b, &snap.species));
        self.book = Some(snap.species.clone());
        if fresh_book {
            for n in self.npcs.iter_mut() {
                let species = snap.species.of(&n.def.persona.species);
                let body_ty = snap.body_type(&species.body).map(|b| b.id).unwrap_or(0);
                if species.as_ref() != n.species.as_ref() || body_ty != n.body_ty {
                    fit(n, snap, seed);
                }
            }
        }
        for def in &snap.characters {
            if !self.index.contains_key(&def.id) {
                self.add(def.clone(), snap, seed);
            }
        }
    }

    /// Bring one character into the cast.
    pub fn add(&mut self, def: Arc<CharacterDef>, snap: &WorldSnapshot, seed: u64) {
        let s = &def.state;
        let pos = if s.x == 0.0 && s.z == 0.0 { def.home } else { Vec3::new(s.x, 0.0, s.z) };
        let pos = Vec3::new(pos.x, snap.terrain.height(pos.x, pos.z), pos.z);
        let rng = crate::noise::pcg(def.id as u32 ^ 0xC0FFEE ^ seed as u32);
        let mut a = Actor::new(pos, s.yaw);
        a.asleep = s.asleep;
        let traits = Traits::from_persona(&def.persona, rng);
        let needs = s.needs.unwrap_or_else(|| {
            let r = |k: u32| crate::noise::u2f(crate::noise::pcg(rng ^ k)) * 0.4;
            Needs { hunger: 0.1 + r(1), fatigue: r(2) * 0.5, social: 0.2 + r(3), fun: 0.2 + r(4), curiosity: 0.2 + r(5) }
        });
        let mut n = Npc {
            def: def.clone(),
            species: snap.species.of(""),
            temper: Default::default(),
            body_ty: 0,
            sliders: [0.5; 5],
            a,
            needs,
            traits,
            doing: "idle".into(),
            goal: s.goal.clone(),
            plan: VecDeque::new(),
            plan_from_llm: false,
            think_at: 0.0,
            next_llm: 0.0,
            rng,
            witnessed: VecDeque::new(),
            last_player_near: f64::MIN,
            acc: 0.0,
            last_sim: s.t,
            decisions: VecDeque::new(),
            bored_since: f64::MAX,
            last_line: f64::MIN,
            last_work: f64::MIN,
            last_greet: f64::MIN,
            deliver: None,
            mission: None,
            asked_by: None,
            helping: None,
            deed: None,
            restored: false,
            dead: s.dead,
            dressed: s.dressed,
            tricks: s.tricks.clone(),
            born: s.born,
            parents: s.parents.clone(),
            lineage: if s.lineage != 0 { s.lineage } else { def.id },
            last_birth: s.last_birth,
            frights: s.frights,
            growth: if s.born > 0.0 { 0.3 } else { 1.0 },
        };
        if let Some(w) = s.work.as_ref() {
            n.restore_work(w);
        }
        fit(&mut n, snap, seed);
        self.index.insert(def.id, self.npcs.len());
        self.npcs.push(n);
    }

    pub fn get(&self, id: i64) -> Option<&Npc> {
        self.index.get(&id).map(|i| &self.npcs[*i])
    }

    pub fn get_mut(&mut self, id: i64) -> Option<&mut Npc> {
        self.index.get(&id).copied().map(move |i| &mut self.npcs[i])
    }
}

/// Fit a character to its species: body, look sliders, size, roles and
/// temper (on arrival, and again when its species changes).
pub fn fit(n: &mut Npc, snap: &WorldSnapshot, seed: u64) {
    let species = snap.species.of(&n.def.persona.species);
    let body = snap.body_type(&species.body);
    let bmeta = body.and_then(|b| b.ct.meta.body.clone()).unwrap_or_default();
    let variety = species.varieties.iter().find(|v| v.name == n.def.persona.variety);
    let rng = crate::noise::pcg(n.def.id as u32 ^ 0xC0FFEE ^ seed as u32);
    n.sliders = crate::world::species::sliders(&bmeta, &species, variety, &n.def.persona.look, rng);
    // The young are smaller, all over.
    n.a.dims = body_dims(&bmeta, &species, &n.sliders, snap.species.size_of(&species.name) * n.growth.clamp(0.2, 1.0));
    n.a.roles = super::actor::role_mask(&bmeta);
    n.a.species = if species.is_human() { String::new() } else { species.name.clone() };
    n.traits = Traits::from_persona(&n.def.persona, rng);
    n.temper = n.def.persona.temper.unwrap_or(species.temper);
    if !species.is_human() {
        n.traits = n.traits.with_temper(&species, n.temper);
    }
    n.body_ty = body.map(|b| b.id).unwrap_or(0);
    n.species = species;
}

/// Size numbers for a character: people draw their own height (a slider);
/// other bodies are scaled by their species' size. `world` is the universe's
/// size multiplier for the species.
pub fn body_dims(b: &crate::lang::ir::Body, sp: &Species, sliders: &[f32; 5], world: f32) -> Dims {
    let (height, scale) = match b.slider("height") {
        Some(i) => (sliders[i], world),
        None => (b.height, sp.size * world),
    };
    let norm = match b.slider("height") {
        Some(_) => 1.75,
        None => b.height * sp.size,
    };
    let mut d = Dims::of(b, height, scale, sp.mass);
    d.mass = sp.mass * (d.height / norm.max(0.01)).powi(3);
    d
}

/// Plans written by the LLM are close to the action JSON but not always
/// exact: accept plain strings for targets and a few common verb aliases.
pub fn parse_step(v: &Value) -> Option<Action> {
    let mut v = v.clone();
    let obj = v.as_object_mut()?;
    if !obj.contains_key("do") {
        for alias in ["action", "verb", "type"] {
            if let Some(x) = obj.remove(alias) {
                obj.insert("do".into(), x);
                break;
            }
        }
    }
    let raw = obj.get("do")?.as_str()?.trim().to_lowercase().replace([' ', '-'], "_");
    let verb = match raw.as_str() {
        "walk_to" | "go_to" | "go" | "approach" | "walk" => "goto",
        "pick_up" | "pickup" | "take" | "grab" | "lift" | "carry" => "hold",
        "put_down" | "set_down" | "release" => "drop",
        "toss" => "throw",
        "speak" | "tell" | "talk" => "say",
        // Making is something you do, in words, like everything else.
        "make" | "build" | "craft" | "create" => "do",
        "hand" | "offer" => "give",
        "put_on" | "dress" | "don" => "wear",
        "mount" | "ride_on" | "climb_on" => "ride",
        "get_down" | "get_off" | "dismount" => "dismount",
        "take_off" | "undress" | "doff" => "take_off",
        "home" | "go_home" => "go_home",
        v => v,
    }
    .to_string();
    obj.insert("do".into(), Value::String(verb.clone()));
    // For do and use, `at` is a spot (a point); a plan never names one.
    if matches!(verb.as_str(), "do" | "use") && obj.get("at").is_some_and(|a| !a.is_array()) {
        obj.remove("at");
    }
    for key in ["target", "at", "to", "on", "with"] {
        if let Some(x) = obj.get(key).cloned() {
            let fixed = match x {
                Value::String(s) => Some(json!({ "name": s })),
                Value::Number(n) if key != "at" || verb != "place" => Some(json!({ "thing": n })),
                _ => None,
            };
            if let Some(f) = fixed {
                obj.insert(key.into(), f);
            }
        }
    }
    if verb == "say" && !obj.contains_key("text") {
        if let Some(x) = obj.remove("line").or_else(|| obj.remove("words")) {
            obj.insert("text".into(), x);
        }
    }
    if verb == "gesture" && !obj.contains_key("kind") {
        if let Some(x) = obj.remove("gesture").or_else(|| obj.remove("name")) {
            obj.insert("kind".into(), x);
        }
    }
    if verb == "do" && !obj.contains_key("text") {
        if let Some(x) = obj.remove("what").or_else(|| obj.remove("thing")) {
            obj.insert("text".into(), x);
        }
    }
    // {"do": "ask", "who": ["Rosa", "Ben"], "for": "a grill"}: names into targets.
    if verb == "ask" {
        let who = obj.remove("who").or_else(|| obj.remove("to")).or_else(|| obj.remove("target")).unwrap_or(Value::Null);
        let list: Vec<Value> = match who {
            Value::Array(v) => v.into_iter().filter_map(|x| match x {
                Value::String(s) => Some(json!({ "name": s })),
                Value::Object(_) => Some(x),
                _ => None,
            }).collect(),
            Value::String(s) => vec![json!({ "name": s })],
            o @ Value::Object(_) => vec![o],
            _ => vec![],
        };
        obj.insert("who".into(), Value::Array(list));
        if !obj.contains_key("what") && !obj.contains_key("for") {
            if let Some(x) = obj.remove("text").or_else(|| obj.remove("thing")) {
                obj.insert("what".into(), x);
            }
        }
    }
    // {"do": "create", "text": "a wooden ball"} → "make a wooden ball".
    if let (Some(Value::String(t)), true) = (obj.get("text").cloned(), matches!(raw.as_str(), "make" | "build" | "craft" | "create")) {
        if !t.trim_start().to_lowercase().starts_with(&raw) {
            obj.insert("text".into(), Value::String(format!("{} {}", if raw == "create" { "make" } else { &raw }, t.trim())));
        }
    }
    // Gestures named as verbs ("hug", "wave").
    if GestureKind::parse(&verb).is_some() && !matches!(verb.as_str(), "sit") {
        obj.insert("kind".into(), Value::String(verb.clone()));
        obj.insert("do".into(), Value::String("gesture".into()));
        if let Some(t) = obj.remove("target") {
            obj.insert("to".into(), t);
        }
    }
    serde_json::from_value(v).ok()
}

/// Short spoken lines for when no LLM speaks for them.
pub fn template(kind: &str, other: &str, hour: f32, k: u32) -> String {
    template_line(kind, other, hour, k)
}

fn template_line(kind: &str, other: &str, hour: f32, k: u32) -> String {
    let pick = |v: &[&str]| v[(k as usize) % v.len()].replace("{o}", other);
    match kind {
        "greet" => {
            if hour < 11.0 {
                pick(&["Morning, {o}.", "Good morning, {o}!", "Up early, {o}?"])
            } else if hour < 18.0 {
                pick(&["Hello, {o}.", "{o}! Good to see you.", "Afternoon, {o}."])
            } else {
                pick(&["Evening, {o}.", "Still about, {o}?", "Good evening."])
            }
        }
        "catch" => pick(&["Catch!", "Here it comes!", "Yours!", "Heads up!"]),
        "caught" => pick(&["Got it!", "Nice throw!", "Ha!", "Again!"]),
        "missed" => pick(&["Oops!", "Missed it!", "Too high!"]),
        "fire" => pick(&["Fire! Get back!", "It's burning!", "Look at that blaze…"]),
        "thanks" => pick(&["For me? Thank you.", "Thank you, {o}.", "Oh! That's kind."]),
        "hug" => pick(&["Come here, you.", "Missed you.", "Oh, {o}."]),
        "bored" => pick(&["Nothing ever happens here.", "Hm.", "What to do…"]),
        "cheer" => pick(&["Yes!", "Did you see that?", "Ha! In it goes!"]),
        "yes" => pick(&["Sure!", "Gladly.", "Why not!", "All right."]),
        "no" => pick(&["Not now.", "Maybe later.", "I'd rather not."]),
        _ => pick(&["Hm.", "Well then.", "Right."]),
    }
}

impl Sim {
    // ------------------------------------------------------------ actors

    pub fn step_actors(&mut self, dt: f32) {
        // The player: scripted plans and tasks (agents), pose.
        self.step_plan(ActorId::Player);
        self.run_task(ActorId::Player, dt);
        let held_big = self.held_size(ActorId::Player);
        let t = self.t;
        self.player.update_pose(t, dt, held_big);
        let near = self.cfg.near;
        let medium = self.cfg.medium;
        let hz = self.cfg.medium_hz;
        for i in 0..self.cast.npcs.len() {
            let (cid, d) = {
                let n = &self.cast.npcs[i];
                if n.dead {
                    continue;
                }
                (n.def.id, self.dist_to_player(n.a.pos))
            };
            if d <= near {
                self.npc_tick(cid, dt, true);
            } else if d <= medium {
                let n = &mut self.cast.npcs[i];
                n.acc += dt;
                if n.acc >= 1.0 / hz {
                    let step = n.acc.min(2.0);
                    n.acc = 0.0;
                    self.npc_tick(cid, step, false);
                }
            }
        }
    }

    fn held_size(&self, who: ActorId) -> Option<bool> {
        let id = self.actor(who)?.held?;
        let t = self.things.get(id)?;
        let ty = self.snap.type_of(t.type_id)?;
        Some(ty.radius() * t.scale > 0.35 || t.co_holder.is_some())
    }

    /// Give an actor a plan of actions (replacing any current one).
    pub fn plan(&mut self, who: ActorId, actions: Vec<Action>, goal: &str, from_llm: bool) {
        match who {
            ActorId::Player => {
                self.player_plan = actions.into();
            }
            ActorId::Npc(c) => {
                if let Some(n) = self.cast.get_mut(c) {
                    n.plan = actions.into();
                    n.goal = goal.to_string();
                    n.plan_from_llm = from_llm;
                    n.a.task = None;
                }
            }
        }
    }

    fn plan_mut(&mut self, who: ActorId) -> Option<&mut VecDeque<Action>> {
        match who {
            ActorId::Player => Some(&mut self.player_plan),
            ActorId::Npc(c) => self.cast.get_mut(c).map(|n| &mut n.plan),
        }
    }

    /// Carry out the next action of a plan when the current task is done.
    /// Returns false when a step failed.
    pub fn step_plan(&mut self, who: ActorId) -> bool {
        for _ in 0..3 {
            if self.actor(who).is_none_or(|a| a.task.is_some()) {
                return true;
            }
            let Some(next) = self.plan_mut(who).and_then(|p| p.pop_front()) else { return true };
            let label = format!("{:?}", next.verb());
            match self.act(who, next.clone()) {
                Ok(o) => {
                    if !o.ok {
                        self.plan_failed(who, &o.msg);
                        return false;
                    }
                }
                Err(ActErr::TooFar { at, dist }) => {
                    if dist > 120.0 {
                        self.plan_failed(who, &format!("{label}: too far ({dist:.0} m)"));
                        return false;
                    }
                    // Standing right under it: out of reach (up a bush, on a roof).
                    let me = self.actor(who).map(|a| a.pos).unwrap_or(at);
                    if Vec3::new(at.x - me.x, 0.0, at.z - me.z).length() < self.reach_of(who) * 0.6 + 0.3 {
                        self.plan_failed(who, &format!("{label}: out of reach"));
                        return false;
                    }
                    if let Some(p) = self.plan_mut(who) {
                        p.push_front(next);
                    }
                    let run = matches!(who, ActorId::Npc(_)) && dist > 25.0;
                    let deadline = self.t + 90.0;
                    let stop = self.reach_of(who) * 0.6;
                    self.set_task(who, Task::Goto { target: Target::Point(at.to_array()), stop, run, deadline });
                    return true;
                }
                Err(ActErr::Fail(msg)) => {
                    self.plan_failed(who, &msg);
                    return false;
                }
            }
        }
        true
    }

    fn plan_failed(&mut self, who: ActorId, why: &str) {
        let t = self.t;
        if let Some(p) = self.plan_mut(who) {
            p.clear();
        }
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get_mut(c) {
                n.log_decision(t, format!("plan failed: {why}"));
                n.goal.clear();
                n.think_at = t + 2.0;
            }
        }
        self.event("plan_failed", Some(who), None, why.to_string(), self.actor(who).map(|a| a.pos), json!({}));
    }

    /// Advance an actor's current task. True while it is still running.
    pub fn run_task(&mut self, who: ActorId, dt: f32) -> bool {
        let Some(task) = self.actor(who).and_then(|a| a.task.clone()) else { return false };
        // A long way to go: characters take their own mount if it is near.
        if let (ActorId::Npc(_), Task::Goto { target, .. }) = (who, &task) {
            if self.actor(who).is_some_and(|a| a.riding.is_none()) {
                let far = self.resolve(target, who).map(|r| r.pos).zip(self.actor(who).map(|a| a.pos)).is_some_and(|(p, m)| Vec3::new(p.x - m.x, 0.0, p.z - m.z).length() > 50.0);
                // Once per trip: a mount that says no isn't asked again every step.
                let asked = matches!(task, Task::Goto { deadline, .. } if self.interp.mount_asked.get(&who) == Some(&deadline.to_bits()));
                if far && !asked {
                    if let (Some(m), Task::Goto { deadline, .. }) = (self.own_mount(who, 15.0), &task) {
                        self.interp.mount_asked.insert(who, deadline.to_bits());
                        let _ = self.ride(who, m);
                    }
                }
            }
        }
        // What carries them: their own legs (or wings), or their mount.
        let mover = self.actor(who).and_then(|a| a.riding).unwrap_or(who);
        let (speed_walk, speed_run) = self.speeds(mover);
        let flyer = self.can_fly(mover);
        let fly = self.fly_speed(mover);
        let airborne = self.actor(mover).is_some_and(|a| a.alt > 0.3);
        let now = self.t;
        let me = self.actor(who).map(|a| a.pos).unwrap_or_default();
        let mut done = false;
        let mut failed = false;
        match &task {
            Task::Goto { target, stop, run, deadline } => {
                let dest = self.resolve(target, who).map(|r| r.pos);
                match dest {
                    None => failed = true,
                    Some(p) => {
                        let flat = Vec3::new(p.x - me.x, 0.0, p.z - me.z);
                        if flat.length() <= *stop {
                            if airborne {
                                // Come down first.
                                self.fly_toward(mover, p, fly, dt, true);
                            } else {
                                done = true;
                                if let Some(a) = self.actor_mut(who) {
                                    a.face(flat, 6.0);
                                }
                            }
                        } else if now > *deadline {
                            failed = true;
                        } else if flyer && (airborne || flat.length() > 20.0) {
                            self.fly_toward(mover, p, fly, dt, true);
                        } else {
                            let sp = if *run { speed_run.max(speed_walk) } else { speed_walk };
                            if !self.step_toward(mover, p, sp, dt) {
                                failed = true;
                            }
                        }
                    }
                }
            }
            Task::Move { dir, until } => {
                if now >= *until {
                    done = true;
                } else {
                    let p = me + *dir * 5.0;
                    self.step_toward(mover, p, speed_walk, dt);
                }
            }
            Task::Wait { until } => done = now >= *until,
            Task::Follow { who: other, dist, until } => {
                if now >= *until {
                    done = true;
                } else if let Some(p) = self.actor(*other).map(|a| a.pos) {
                    let d = (p - me).length();
                    if flyer && (airborne || d > 20.0) && d > *dist {
                        self.fly_toward(mover, p, fly, dt, true);
                    } else if d > *dist {
                        let sp = if d > 6.0 { speed_run.max(speed_walk) } else { speed_walk.max(1.6) };
                        self.step_toward(mover, p, sp, dt);
                    } else if let Some(a) = self.actor_mut(who) {
                        a.face(p - me, dt * 4.0);
                    }
                } else {
                    failed = true;
                }
            }
            Task::Face { target, until } => {
                if now >= *until {
                    done = true;
                } else if let Some(p) = self.resolve(target, who).map(|r| r.pos) {
                    if let Some(a) = self.actor_mut(who) {
                        a.face(p - me, dt * 4.0);
                    }
                }
            }
        }
        if done || failed {
            if let Some(a) = self.actor_mut(who) {
                if a.task.as_ref() == Some(&task) {
                    a.task = None;
                }
            }
            // Characters get down when they get there.
            if let (Task::Goto { .. }, ActorId::Npc(c)) = (&task, who) {
                if done && mover != who && self.cast.get(c).is_some_and(|n| !matches!(n.plan.front(), Some(Action::Goto { .. }))) {
                    let _ = self.dismount(who);
                }
            }
            if failed {
                if let ActorId::Npc(c) = who {
                    if let Some(n) = self.cast.get_mut(c) {
                        n.plan.clear();
                        n.think_at = now + 1.0;
                    }
                }
            }
        }
        !(done || failed)
    }

    /// Walking and running speeds (m/s): the traveller's stride, or the
    /// species' own.
    pub fn speeds(&self, who: ActorId) -> (f32, f32) {
        // Armour and loads slow a body down.
        let worn: f32 = self.things.live().filter(|t| t.worn == Some(who)).map(|t| t.mass()).sum();
        let k = (1.0 - worn / (self.strength(who) * 3.0)).clamp(0.5, 1.0);
        let (w, r) = self.base_speeds(who);
        (w * k, r * k)
    }

    fn base_speeds(&self, who: ActorId) -> (f32, f32) {
        match who {
            ActorId::Player => {
                let s = PLAYER_SPEED * (self.player.dims.height / 1.75).sqrt();
                (s, s)
            }
            ActorId::Npc(c) => match self.cast.get(c) {
                Some(n) if !n.species.is_human() => (n.species.moves.walk.max(0.2), n.species.moves.run.max(n.species.moves.walk)),
                _ => (NPC_WALK, NPC_RUN),
            },
        }
    }

    /// Walk towards a point. False when stuck.
    fn step_toward(&mut self, who: ActorId, p: Vec3, speed: f32, dt: f32) -> bool {
        let Some(me) = self.actor(who).map(|a| a.pos) else { return false };
        let d = Vec3::new(p.x - me.x, 0.0, p.z - me.z);
        let len = d.length();
        if len < 1e-3 {
            return true;
        }
        let dir = d / len;
        if let Some(a) = self.actor_mut(who) {
            a.face(dir, dt * 6.0);
            a.asleep = false;
        }
        let step = dir * (speed * dt).min(len);
        self.walk(who, step);
        self.kick_things(who, step);
        let moved = self.actor(who).map(|a| a.moved).unwrap_or(0.0);
        let blocked = moved < speed * dt * 0.15 && len >= 0.3;
        if let Some(a) = self.actor_mut(who) {
            a.phase += moved * 3.2;
            a.stuck = if blocked { a.stuck + dt } else { 0.0 };
            // Someone in the way for a moment is not a wall.
            a.stuck < 1.5
        } else {
            false
        }
    }

    // ------------------------------------------------------------ one character

    fn npc_tick(&mut self, cid: i64, dt: f32, near: bool) {
        let t = self.t;
        let night = self.night();
        let hour = self.hour();
        let player = self.player.pos;
        let Some(n) = self.cast.get_mut(cid) else { return };
        n.last_sim = t;
        // Needs drift, each at its species' pace.
        let tr = n.traits;
        let rate = n.species.needs;
        let sapient = n.species.mind == crate::world::species::Mind::Sapient;
        // People sleep at night; other species keep their own hours.
        let night = if n.species.is_human() { night } else { n.species.sleeps_at(hour) };
        n.needs.hunger = (n.needs.hunger + dt / 900.0 * rate.hunger).min(1.0);
        if n.a.asleep {
            n.needs.fatigue = (n.needs.fatigue - dt / 300.0).max(0.0);
        } else {
            n.needs.fatigue = (n.needs.fatigue + dt / 1100.0 * rate.fatigue).min(1.0);
        }
        n.needs.social = (n.needs.social + dt / 420.0 * (0.4 + tr.sociable) * rate.social).min(1.0);
        n.needs.fun = (n.needs.fun + dt / 520.0 * (0.4 + tr.playful) * rate.fun).min(1.0);
        n.needs.curiosity = (n.needs.curiosity - dt / 400.0 / rate.curiosity.max(0.1)).max(0.0);
        // Talking with the player: stand and face them.
        if self.talking_to == Some(cid) {
            if let Some(n) = self.cast.get_mut(cid) {
                n.a.task = None;
                n.a.asleep = false;
                n.a.face(player - n.a.pos, dt * 4.0);
                n.doing = "talking with the traveller".into();
                n.needs.social = (n.needs.social - dt / 60.0).max(0.0);
            }
            let held_big = self.held_size(ActorId::Npc(cid));
            if let Some(n) = self.cast.get_mut(cid) {
                n.a.update_pose(t, dt, held_big);
            }
            return;
        }
        // The player comes close.
        let Some(n) = self.cast.get_mut(cid) else { return };
        let dp = (n.a.pos - player).length();
        if dp < 9.0 && !n.a.asleep {
            if t - n.last_player_near > 240.0 {
                n.last_player_near = t;
                let ctx = format!("The traveller has come within a few metres of you. It is {}.", crate::render::sky::time_label(t));
                if self.has_llm && near && sapient {
                    let context = self.decide_context(cid, &ctx);
                    self.request(Request::Decide { cid, event: "player_near".into(), context }, player);
                }
            }
        }
        // Night: home and to bed.
        let Some(n) = self.cast.get_mut(cid) else { return };
        if night && !n.a.asleep && n.a.task.is_none() && n.mission.is_none() && !n.plan.iter().any(|a| matches!(a, Action::Sleep)) && !self.social.busy(ActorId::Npc(cid)) {
            let home = n.def.home;
            n.plan = vec![Action::GoHome, Action::Sleep].into();
            n.goal = "go home to sleep".into();
            n.doing = "heading home".into();
            if (n.a.pos - home).length() < 1.5 {
                n.plan = vec![Action::Sleep].into();
            }
        }
        if !night {
            if let Some(n) = self.cast.get_mut(cid) {
                if n.a.asleep {
                    n.a.asleep = false;
                    n.think_at = t + 1.0 + n.rand() as f64 * 3.0;
                    n.doing = "waking up".into();
                }
            }
        }
        if self.cast.get(cid).is_some_and(|n| n.restored) {
            self.after_load(cid);
        }
        self.need_tick(cid);
        let asleep = self.cast.get(cid).is_some_and(|n| n.a.asleep);
        if !asleep {
            let me = ActorId::Npc(cid);
            self.step_plan(me);
            self.run_task(me, dt);
            let idle = self.cast.get(cid).is_some_and(|n| n.a.task.is_none() && n.plan.is_empty());
            if idle && !self.social.busy(me) && t >= self.cast.get(cid).map(|n| n.think_at).unwrap_or(0.0) {
                self.think(cid);
            }
        }
        self.grow(cid);
        let held_big = self.held_size(ActorId::Npc(cid));
        let me = ActorId::Npc(cid);
        // A flyer with nothing to do comes down.
        if self.actor(me).is_some_and(|a| a.alt > 0.0 && a.task.is_none() && a.riding.is_none()) && self.rider_of(me).is_none() {
            self.settle(me, dt);
        }
        if let Some(n) = self.cast.get_mut(cid) {
            n.a.update_pose(t, dt, held_big);
            if n.a.moved < 1e-4 && n.a.alt <= 0.0 && n.a.riding.is_none() {
                n.a.pos.y = self.snap.terrain.height(n.a.pos.x, n.a.pos.z);
            }
        }
    }

    // ------------------------------------------------------------ choosing

    /// Pick what to do next from needs, surroundings and personality.
    fn think(&mut self, cid: i64) {
        let t = self.t;
        let Some(n) = self.cast.get(cid) else { return };
        let me = ActorId::Npc(cid);
        let pos = n.a.pos;
        let needs = n.needs;
        let tr = n.traits;
        let home = n.def.home;
        let held = n.a.held;
        let name = n.name().to_string();
        let next_think = |s: &mut Sim, secs: f64| {
            if let Some(n) = s.cast.get_mut(cid) {
                n.think_at = s.t + secs;
            }
        };
        // 0. What they wear is on fire: off with it.
        if self.mind_of(me) == crate::world::species::Mind::Sapient {
            if let Some(id) = self.worn_by(me).into_iter().find(|id| self.things.get(*id).is_some_and(|t| t.props[P_FIRE] > 0.05)) {
                let line = template_line("fire", "", self.hour(), self.cast.get_mut(cid).map(|n| n.rand() * 100.0).unwrap_or(0.0) as u32);
                self.say(me, &line, None);
                let mut steps = Vec::new();
                if held.is_some() {
                    steps.push(Action::Drop);
                }
                steps.push(Action::TakeOff { target: Some(Target::Thing(id)), from: None });
                steps.push(Action::Drop);
                self.plan(me, steps, "get the burning thing off", false);
                self.set_doing(cid, "tearing off something burning");
                next_think(self, 2.0);
                return;
            }
        }
        // 1. Fire close by: get away (the brave stay to watch).
        let fires: Vec<super::env::Burning> = self.burning().into_iter().filter(|f| !f.held).collect();
        if let Some(f) = fires.iter().filter(|f| (f.pos - pos).length() < 9.0 + f.size).min_by(|a, b| (a.pos - pos).length().total_cmp(&(b.pos - pos).length())) {
            let away = (pos - f.pos).normalize_or_zero();
            let dest = pos + if away == Vec3::ZERO { Vec3::X } else { away } * (12.0 + f.size * 2.0);
            let line = template_line("fire", "", self.hour(), self.cast.get_mut(cid).map(|n| n.rand() * 100.0).unwrap_or(0.0) as u32);
            self.say(me, &line, None);
            self.plan(me, vec![Action::Goto { target: Target::Point(dest.to_array()), run: true }], "get away from the fire", false);
            self.set_doing(cid, "fleeing the fire");
            next_think(self, 3.0);
            return;
        }
        if let Some(f) = fires.iter().filter(|f| (f.pos - pos).length() < 45.0).min_by(|a, b| (a.pos - pos).length().total_cmp(&(b.pos - pos).length())) {
            if tr.curious + tr.brave > 0.9 && needs.curiosity > 0.2 {
                let d = (f.pos - pos).length();
                let watch = f.pos + (pos - f.pos).normalize_or_zero() * (12.0 + f.size).min(d);
                self.plan(me, vec![Action::Goto { target: Target::Point(watch.to_array()), run: false }, Action::Wait { secs: 8.0 }], "watch the fire", false);
                self.set_doing(cid, "watching the fire");
                next_think(self, 6.0);
                return;
            }
        }
        if self.mind_of(me) != crate::world::species::Mind::Sapient {
            self.think_animal(cid);
            return;
        }
        // Something that would hunt them: get away (people too).
        if let Some((from, at)) = self.threat(me) {
            self.flee(cid, from, at);
            return;
        }
        // Candidate scores.
        let mut best: (f32, &str) = (0.15 + 0.1 * self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0), "wander");
        fn consider(best: &mut (f32, &'static str), score: f32, what: &'static str) {
            if score > best.0 {
                *best = (score, what);
            }
        }
        let food = self.nearest_matching(pos, 30.0, |p| p[P_EDIBLE] > 0.05 && p[P_MASS] < 5.0);
        if needs.hunger > 0.5 {
            consider(&mut best, needs.hunger * if food.is_some() || held.is_some_and(|h| self.things.get(h).is_some_and(|x| x.props[P_EDIBLE] > 0.0)) { 1.1 } else { 0.3 }, "eat");
        }
        let friend = self.best_company(cid, 35.0);
        if let Some((_, aff)) = friend {
            consider(&mut best, needs.social * (0.5 + tr.sociable) * (0.6 + aff.max(0.0)), "socialize");
        }
        let ball = self.nearest_matching(pos, 30.0, |p| p[P_BOUNCE] >= 0.45 && p[P_MASS] <= 3.0);
        if ball.is_some() || held.is_some_and(|h| self.things.get(h).is_some_and(|x| x.props[P_BOUNCE] >= 0.45 && x.mass() <= 3.0)) {
            consider(&mut best, needs.fun * (0.4 + tr.playful) * 1.2, "play");
        }
        let novelty = self.novelty_near(cid, pos, 40.0);
        if novelty.is_some() {
            consider(&mut best, needs.curiosity * (0.5 + tr.curious) * 1.3, "look");
        }
        if needs.fatigue > 0.7 && !self.night() {
            consider(&mut best, needs.fatigue * 0.8, "rest");
        }
        // Loose small things lying about (sticks, stones): bring some home, or toss one.
        let loose = if held.is_none() { self.loose_thing(pos, home, 14.0) } else { None };
        if loose.is_some() {
            let pile = self.things.near(home, 4.0).len() as f32;
            if (pos - home).length() > 6.0 {
                consider(&mut best, 0.18 + tr.crafty * 0.35 - pile * 0.04, "gather");
            }
            if ball.is_none() {
                consider(&mut best, needs.fun * tr.playful * 0.75, "toss");
            }
        }
        // Their craft: make, fix or improve something nearby. Rare, and only
        // when nothing is already being built, since each piece of work may
        // add a type to the world.
        let can_work = self.has_llm
            && self.interp.building.is_empty()
            && t - self.cast.last_work > self.cfg.work_gap_secs as f64
            && self.cast.get(cid).is_some_and(|n| t - n.last_work > self.cfg.work_secs as f64 && t >= n.next_llm);
        if can_work {
            consider(&mut best, 0.1 + tr.crafty * 0.45, "work");
        }
        if needs.fun > 0.85 && best.1 == "wander" {
            consider(&mut best, 0.5, "bored");
        }
        let choice = best.1;
        let hour = self.hour();
        let k = self.cast.get_mut(cid).map(|n| crate::noise::pcg(n.rng)).unwrap_or(0);
        match choice {
            "eat" => {
                if let Some(h) = held.filter(|h| self.things.get(*h).is_some_and(|x| x.props[P_EDIBLE] > 0.0)) {
                    self.plan(me, vec![Action::Eat { target: Some(Target::Thing(h)) }], "eat", false);
                } else if let Some((tg, _)) = food {
                    let mut steps = Vec::new();
                    if held.is_some() {
                        steps.push(Action::Drop);
                    }
                    steps.push(Action::Eat { target: Some(tg) });
                    self.plan(me, steps, "find something to eat", false);
                } else {
                    self.ask(cid, "hungry", "You are hungry and see nothing to eat nearby.");
                    self.wander(cid, home, 25.0);
                }
                self.set_doing(cid, "looking for food");
                next_think(self, 4.0);
            }
            "socialize" => {
                let Some((other, aff)) = friend else { return };
                let oname = self.actor_name(other);
                let mut steps = vec![Action::Goto { target: Target::Actor(other), run: false }];
                if other == ActorId::Player {
                    let first = oname.split_whitespace().next().unwrap_or("").to_string();
                    steps.push(Action::Say { text: template_line("greet", if first == "the" { "traveller" } else { &first }, hour, k), to: Some(Target::Actor(other)) });
                }
                let hug_ok = aff > 0.6 && other != ActorId::Player && self.social.rel(me, other).is_some_and(|r| r.family || r.partner || r.affection > 0.7);
                if hug_ok && k % 3 == 0 {
                    steps.push(Action::Gesture { kind: "hug".into(), to: Some(Target::Actor(other)) });
                } else {
                    steps.push(Action::Gesture { kind: "wave".into(), to: Some(Target::Actor(other)) });
                }
                self.plan(me, steps, &format!("spend time with {oname}"), false);
                if let ActorId::Npc(o) = other {
                    self.social.want_chat(cid, o, t);
                }
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.social = (n.needs.social - 0.35).max(0.0);
                }
                self.set_doing(cid, &format!("seeking out {oname}"));
                next_think(self, 8.0);
            }
            "play" => {
                let ball_id = held.filter(|h| self.things.get(*h).is_some_and(|x| x.props[P_BOUNCE] >= 0.45)).or_else(|| ball.as_ref().and_then(|(tg, _)| self.liven(tg)));
                let Some(ball_id) = ball_id else { return };
                let partner = self.best_company(cid, 25.0).map(|x| x.0);
                if let Some(p) = partner {
                    if self.propose(me, p, "catch", Some(ball_id)).is_ok() {
                        self.set_doing(cid, "suggesting a game of catch");
                        next_think(self, 6.0);
                        return;
                    }
                }
                // Alone: throw it at something that looks like it is for throwing at, or up in the air.
                let mark = self.throwing_mark(pos, 30.0);
                let mut steps = Vec::new();
                if held != Some(ball_id) {
                    if held.is_some() {
                        steps.push(Action::Drop);
                    }
                    steps.push(Action::Hold { target: Target::Thing(ball_id) });
                }
                match mark {
                    Some((mt, mp)) => {
                        let stand = mp + (pos - mp).normalize_or_zero() * 5.0;
                        steps.push(Action::Goto { target: Target::Point(stand.to_array()), run: false });
                        steps.push(Action::Throw { at: Some(mt), dir: None, force: None });
                        self.set_doing(cid, "throwing at a target");
                    }
                    None => {
                        steps.push(Action::Throw { at: None, dir: Some([0.1, 1.0, 0.1]), force: Some(6.0) });
                        self.set_doing(cid, "tossing a ball");
                    }
                }
                self.plan(me, steps, "play", false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.fun = (n.needs.fun - 0.2).max(0.0);
                    n.a.catching = t + 4.0;
                }
                next_think(self, 5.0);
            }
            "look" => {
                let Some((tg, p, what)) = novelty else { return };
                let stand = p + (pos - p).normalize_or_zero() * 3.0;
                self.plan(me, vec![Action::Goto { target: Target::Point(stand.to_array()), run: false }, Action::Wait { secs: 5.0 }], &format!("look at the {what}"), false);
                let _ = tg;
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.curiosity = (n.needs.curiosity - 0.4).max(0.0);
                }
                self.social.seen_novelty(cid, &what);
                self.set_doing(cid, &format!("looking at the {what}"));
                next_think(self, 6.0);
            }
            "gather" => {
                let Some((tg, _)) = loose else { return };
                let n = self.things.near(home, 4.0).len() as f32;
                let a = n * 1.1 + cid as f32;
                let spot = home + Vec3::new(a.cos(), 0.0, a.sin()) * (1.4 + (n * 0.15).min(1.5));
                let spot = Vec3::new(spot.x, self.snap.terrain.height(spot.x, spot.z), spot.z);
                self.plan(me, vec![Action::Hold { target: tg }, Action::Goto { target: Target::Point(spot.to_array()), run: false }, Action::Place { at: spot.to_array() }], "bring it home", false);
                self.set_doing(cid, "gathering things to bring home");
                next_think(self, 8.0);
            }
            "toss" => {
                let Some((tg, _)) = loose else { return };
                let throw = match self.throwing_mark(pos, 25.0) {
                    Some((mt, _)) => Action::Throw { at: Some(mt), dir: None, force: None },
                    None => {
                        let a = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.0) * std::f32::consts::TAU;
                        Action::Throw { at: None, dir: Some([a.cos(), 0.6, a.sin()]), force: Some(7.0 + 5.0 * (a.sin() * 0.5 + 0.5)) }
                    }
                };
                self.plan(me, vec![Action::Hold { target: tg }, throw], "throw something for fun", false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.fun = (n.needs.fun - 0.12).max(0.0);
                }
                self.set_doing(cid, "throwing stones");
                next_think(self, 5.0);
            }
            "rest" => {
                self.plan(me, vec![Action::Gesture { kind: "sit".into(), to: None }, Action::Wait { secs: 15.0 }], "rest", false);
                if let Some(n) = self.cast.get_mut(cid) {
                    n.needs.fatigue = (n.needs.fatigue - 0.25).max(0.0);
                }
                self.set_doing(cid, "resting");
                next_think(self, 16.0);
            }
            "work" => {
                self.cast.last_work = t;
                if let Some(n) = self.cast.get_mut(cid) {
                    n.last_work = t;
                }
                self.ask(cid, "work", "You have a little time for your craft or daily work. Look at what is around you: is there something you could make, fix or improve that fits who you are? If so, do it (a \"do\" step, in words). If nothing fits, carry on as you were.");
                self.set_doing(cid, "thinking about work");
                next_think(self, 8.0);
            }
            "bored" => {
                let since = self.cast.get(cid).map(|n| n.bored_since).unwrap_or(f64::MAX);
                if since == f64::MAX {
                    if let Some(n) = self.cast.get_mut(cid) {
                        n.bored_since = t;
                    }
                } else if t - since > 45.0 {
                    if let Some(n) = self.cast.get_mut(cid) {
                        n.bored_since = f64::MAX;
                    }
                    self.ask(cid, "bored", "You are bored: nothing to play with and nobody around to play with. You could make something, fix or improve something nearby, find someone, or go somewhere.");
                }
                self.wander(cid, home, 30.0);
                self.set_doing(cid, "at a loose end");
                next_think(self, 6.0);
            }
            _ => {
                self.wander(cid, home, 17.0);
                self.set_doing(cid, "pottering about");
                let r = self.cast.get_mut(cid).map(|n| n.rand()).unwrap_or(0.5);
                next_think(self, 3.0 + r as f64 * 6.0);
            }
        }
        let _ = name;
    }

    pub fn set_doing(&mut self, cid: i64, what: &str) {
        if let Some(n) = self.cast.get_mut(cid) {
            n.doing = what.to_string();
        }
    }

    /// Walk somewhere on dry land near home.
    pub(super) fn wander(&mut self, cid: i64, home: Vec3, radius: f32) {
        for _ in 0..6 {
            let (a, r) = match self.cast.get_mut(cid) {
                Some(n) => (n.rand() * std::f32::consts::TAU, 3.0 + n.rand() * radius),
                None => return,
            };
            let p = home + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
            if self.snap.terrain.height(p.x, p.z) > crate::terrain::WATER_LEVEL + 0.3 {
                self.plan(ActorId::Npc(cid), vec![Action::Goto { target: Target::Point([p.x, 0.0, p.z]), run: false }], "", false);
                return;
            }
        }
    }

    /// The most liked awake person nearby (and how much they are liked).
    pub fn best_company(&self, cid: i64, range: f32) -> Option<(ActorId, f32)> {
        let me = ActorId::Npc(cid);
        let pos = self.cast.get(cid)?.a.pos;
        let mut best: Option<(f32, ActorId, f32)> = None;
        for o in self.actor_ids() {
            if o == me {
                continue;
            }
            let Some(a) = self.actor(o) else { continue };
            let d = (a.pos - pos).length();
            if d > range || a.asleep {
                continue;
            }
            if o == ActorId::Player && self.talking_to.is_some() {
                continue;
            }
            let aff = self.social.rel(me, o).map(|r| r.affection - r.rivalry).unwrap_or(0.0);
            if aff < -0.2 || self.social.busy(o) {
                continue;
            }
            let score = aff + 0.3 - d / range * 0.3;
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, o, aff));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// The nearest live thing or scatter item whose properties match.
    pub fn nearest_matching(&mut self, p: Vec3, range: f32, f: impl Fn(&[f32]) -> bool) -> Option<(Target, Vec3)> {
        let mut best: Option<(f32, Target, Vec3)> = None;
        for id in self.things.near(p, range) {
            let Some(t) = self.things.get(id) else { continue };
            if t.held() || t.anchored {
                continue;
            }
            if !f(&t.props) {
                continue;
            }
            let d = (t.pos - p).length();
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Thing(id), t.pos));
            }
        }
        let snap = self.snap.clone();
        for it in self.cache.items_near(&snap, p, range.min(20.0)) {
            let Some(ty) = snap.type_of(it.inst.info[0]) else { continue };
            let mut base = (*self.type_props.get(&self.vocab, ty)).clone();
            if let Some(c) = self.field.cells.get(&it.cell) {
                base = c.props.clone();
            }
            if !f(&base) {
                continue;
            }
            let d = (it.inst.pos() - p).length();
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Cell([it.cell.0, it.cell.1]), it.inst.pos()));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// A small loose thing worth picking up (not food, not alive, not already
    /// in someone's pile at home).
    fn loose_thing(&mut self, p: Vec3, home: Vec3, range: f32) -> Option<(Target, Vec3)> {
        let homes: Vec<Vec3> = self.cast.npcs.iter().map(|n| n.def.home).collect();
        let near_a_home = |q: Vec3| homes.iter().any(|h| (*h - q).length() < 5.0);
        let mut best: Option<(f32, Target, Vec3)> = None;
        for id in self.things.near(p, range) {
            let Some(t) = self.things.get(id) else { continue };
            if t.held() || t.anchored || t.mass() > 3.0 || t.props[P_EDIBLE] > 0.0 || t.props[P_FIRE] > 0.0 || near_a_home(t.pos) {
                continue;
            }
            let d = (t.pos - p).length();
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Thing(id), t.pos));
            }
        }
        let snap = self.snap.clone();
        for it in self.cache.items_near(&snap, p, range) {
            let Some(ty) = snap.type_of(it.inst.info[0]) else { continue };
            if !(ty.has_tag("stick") || ty.has_tag("stone")) || near_a_home(it.inst.pos()) || self.field.cells.contains_key(&it.cell) {
                continue;
            }
            let d = (it.inst.pos() - p).length();
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Cell([it.cell.0, it.cell.1]), it.inst.pos()));
            }
        }
        let _ = home;
        best.map(|b| (b.1, b.2))
    }

    /// Something made recently that this character hasn't looked at yet.
    fn novelty_near(&self, cid: i64, p: Vec3, range: f32) -> Option<(Target, Vec3, String)> {
        let mut best: Option<(f32, Target, Vec3, String)> = None;
        for t in self.things.live() {
            if self.t - t.born > 600.0 || t.held() {
                continue;
            }
            let d = (t.pos - p).length();
            if d > range {
                continue;
            }
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            if ty.builtin || self.social.has_seen(cid, ty.name()) {
                continue;
            }
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Thing(t.id), t.pos, ty.name().to_string()));
            }
        }
        best.map(|b| (b.1, b.2, b.3))
    }

    /// Something that looks made for throwing things at (a hoop, a goal, a
    /// target, a basket, a bell…), by its tags or name.
    fn throwing_mark(&self, p: Vec3, range: f32) -> Option<(Target, Vec3)> {
        const WORDS: &[&str] = &["hoop", "goal", "target", "basket", "net", "ring", "bell", "bucket", "bin", "barrel"];
        let fits = |ty: &TypeEntry| WORDS.iter().any(|w| ty.has_tag(w) || ty.name().to_lowercase().contains(w));
        let mut best: Option<(f32, Target, Vec3)> = None;
        for pl in &self.snap.instances {
            let d = (pl.pos - p).length();
            if d > range {
                continue;
            }
            let Some(ty) = self.snap.type_of(pl.type_id) else { continue };
            if fits(ty) && best.as_ref().is_none_or(|b| d < b.0) {
                let target = match self.things.by_instance.get(&pl.id) {
                    Some(t) => Target::Thing(*t),
                    None => Target::Instance(pl.id),
                };
                best = Some((d, target, pl.pos));
            }
        }
        for t in self.things.live() {
            let d = (t.pos - p).length();
            if d > range || t.held() {
                continue;
            }
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            if fits(ty) && best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, Target::Thing(t.id), t.pos));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    // ------------------------------------------------------------ the LLM

    /// Ask the planner about an event (rate-limited per character).
    pub fn ask(&mut self, cid: i64, event: &str, what: &str) {
        self.ask_planner(cid, event, what, false);
    }

    /// The traveller said something to a character and they answered: they
    /// may now do what was asked (make it and hand it over, show the way…).
    pub fn asked(&mut self, cid: i64, said: &str, replied: &str) {
        let what = format!(
            "The traveller just said to you: \"{said}\". You answered: \"{replied}\". If they asked you to do, make, fetch, give or show something and you agreed, do it now: to make something for them, a \"do\" step that makes it, then a \"give\" step to the traveller (it is handed over once made). If you refused, or nothing was asked, reply with no steps. Set \"say\" to null: you have already answered."
        );
        self.ask_planner(cid, "asked", &what, true);
    }

    /// Ask the planner what to do about an event. Returns whether it was asked.
    fn ask_planner(&mut self, cid: i64, event: &str, what: &str, now: bool) -> bool {
        if !self.has_llm || self.mind_of(ActorId::Npc(cid)) != crate::world::species::Mind::Sapient {
            return false;
        }
        let t = self.t;
        let Some(n) = self.cast.get_mut(cid) else { return false };
        if t < n.next_llm && !now {
            return false;
        }
        n.next_llm = t + 60.0;
        let pos = n.a.pos;
        if self.dist_to_player(pos) > self.cfg.near && !self.cfg.medium_llm {
            return false;
        }
        let context = self.decide_context(cid, what);
        self.request(Request::Decide { cid, event: event.into(), context }, pos);
        true
    }

    pub fn ask_planner_now(&mut self, cid: i64, event: &str, what: &str) -> bool {
        self.ask_planner(cid, event, what, true)
    }

    /// What a character knows right now, for the planner.
    pub fn decide_context(&mut self, cid: i64, what: &str) -> String {
        let Some(n) = self.cast.get(cid) else { return what.to_string() };
        let me = ActorId::Npc(cid);
        let pos = n.a.pos;
        let held = n.a.held.map(|h| self.thing_name(h));
        let needs = n.needs;
        let goal = n.goal.clone();
        let mut things = Vec::new();
        for id in self.things.near(pos, 25.0).into_iter().take(12) {
            let Some(t) = self.things.get(id) else { continue };
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            let mut notes = Vec::new();
            for (i, label) in [(P_BOUNCE, "bouncy"), (P_EDIBLE, "edible"), (P_FIRE, "on fire"), (P_LIGHT, "gives light"), (P_WET, "wet"), (P_FRAGILE, "fragile")] {
                if t.props[i] > 0.4 {
                    notes.push(label);
                }
            }
            if t.mass() > STRENGTH && !t.anchored {
                notes.push("needs two to carry");
            }
            let by = t.origin.made_by.clone().map(|m| format!(", made by {m}")).unwrap_or_default();
            things.push(format!("{} ({:.0} m{}{}{})", ty.name(), (t.pos - pos).length(), if notes.is_empty() { "" } else { ", " }, notes.join(", "), by));
        }
        for pl in &self.snap.instances {
            let d = (pl.pos - pos).length();
            if d < 30.0 && !self.things.by_instance.contains_key(&pl.id) {
                if let Some(ty) = self.snap.type_of(pl.type_id) {
                    things.push(format!("{} ({d:.0} m)", ty.name()));
                }
            }
        }
        things.truncate(16);
        let mut people = Vec::new();
        for o in self.actor_ids() {
            if o == me {
                continue;
            }
            let Some(a) = self.actor(o) else { continue };
            let d = (a.pos - pos).length();
            if d > 40.0 {
                continue;
            }
            let rel = self.social.rel(me, o).map(|r| r.describe()).unwrap_or_else(|| "a stranger".into());
            let doing = match o {
                ActorId::Npc(c) => self.cast.get(c).map(|x| x.doing.clone()).unwrap_or_default(),
                ActorId::Player => "the traveller".into(),
            };
            people.push(format!("{} ({d:.0} m, {rel}{}{})", self.actor_name(o), if doing.is_empty() { "" } else { ", " }, doing));
        }
        let recent: Vec<String> = self.log.recent.iter().rev().filter(|e| e.pos.is_some_and(|p| (Vec3::from(p) - pos).length() < 40.0) && self.t - e.t < 300.0).take(6).map(|e| e.text.clone()).collect();
        format!(
            "{what}\nIt is {}. You hold: {}. Your current goal: {}.\nYou feel: hunger {:.1}, tiredness {:.1}, loneliness {:.1}, boredom {:.1}, curiosity {:.1} (0 = fine, 1 = urgent).\nThings around you: {}.\nPeople around you: {}.\nRecently near you: {}.",
            crate::render::sky::time_label(self.t),
            held.unwrap_or_else(|| "nothing".into()),
            if goal.is_empty() { "none".into() } else { goal },
            needs.hunger,
            needs.fatigue,
            needs.social,
            needs.fun,
            needs.curiosity,
            if things.is_empty() { "nothing in particular".into() } else { things.join("; ") },
            if people.is_empty() { "nobody".into() } else { people.join("; ") },
            if recent.is_empty() { "nothing".into() } else { recent.join("; ") },
        )
    }

    /// A planner decision arrived.
    pub fn on_decision(&mut self, cid: i64, d: Decision) {
        let me = ActorId::Npc(cid);
        let t = self.t;
        let night = self.night();
        let Some(n) = self.cast.get_mut(cid) else { return };
        if n.a.asleep && night {
            return;
        }
        let summary = d.goal.clone().unwrap_or_else(|| d.action.clone());
        n.log_decision(t, format!("decided: {summary}"));
        let mut steps: Vec<Action> = d.steps.iter().filter_map(parse_step).take(10).collect();
        let said = d.say.as_ref().is_some_and(|s| !s.trim().is_empty());
        let mut steps = self.need_decision(cid, std::mem::take(&mut steps), said);
        // Making something and then giving it: the thing takes a while to be
        // made, so the giving waits until it is in their hands.
        if let Some(i) = steps.iter().position(|a| matches!(a, Action::Do { .. })) {
            if let Some(j) = steps.iter().skip(i + 1).position(|a| matches!(a, Action::Give { .. })).map(|j| j + i + 1) {
                if let Action::Give { to } = steps.remove(j) {
                    if let Some(Target::Actor(other)) = self.resolve(&to, me).map(|r| r.target) {
                        if let Some(n) = self.cast.get_mut(cid) {
                            n.deliver = Some((other, t + 300.0));
                        }
                    }
                }
            }
        }
        let line = d.say.clone().or(d.line.clone()).filter(|l| !l.trim().is_empty());
        if steps.is_empty() {
            match d.action.as_str() {
                "approach" | "approach_player" | "greet" => {
                    steps.push(Action::Goto { target: Target::Actor(ActorId::Player), run: false });
                    if let Some(l) = &line {
                        steps.push(Action::Say { text: l.clone(), to: Some(Target::Actor(ActorId::Player)) });
                    }
                    steps.push(Action::Follow { target: Target::Actor(ActorId::Player), secs: Some(0.1) });
                }
                "watch" | "look" => {
                    let until = t + 8.0;
                    if let Some(n) = self.cast.get_mut(cid) {
                        n.a.task = Some(Task::Face { target: Target::Actor(ActorId::Player), until });
                    }
                }
                "go_home" | "home" | "sleep" => steps.push(Action::GoHome),
                _ => {}
            }
        } else if let Some(l) = &line {
            self.say(me, l, None);
        }
        if !steps.is_empty() {
            let goal = d.goal.clone().unwrap_or_else(|| d.action.clone());
            self.plan(me, steps, &goal, true);
            self.set_doing(cid, &goal);
            let pos = self.cast.get(cid).map(|n| n.a.pos);
            self.event("decided", Some(me), None, format!("{} decided to {goal}", self.actor_name(me)), pos, json!({ "steps": d.steps }));
        }
    }

    /// The player spoke to a character outside the talk screen (agents).
    pub fn player_talks(&mut self, cid: i64, text: &str) {
        if !self.speaks(ActorId::Npc(cid)) {
            self.animal_answers(cid);
            return;
        }
        let context = self.decide_context(cid, "The traveller is talking to you.");
        self.request_now(Request::Talk { cid, text: text.to_string(), context });
    }

    /// A line of dialogue arrived (from the brain) for a character.
    pub fn npc_said(&mut self, cid: i64, text: &str) {
        let me = ActorId::Npc(cid);
        let Some(pos) = self.actor(me).map(|a| a.pos) else { return };
        let name = self.actor_name(me);
        self.event("said", Some(me), Some("player".into()), format!("{name} said: \"{text}\""), Some(pos), json!({ "text": text }));
    }

    /// Notice gestures and ask the planner about big ones.
    pub fn npc_note_event(&mut self, cid: i64, what: &str) {
        let t = self.t;
        if let Some(n) = self.cast.get_mut(cid) {
            n.log_decision(t, what.to_string());
        }
    }

    pub fn say_template(&mut self, who: ActorId, kind: &str, other: &str) {
        let k = match who {
            ActorId::Npc(c) => self.cast.get_mut(c).map(|n| {
                n.rng = crate::noise::pcg(n.rng);
                n.rng
            }),
            _ => None,
        }
        .unwrap_or(0);
        let t = self.t;
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get_mut(c) {
                if t - n.last_line < 3.0 {
                    return;
                }
                n.last_line = t;
            }
        }
        let line = template_line(kind, other, self.hour(), k);
        self.say(who, &line, None);
    }
}
