//! The shared action set. The player (keys and mouse), every character (its
//! mind and the plans the LLM writes) and the test agent (`pocket act`) all
//! act through `Sim::act`, so anything a character can do the agent can do,
//! and test, the same way.

use super::actor::{GestureKind, GestureRun, Task};
use super::props::*;
use super::persist;
use super::things::{Origin, Thing, ThingId};
use super::render::thing_inst;
use super::{ActorId, Note, Request, Sim, Target, article};
use crate::render::GpuInst;
use crate::world::TypeEntry;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use serde_json::json;

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Action {
    /// Walk in a world direction (x, z) for `secs`.
    Move {
        dir: [f32; 2],
        #[serde(default = "one")]
        secs: f32,
    },
    /// Face a compass heading in degrees (0 = +z, 90 = +x).
    Turn { deg: f32 },
    Goto {
        target: Target,
        #[serde(default)]
        run: bool,
    },
    Hold { target: Target },
    Drop,
    /// Set the held thing down at a point within reach.
    Place { at: [f32; 3] },
    Throw {
        #[serde(default)]
        at: Option<Target>,
        #[serde(default)]
        dir: Option<[f32; 3]>,
        #[serde(default)]
        force: Option<f32>,
    },
    /// Use a thing (default: the held one), optionally on something.
    Use {
        #[serde(default)]
        target: Option<Target>,
        #[serde(default)]
        on: Option<Target>,
        /// The point on the thing that was touched (world), if known.
        #[serde(default)]
        at: Option<[f32; 3]>,
    },
    Eat {
        #[serde(default)]
        target: Option<Target>,
    },
    /// Anything else, in words: interpreted by the world.
    Do {
        text: String,
        #[serde(default)]
        on: Option<Target>,
        /// The point on the thing that was touched (world), if known.
        #[serde(default)]
        at: Option<[f32; 3]>,
    },
    Create { text: String },
    Say {
        text: String,
        #[serde(default)]
        to: Option<Target>,
    },
    Gesture {
        kind: String,
        #[serde(default)]
        to: Option<Target>,
    },
    /// Suggest doing something together ("catch", "carry", "dance", "walk", a hug…).
    Propose {
        to: Target,
        activity: String,
        #[serde(default)]
        with: Option<Target>,
    },
    /// Say yes or no to a proposal (the latest one, or the one from `to`).
    Answer {
        yes: bool,
        #[serde(default)]
        to: Option<Target>,
    },
    /// Hand the held thing to someone.
    Give { to: Target },
    /// Put on a layer (clothing, armour, a collar, a saddle): the held one or
    /// `target`, on yourself or on someone (`on`), who must agree.
    Wear {
        #[serde(default)]
        target: Option<Target>,
        #[serde(default)]
        on: Option<Target>,
    },
    /// Climb onto someone who can carry you (a horse, a griffin), if they agree.
    Ride { target: Target },
    /// Get down from what you ride.
    Dismount,
    /// Take a layer off yourself or someone (`from`), into your hands.
    TakeOff {
        #[serde(default)]
        target: Option<Target>,
        #[serde(default)]
        from: Option<Target>,
    },
    Follow {
        target: Target,
        #[serde(default)]
        secs: Option<f32>,
    },
    Wait {
        #[serde(default = "one")]
        secs: f32,
    },
    Sleep,
    Wake,
    GoHome,
    /// Ask people, best first, to make or give something; one at a time until one agrees.
    Ask {
        #[serde(default)]
        who: Vec<Target>,
        #[serde(default, alias = "for")]
        what: String,
    },
    /// Put the ask to the person stood by (the second half of `Ask`).
    Plea { to: Target },
    /// Wait until the hour of the day comes round.
    WaitUntil { hour: f32 },
    /// Go back to the deed a need held up.
    Resume {
        #[serde(default)]
        on: Option<Target>,
    },
}

impl Action {
    pub fn verb(&self) -> &'static str {
        match self {
            Action::Move { .. } => "move",
            Action::Turn { .. } => "turn",
            Action::Goto { .. } => "goto",
            Action::Hold { .. } => "hold",
            Action::Drop => "drop",
            Action::Place { .. } => "place",
            Action::Throw { .. } => "throw",
            Action::Use { .. } => "use",
            Action::Eat { .. } => "eat",
            Action::Do { .. } => "do",
            Action::Create { .. } => "create",
            Action::Say { .. } => "say",
            Action::Gesture { .. } => "gesture",
            Action::Propose { .. } => "propose",
            Action::Answer { .. } => "answer",
            Action::Give { .. } => "give",
            Action::Wear { .. } => "wear",
            Action::Ride { .. } => "ride",
            Action::Dismount => "dismount",
            Action::TakeOff { .. } => "take_off",
            Action::Follow { .. } => "follow",
            Action::Wait { .. } => "wait",
            Action::Sleep => "sleep",
            Action::Wake => "wake",
            Action::GoHome => "go_home",
            Action::Ask { .. } => "ask",
            Action::Plea { .. } => "plea",
            Action::WaitUntil { .. } => "wait_until",
            Action::Resume { .. } => "resume",
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Outcome {
    pub ok: bool,
    pub msg: String,
    /// An LLM request id when the result arrives later.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thing: Option<ThingId>,
}

impl Outcome {
    pub fn ok(msg: impl Into<String>) -> Outcome {
        Outcome { ok: true, msg: msg.into(), pending: None, thing: None }
    }
    fn thing(mut self, id: ThingId) -> Outcome {
        self.thing = Some(id);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActErr {
    /// Walk closer to `at` and try again.
    TooFar { at: Vec3, dist: f32 },
    Fail(String),
}

impl std::fmt::Display for ActErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActErr::TooFar { dist, .. } => write!(f, "too far away ({dist:.1} m); go closer first"),
            ActErr::Fail(s) => write!(f, "{s}"),
        }
    }
}

fn fail<T>(s: impl Into<String>) -> Result<T, ActErr> {
    Err(ActErr::Fail(s.into()))
}

/// A resolved target: what it is and where.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub target: Target,
    pub pos: Vec3,
    pub name: String,
    /// Its placed shape and local bounding box (lo, hi), for reach.
    pub bounds: Option<(GpuInst, Vec3, Vec3)>,
}

impl Resolved {
    fn new(target: Target, pos: Vec3, name: String) -> Self {
        Resolved { target, pos, name, bounds: None }
    }

    fn boxed(target: Target, pos: Vec3, name: String, inst: GpuInst, ty: &TypeEntry) -> Self {
        let b = ty.ct.meta.bounds;
        let bounds = Some((inst, Vec3::new(-b[0], ty.bottom, -b[2]), Vec3::new(b[0], ty.top.max(ty.bottom), b[2])));
        Resolved { target, pos, name, bounds }
    }

    /// The point of it nearest `from`: on its bounding box, so a big
    /// building is in reach from beside its wall, not just its middle.
    pub fn nearest(&self, from: Vec3) -> Vec3 {
        match &self.bounds {
            Some((inst, lo, hi)) => inst.from_local(Vec3::from(inst.to_local(from)).clamp(*lo, *hi)),
            None => self.pos,
        }
    }
}

impl Sim {
    // ------------------------------------------------------------ targets

    /// Ok if `who` can reach some part of it; else how far its nearest part is.
    fn reach(&self, who: ActorId, r: &Resolved) -> Result<(), ActErr> {
        let me = self.actor(who).map(|a| a.pos).unwrap_or(r.pos);
        let p = r.nearest(me);
        if self.in_reach(who, p) {
            return Ok(());
        }
        // Walk towards its middle (characters' plans judge "out of reach"
        // from there); the distance told is to its nearest part.
        Err(ActErr::TooFar { at: r.pos, dist: (p - me).length() })
    }

    /// Make a target concrete: names become the nearest match, and the result
    /// carries a position and a display name.
    pub fn resolve(&mut self, t: &Target, from: ActorId) -> Option<Resolved> {
        let origin = self.actor(from).map(|a| a.pos).unwrap_or(self.player.pos);
        match t {
            Target::Thing(id) => {
                let th = self.things.get(*id)?;
                let ty = self.snap.type_of(th.type_id)?;
                let (c, _) = th.proxy(ty);
                Some(Resolved::boxed(t.clone(), c, ty.name().to_string(), thing_inst(th, ty, [0.0; 4]), ty))
            }
            Target::Actor(a) => {
                let p = self.actor(*a)?.pos;
                Some(Resolved::new(t.clone(), p, self.actor_name(*a)))
            }
            Target::Instance(i) => {
                if let Some(id) = self.things.by_instance.get(i).copied() {
                    return self.resolve(&Target::Thing(id), from);
                }
                let p = self.snap.instances.iter().find(|p| p.id == *i)?;
                let ty = self.snap.type_of(p.type_id)?;
                Some(Resolved::boxed(t.clone(), p.pos + Vec3::Y * ty.sphere_cy.min(1.0) * p.scale, ty.name().to_string(), p.gpu(ty, 1.0), ty))
            }
            Target::Cell(c) => {
                let cell = (c[0], c[1]);
                if let Some(id) = self.things.taken.get(&cell).copied() {
                    return self.resolve(&Target::Thing(id), from);
                }
                let snap = self.snap.clone();
                let it = self.cache.item_at(&snap, cell)?;
                let ty = snap.type_of(it.inst.info[0])?;
                Some(Resolved::boxed(t.clone(), it.inst.pos(), ty.name().to_string(), it.inst, ty))
            }
            Target::Point(p) => Some(Resolved::new(t.clone(), Vec3::from(*p), "there".into())),
            Target::Name(n) => {
                let r = self.find_named(n, origin, from)?;
                self.resolve(&r, from)
            }
        }
    }

    /// The nearest thing or person whose name matches (exact names first).
    pub fn find_named(&mut self, name: &str, near: Vec3, from: ActorId) -> Option<Target> {
        let q = name.trim().trim_start_matches("the ").trim_start_matches("a ").trim_start_matches("an ").to_lowercase();
        if q.is_empty() {
            return None;
        }
        if matches!(q.as_str(), "me" | "myself" | "self") {
            return Some(Target::Actor(from));
        }
        if matches!(q.as_str(), "player" | "traveller" | "traveler" | "you") {
            return Some(Target::Actor(ActorId::Player));
        }
        let score = |n: &str| -> Option<f32> {
            let n = n.to_lowercase();
            if n == q {
                Some(0.0)
            } else if n.contains(&q) || q.contains(&n) {
                Some(1.0)
            } else if n.split_whitespace().any(|w| q.split_whitespace().any(|x| x == w)) {
                Some(2.0)
            } else {
                None
            }
        };
        let mut best: Option<(f32, f32, Target)> = None;
        let consider = |s: f32, d: f32, t: Target, best: &mut Option<(f32, f32, Target)>| {
            if best.as_ref().is_none_or(|b| (s, d) < (b.0, b.1)) {
                *best = Some((s, d, t));
            }
        };
        for a in self.actor_ids() {
            if a == from {
                continue;
            }
            let n = self.actor_name(a);
            if let Some(s) = score(&n) {
                let d = self.actor(a).map(|x| (x.pos - near).length()).unwrap_or(1e9);
                consider(s, d, Target::Actor(a), &mut best);
            }
        }
        for t in self.things.live() {
            let Some(ty) = self.snap.type_of(t.type_id) else { continue };
            if let Some(s) = score(ty.name()) {
                consider(s, (t.pos - near).length(), Target::Thing(t.id), &mut best);
            }
        }
        for p in &self.snap.instances {
            if self.things.by_instance.contains_key(&p.id) || (p.pos - near).length() > 300.0 {
                continue;
            }
            let Some(ty) = self.snap.type_of(p.type_id) else { continue };
            if let Some(s) = score(ty.name()) {
                consider(s, (p.pos - near).length(), Target::Instance(p.id), &mut best);
            }
        }
        if best.as_ref().is_none_or(|b| b.1 > 30.0 || b.0 > 0.0) {
            let snap = self.snap.clone();
            for it in self.cache.items_near(&snap, near, 30.0) {
                let Some(ty) = snap.type_of(it.inst.info[0]) else { continue };
                if let Some(s) = score(ty.name()).or_else(|| ty.ct.meta.tags.iter().find_map(|t| score(t).map(|x| x + 0.5))) {
                    consider(s, (it.inst.pos() - near).length(), Target::Cell([it.cell.0, it.cell.1]), &mut best);
                }
            }
        }
        best.map(|b| b.2)
    }

    /// Bring a target into the live layer and return its thing id.
    pub fn liven(&mut self, t: &Target) -> Option<ThingId> {
        match t {
            Target::Thing(id) => self.things.get(*id).map(|t| t.id),
            Target::Instance(i) => self.promote_instance(*i),
            Target::Cell(c) => self.promote_cell((c[0], c[1])),
            _ => None,
        }
    }

    pub fn promote_instance(&mut self, inst: i64) -> Option<ThingId> {
        if let Some(id) = self.things.by_instance.get(&inst) {
            return Some(*id);
        }
        let p = self.snap.instances.iter().find(|p| p.id == inst)?.clone();
        let (ty, base) = self.type_info(p.type_id)?;
        let id = self.things.alloc();
        let mut th = Thing::new(id, &ty, p.pos, p.rot_y, p.scale, p.params, scaled((*base).clone(), p.scale), self.t);
        th.origin = Origin { instance: Some(inst), made_by: persist::made_by(&self.db, inst), ..Default::default() };
        self.things.insert(th);
        self.sync_overlay();
        Some(id)
    }

    pub fn promote_cell(&mut self, cell: (i32, i32)) -> Option<ThingId> {
        if let Some(id) = self.things.taken.get(&cell) {
            // Negative: used up (eaten, burnt away); nothing there now.
            return (*id >= 0).then_some(*id);
        }
        let snap = self.snap.clone();
        let it = self.cache.item_at(&snap, cell)?;
        if !self.cache.overlay.shows(cell) {
            return None;
        }
        let (ty, base) = self.type_info(it.inst.info[0])?;
        let k = it.inst.k();
        let mut params = [0.0; 8];
        params.copy_from_slice(&k[..8]);
        let scale = it.inst.pos_scale[3];
        let yaw = it.inst.rot[1].atan2(it.inst.rot[0]);
        let props = match self.field.cells.remove(&cell) {
            Some(c) => c.props,
            None => scaled((*base).clone(), scale),
        };
        let id = self.things.alloc();
        let mut th = Thing::new(id, &ty, it.inst.pos(), yaw, scale, params, props, self.t);
        th.origin = Origin { cell: Some(cell), ..Default::default() };
        self.things.insert(th);
        self.sync_overlay();
        Some(id)
    }

    // ------------------------------------------------------------ acting

    /// Do something now. Long actions (walking, waiting) start a task.
    pub fn act(&mut self, who: ActorId, action: Action) -> Result<Outcome, ActErr> {
        if self.actor(who).is_none() {
            return fail("no such actor");
        }
        if who != ActorId::Player {
            if let Some(n) = self.cast.get_mut(who.code()) {
                if n.a.asleep && !matches!(action, Action::Wake | Action::Sleep) {
                    n.a.asleep = false;
                }
            }
        }
        let verb = action.verb();
        let r = self.act_inner(who, action);
        if who == ActorId::Player {
            if let Ok(o) = &r {
                if o.ok && !matches!(verb, "move" | "turn" | "wait" | "goto") {
                    self.player_did(o.msg.clone());
                }
            }
        }
        r
    }

    fn act_inner(&mut self, who: ActorId, action: Action) -> Result<Outcome, ActErr> {
        let me = self.actor(who).cloned().ok_or(ActErr::Fail("no such actor".into()))?;
        let name = self.actor_name(who);
        match action {
            Action::Move { dir, secs } => {
                let d = Vec3::new(dir[0], 0.0, dir[1]).normalize_or_zero();
                if d == Vec3::ZERO {
                    return fail("move needs a direction");
                }
                let until = self.t + secs.clamp(0.0, 60.0) as f64;
                self.set_task(who, Task::Move { dir: d, until });
                Ok(Outcome::ok(format!("{name} walks {}", super::compass(d))))
            }
            Action::Turn { deg } => {
                if let Some(a) = self.actor_mut(who) {
                    a.yaw = deg.to_radians();
                }
                Ok(Outcome::ok(format!("{name} turns to {deg:.0}°")))
            }
            Action::Goto { target, run } => {
                let r = self.resolve(&target, who).ok_or_else(|| not_found(&target))?;
                // A thing is reached at its edge, not its middle (a house is
                // walked up to, not into).
                let half = |ty: Option<&std::sync::Arc<crate::world::TypeEntry>>, scale: f32| ty.map(|t| t.ct.meta.bounds[0].max(t.ct.meta.bounds[2]) * scale).unwrap_or(0.0);
                let edge = match &r.target {
                    Target::Thing(id) => self.things.get(*id).map(|t| half(self.snap.type_of(t.type_id), t.scale)).unwrap_or(0.0),
                    Target::Instance(i) => self.snap.instances.iter().find(|p| p.id == *i).map(|p| half(self.snap.type_of(p.type_id), p.scale)).unwrap_or(0.0),
                    _ => 0.0,
                };
                let stop = match r.target {
                    Target::Actor(_) => 1.4,
                    Target::Point(_) => 0.6,
                    _ => self.reach_of(who) * 0.7 + edge,
                };
                let deadline = self.t + 120.0;
                self.set_task(who, Task::Goto { target: r.target.clone(), stop, run, deadline });
                Ok(Outcome::ok(format!("{name} heads for {}", the(&r.name))))
            }
            Action::Hold { target } => self.hold(who, &target),
            Action::Drop => {
                let id = me.held.ok_or(ActErr::Fail("not holding anything".into()))?;
                let tname = self.thing_name(id);
                let v = me.forward() * 0.6;
                self.drop_thing(id, v);
                let at = me.pos;
                self.event("dropped", Some(who), Some(format!("thing:{id}")), format!("{name} put down the {tname}"), Some(at), json!({}));
                Ok(Outcome::ok(format!("{name} drops the {tname}")).thing(id))
            }
            Action::Place { at } => {
                let id = me.held.ok_or(ActErr::Fail("not holding anything".into()))?;
                let p = Vec3::from(at);
                if !self.in_reach(who, p) {
                    return Err(ActErr::TooFar { at: p, dist: (p - me.pos).length() });
                }
                let tname = self.thing_name(id);
                self.release(id);
                let ty = self.things.get(id).and_then(|t| self.snap.type_of(t.type_id)).cloned();
                let g = self.snap.terrain.height(p.x, p.z);
                if let (Some(t), Some(ty)) = (self.things.get_mut(id), ty) {
                    t.pos = Vec3::new(p.x, t.rest_y(&ty, g.max(p.y.min(g + 3.0))), p.z);
                    t.vel = Vec3::ZERO;
                    t.asleep = false;
                    t.dirty = true;
                }
                self.things.moved();
                self.event("placed", Some(who), Some(format!("thing:{id}")), format!("{name} set the {tname} down"), Some(p), json!({}));
                Ok(Outcome::ok(format!("{name} sets the {tname} down")).thing(id))
            }
            Action::Throw { at, dir, force } => self.throw(who, at, dir, force),
            Action::Use { target, on, at } => self.use_thing(who, target, on, at.map(Vec3::from)),
            Action::Eat { target } => {
                let id = match target {
                    Some(t) => {
                        let r = self.resolve(&t, who).ok_or_else(|| not_found(&t))?;
                        if me.held.is_none_or(|h| Target::Thing(h) != r.target) {
                            self.reach(who, &r)?;
                        }
                        self.liven(&r.target).ok_or(ActErr::Fail("can't eat that".into()))?
                    }
                    None => me.held.ok_or(ActErr::Fail("eat what?".into()))?,
                };
                self.eat(who, id)
            }
            Action::Do { text, on, at } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return fail("do what?");
                }
                let target = match on {
                    Some(t) => Some(self.resolve(&t, who).ok_or_else(|| not_found(&t))?),
                    None => None,
                };
                self.interpret(who, &text, target, at.map(Vec3::from))
            }
            Action::Create { text } => {
                if !self.has_llm {
                    return fail("making new things needs an LLM");
                }
                let id = self.next_id();
                let eye = me.eye();
                let pitch = if who == ActorId::Player { self.look_pitch } else { -0.12 };
                let cam = crate::render::Camera { pos: eye, yaw: me.yaw, pitch, fov_y: 1.05 };
                let npcs = self.npc_views();
                let snap = self.snap.clone();
                let view = crate::world::describe::describe(&snap, &mut self.cache, &npcs, &cam, 1.6, self.t);
                let ahead = me.pos + me.forward() * 6.0;
                let target = match &view.target {
                    Some(t) if t.distance > 3.0 && t.distance < 30.0 => Vec3::new(t.x, t.y, t.z),
                    _ => Vec3::new(ahead.x, self.snap.terrain.height(ahead.x, ahead.z), ahead.z),
                };
                let req = Request::Create { id, by: who, text: text.clone(), target, yaw: me.yaw, view: Box::new(view) };
                if who == ActorId::Player {
                    self.request_now(req);
                } else {
                    self.request(req, me.pos);
                }
                self.interp.creating.insert(id, (who, self.t, text.clone()));
                self.event("create", Some(who), None, format!("{name} sets out to make {text}"), Some(me.pos), json!({ "text": text, "id": id }));
                Ok(Outcome { ok: true, msg: format!("{name} starts making {text}"), pending: Some(id), thing: None })
            }
            Action::Say { text, to } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return fail("say what?");
                }
                let to_r = match to {
                    Some(t) => self.resolve(&t, who),
                    None => None,
                };
                self.say(who, &text, to_r.as_ref().map(|r| r.target.clone()));
                Ok(Outcome::ok(format!("{name} says \"{text}\"")))
            }
            Action::Gesture { kind, to } => {
                let to_r = match &to {
                    Some(t) => Some(self.resolve(t, who).ok_or_else(|| not_found(t))?),
                    None => None,
                };
                match GestureKind::parse(&kind) {
                    Some(k) => {
                        self.ask_body_gesture(who, k);
                        self.gesture(who, k, to_r)
                    }
                    None if self.has_llm && kind.trim().len() <= 24 && kind.trim().chars().all(|c| c.is_alphabetic() || c == ' ' || c == '_' || c == '-') => {
                        // A gesture nobody knows yet: have its pose written, then do it.
                        let id = self.next_id();
                        let name = kind.trim().to_lowercase().replace(['-', ' '], "_");
                        self.interp.gestures.insert(id, (who, to_r.map(|r| r.target), name.clone()));
                        let req = Request::BuildGesture { id, name: name.clone(), body: String::new() };
                        if who == ActorId::Player {
                            self.request_now(req);
                        } else {
                            self.request(req, me.pos);
                        }
                        Ok(Outcome { ok: true, msg: format!("{} tries to {}…", self.actor_name(who), name.replace('_', " ")), pending: Some(id), thing: None })
                    }
                    None => Err(ActErr::Fail(format!("unknown gesture '{kind}' (wave, bow, nod, point, cheer, shrug, dance, sit, handshake, high_five, hug, kiss)"))),
                }
            }
            Action::Propose { to, activity, with } => {
                let r = self.resolve(&to, who).ok_or_else(|| not_found(&to))?;
                let Target::Actor(other) = r.target else { return fail("you can only propose things to people") };
                let with = match with {
                    Some(w) => self.resolve(&w, who).and_then(|r| self.liven(&r.target)),
                    None => None,
                };
                self.propose(who, other, &activity, with)
            }
            Action::Answer { yes, to } => {
                let from = match to {
                    Some(t) => match self.resolve(&t, who).map(|r| r.target) {
                        Some(Target::Actor(a)) => Some(a),
                        _ => return fail("answer whom?"),
                    },
                    None => None,
                };
                self.answer(who, from, yes)
            }
            Action::Ride { target } => {
                let r = self.resolve(&target, who).ok_or_else(|| not_found(&target))?;
                let Target::Actor(m) = r.target else { return fail("you can only ride a living body") };
                self.ride(who, m)
            }
            Action::Dismount => self.dismount(who),
            Action::Wear { target, on } => {
                let wearer = match &on {
                    Some(t) => match self.resolve(t, who).map(|r| (r.target, r.pos)) {
                        Some((Target::Actor(a), p)) => {
                            if (p - me.pos).length() > self.reach_of(who) + 0.6 {
                                return Err(ActErr::TooFar { at: p, dist: (p - me.pos).length() });
                            }
                            a
                        }
                        _ => return fail("put it on whom?"),
                    },
                    None => who,
                };
                let id = match &target {
                    Some(t) => {
                        let r = self.resolve(t, who).ok_or_else(|| not_found(t))?;
                        self.reach(who, &r)?;
                        self.liven(&r.target).ok_or(ActErr::Fail(format!("the {} can't be worn", r.name)))?
                    }
                    None => me.held.ok_or(ActErr::Fail("not holding anything to put on".into()))?,
                };
                self.wear(who, wearer, id)
            }
            Action::TakeOff { target, from } => {
                let wearer = match &from {
                    Some(t) => match self.resolve(t, who).map(|r| r.target) {
                        Some(Target::Actor(a)) => a,
                        _ => return fail("take it off whom?"),
                    },
                    None => who,
                };
                let id = match &target {
                    Some(t) => match self.resolve(t, who).map(|r| r.target) {
                        Some(Target::Thing(id)) => id,
                        _ => return Err(not_found(t)),
                    },
                    None => self.worn_by(wearer).last().copied().ok_or(ActErr::Fail(format!("{} isn't wearing anything to take off", self.actor_name(wearer))))?,
                };
                self.take_off(who, wearer, id)
            }
            Action::Give { to } => {
                let id = me.held.ok_or(ActErr::Fail("not holding anything to give".into()))?;
                let r = self.resolve(&to, who).ok_or_else(|| not_found(&to))?;
                let Target::Actor(other) = r.target else { return fail("give it to whom?") };
                if (r.pos - me.pos).length() > self.reach_of(who) + 0.4 {
                    return Err(ActErr::TooFar { at: r.pos, dist: (r.pos - me.pos).length() });
                }
                self.give(who, other, id)
            }
            Action::Follow { target, secs } => {
                let r = self.resolve(&target, who).ok_or_else(|| not_found(&target))?;
                let Target::Actor(other) = r.target else { return fail("follow whom?") };
                let until = self.t + secs.unwrap_or(60.0).clamp(1.0, 600.0) as f64;
                self.set_task(who, Task::Follow { who: other, dist: 2.0, until });
                Ok(Outcome::ok(format!("{name} follows {}", r.name)))
            }
            Action::Wait { secs } => {
                let until = self.t + secs.clamp(0.0, 600.0) as f64;
                self.set_task(who, Task::Wait { until });
                Ok(Outcome::ok(format!("{name} waits")))
            }
            Action::WaitUntil { hour } => {
                let hours = (hour.rem_euclid(24.0) - self.hour()).rem_euclid(24.0) as f64;
                let until = self.t + hours * crate::render::sky::DAY_SECONDS / 24.0;
                self.set_task(who, Task::Wait { until });
                Ok(Outcome::ok(format!("{name} waits for the hour")))
            }
            Action::Ask { who: names, what } => self.ask_step(who, &names, &what),
            Action::Plea { to } => {
                let r = self.resolve(&to, who).ok_or_else(|| not_found(&to))?;
                if (r.pos - me.pos).length() > 3.0 {
                    return Err(ActErr::TooFar { at: r.pos, dist: (r.pos - me.pos).length() });
                }
                self.plea_step(who, &to)
            }
            Action::Resume { on } => self.resume_step(who, on),
            Action::Sleep => {
                if let Some(a) = self.actor_mut(who) {
                    a.asleep = true;
                    a.task = None;
                }
                self.event("slept", Some(who), None, format!("{name} lay down to sleep"), Some(me.pos), json!({}));
                Ok(Outcome::ok(format!("{name} goes to sleep")))
            }
            Action::Wake => {
                if let Some(a) = self.actor_mut(who) {
                    a.asleep = false;
                }
                Ok(Outcome::ok(format!("{name} wakes up")))
            }
            Action::GoHome => {
                let home = match who {
                    ActorId::Npc(c) => self.cast.get(c).map(|n| n.def.home),
                    ActorId::Player => Some(self.snap.spawn),
                };
                let home = home.ok_or(ActErr::Fail("no home".into()))?;
                let deadline = self.t + 300.0;
                self.set_task(who, Task::Goto { target: Target::Point(home.to_array()), stop: 0.8, run: false, deadline });
                Ok(Outcome::ok(format!("{name} heads home")))
            }
        }
    }

    pub fn set_task(&mut self, who: ActorId, task: Task) {
        if let Some(a) = self.actor_mut(who) {
            a.task = Some(task);
            a.asleep = false;
        }
    }

    fn hold(&mut self, who: ActorId, target: &Target) -> Result<Outcome, ActErr> {
        let me = self.actor(who).cloned().ok_or(ActErr::Fail("no such actor".into()))?;
        let name = self.actor_name(who);
        let r = self.resolve(target, who).ok_or_else(|| not_found(target))?;
        if let Target::Actor(_) = r.target {
            return fail("you can't pick up a person; try a hug");
        }
        if let Some(h) = me.held {
            if Some(h) == self.liven_peek(&r.target) {
                return Ok(Outcome::ok(format!("{name} is already holding the {}", r.name)).thing(h));
            }
            return fail(format!("hands are full: drop the {} first", self.thing_name(h)));
        }
        self.reach(who, &r)?;
        let id = self.liven(&r.target).ok_or(ActErr::Fail(format!("the {} can't be picked up", r.name)))?;
        let t = self.things.get(id).cloned().ok_or(ActErr::Fail("it's gone".into()))?;
        if t.anchored {
            return fail(format!("the {} won't budge", r.name));
        }
        let mass = t.mass();
        let strength = self.strength(who);
        match t.holder {
            Some(h) if h != who => {
                if t.co_holder.is_some() || mass <= strength {
                    return fail(format!("{} is holding the {}", self.actor_name(h), r.name));
                }
                // The other end of something heavy.
                if mass > strength * 2.0 {
                    return fail(format!("the {} is too heavy even for two ({mass:.0} kg)", r.name));
                }
                if let Some(t) = self.things.get_mut(id) {
                    t.co_holder = Some(who);
                    t.asleep = false;
                    t.dirty = true;
                }
                if let Some(a) = self.actor_mut(who) {
                    a.held = Some(id);
                }
                let other = self.actor_name(h);
                self.event("carry", Some(who), Some(format!("thing:{id}")), format!("{name} and {other} lift the {} together", r.name), Some(r.pos), json!({ "with": h }));
                self.note_near(r.pos, 30.0, Note::Info(format!("{} and {other} lift the {} together.", super::physics::cap(&name), r.name)));
                self.witness(r.pos, 20.0, &format!("{name} and {other} carried the {} together.", r.name), 0.35, &[who, h]);
                self.social.bond(who, h, 0.05, self.t);
                return Ok(Outcome::ok(format!("{name} lifts the {} together with {other}", r.name)).thing(id));
            }
            _ => {}
        }
        if mass > strength * 2.0 {
            return fail(format!("the {} is far too heavy ({mass:.0} kg)", r.name));
        }
        if let Some(t) = self.things.get_mut(id) {
            t.holder = Some(who);
            t.asleep = false;
            t.thrown_by = None;
            t.dirty = true;
        }
        if let Some(a) = self.actor_mut(who) {
            a.held = Some(id);
        }
        if mass > strength {
            self.event("grip", Some(who), Some(format!("thing:{id}")), format!("{name} grabs one end of the {}; it needs two", r.name), Some(r.pos), json!({ "mass": mass }));
            return Ok(Outcome::ok(format!("{name} grabs one end of the {} ({mass:.0} kg): it needs a second pair of hands", r.name)).thing(id));
        }
        self.event("picked_up", Some(who), Some(format!("thing:{id}")), format!("{name} picked up the {}", r.name), Some(r.pos), json!({}));
        if who != ActorId::Player {
            self.note_near(r.pos, 15.0, Note::Info(format!("{} picks up {} {}.", super::physics::cap(&name), article(&r.name), r.name)));
        }
        Ok(Outcome::ok(format!("{name} picks up the {}", r.name)).thing(id))
    }

    /// Origin, type and scale of a placed object or live thing.
    fn target_shape(&self, t: &Target) -> Option<(Vec3, std::sync::Arc<crate::world::TypeEntry>, f32)> {
        match t {
            Target::Thing(id) => {
                let th = self.things.get(*id)?;
                Some((th.pos, self.snap.type_of(th.type_id)?.clone(), th.scale))
            }
            Target::Instance(i) => {
                if let Some(id) = self.things.by_instance.get(i) {
                    return self.target_shape(&Target::Thing(*id));
                }
                let p = self.snap.instances.iter().find(|p| p.id == *i)?;
                Some((p.pos, self.snap.type_of(p.type_id)?.clone(), p.scale))
            }
            _ => None,
        }
    }

    /// The thing id a target already has, without promoting it.
    fn liven_peek(&self, t: &Target) -> Option<ThingId> {
        match t {
            Target::Thing(id) => Some(*id),
            Target::Instance(i) => self.things.by_instance.get(i).copied(),
            Target::Cell(c) => self.things.taken.get(&(c[0], c[1])).copied(),
            _ => None,
        }
    }

    fn throw(&mut self, who: ActorId, at: Option<Target>, dir: Option<[f32; 3]>, force: Option<f32>) -> Result<Outcome, ActErr> {
        let me = self.actor(who).cloned().ok_or(ActErr::Fail("no such actor".into()))?;
        let name = self.actor_name(who);
        let id = me.held.ok_or(ActErr::Fail("not holding anything to throw".into()))?;
        let t = self.things.get(id).cloned().ok_or(ActErr::Fail("it's gone".into()))?;
        if t.co_holder.is_some() || t.mass() > self.strength(who) {
            return fail("too heavy to throw");
        }
        let tname = self.thing_name(id);
        let vmax = (240.0 / t.mass().max(0.05)).sqrt().min(22.0);
        let ty = self.type_entry(t.type_id).ok_or(ActErr::Fail("it's gone".into()))?;
        let (start, _) = t.proxy(&ty);
        let mut target_actor = None;
        let mut target_name = String::new();
        let v = match (at, dir) {
            (Some(a), _) => {
                let r = self.resolve(&a, who).ok_or(ActErr::Fail("throw at what?".into()))?;
                let mut dest = r.pos;
                let mut lob = false;
                if let Target::Actor(o) = r.target {
                    dest += Vec3::Y * 1.15;
                    target_actor = Some(o);
                } else if let Some((base, ty, scale)) = self.target_shape(&r.target) {
                    // Lob it onto the top of objects: rims and openings face up.
                    dest = base + Vec3::Y * (ty.top * scale - 0.03);
                    lob = true;
                }
                target_name = r.name.clone();
                let d = dest - start;
                let horiz = Vec3::new(d.x, 0.0, d.z).length();
                let tf = if lob { (horiz / 2.8).clamp(0.8, 2.2) } else { (horiz / 9.0).clamp(0.35, 1.6) };
                let mut v = Vec3::new(d.x / tf, d.y / tf + 0.5 * super::physics::GRAVITY * tf, d.z / tf);
                if let Some(f) = force {
                    v = v.normalize_or_zero() * f.clamp(0.5, vmax).max(v.length().min(f));
                }
                if v.length() > vmax {
                    v = v.normalize_or_zero() * vmax;
                }
                if let Some(o) = target_actor {
                    if let Some(x) = self.actor_mut(o) {
                        x.catching = 0.0;
                    }
                    self.ready_to_catch(o, who, tf as f64 + 0.8);
                }
                v
            }
            (None, Some(d)) => Vec3::from(d).normalize_or_zero() * force.unwrap_or(8.0).clamp(0.5, vmax),
            (None, None) => {
                let f = me.forward();
                (f + Vec3::Y * 0.45).normalize() * force.unwrap_or(9.0).clamp(0.5, vmax)
            }
        };
        if let Some(a) = self.actor_mut(who) {
            if v.x.abs() + v.z.abs() > 0.1 {
                a.yaw = v.x.atan2(v.z);
            }
        }
        self.drop_thing(id, v);
        if let Some(t) = self.things.get_mut(id) {
            t.thrown_by = Some((who, self.t));
            t.spin = 4.0;
            t.through = None;
        }
        let msg = if target_name.is_empty() { format!("{name} throws the {tname}") } else { format!("{name} throws the {tname} at {}", the(&target_name)) };
        self.event("threw", Some(who), Some(format!("thing:{id}")), msg.clone(), Some(me.pos), json!({ "at": target_name, "speed": (v.length() as f64 * 10.0).round() / 10.0, "to": target_actor }));
        if who != ActorId::Player {
            self.note_near(me.pos, 25.0, Note::Info(format!("{}.", super::physics::cap(&msg))));
        }
        self.animals_notice_throw(me.pos);
        Ok(Outcome::ok(msg).thing(id))
    }

    /// Use: eat it, run its own `use` code, let the world's rules decide
    /// (contact), or ask the interpreter, in that order.
    fn use_thing(&mut self, who: ActorId, target: Option<Target>, on: Option<Target>, at: Option<Vec3>) -> Result<Outcome, ActErr> {
        let me = self.actor(who).cloned().ok_or(ActErr::Fail("no such actor".into()))?;
        let rt = match &target {
            Some(t) => Some(self.resolve(t, who).ok_or_else(|| not_found(t))?),
            None => None,
        };
        let ro = match &on {
            Some(t) => Some(self.resolve(t, who).ok_or_else(|| not_found(t))?),
            None => None,
        };
        // Which is the tool, which the object.
        let held_t = me.held.map(Target::Thing);
        let (tool, object) = match (rt, ro) {
            (None, None) => (held_t.clone().map(|t| Resolved::new(t, me.pos, self.thing_name(me.held.unwrap_or(0)))).ok_or(ActErr::Fail("use what?".into()))?, None),
            (Some(x), None) => {
                let is_held = held_t.as_ref().is_some_and(|h| self.liven_peek(&x.target).map(Target::Thing).as_ref() == Some(h));
                match (&held_t, is_held) {
                    (Some(h), false) if !matches!(x.target, Target::Point(_)) => (Resolved::new(h.clone(), me.pos, self.thing_name(me.held.unwrap_or(0))), Some(x)),
                    _ => (x, None),
                }
            }
            (Some(x), Some(y)) => (x, Some(y)),
            (None, Some(y)) => (held_t.clone().map(|t| Resolved::new(t, me.pos, self.thing_name(me.held.unwrap_or(0)))).ok_or(ActErr::Fail("use what on it?".into()))?, Some(y)),
        };
        if let Target::Actor(_) = tool.target {
            return fail("use a thing, not a person");
        }
        let tool_held = held_t.as_ref().is_some_and(|h| self.liven_peek(&tool.target).map(Target::Thing).as_ref() == Some(h));
        if !tool_held {
            self.reach(who, &tool)?;
        }
        if let Some(o) = &object {
            if !matches!(o.target, Target::Point(_)) {
                self.reach(who, o)?;
            }
        }
        let a = self.liven(&tool.target).ok_or(ActErr::Fail(format!("can't use the {}", tool.name)))?;
        let b = match &object {
            Some(o) => match o.target {
                Target::Actor(_) | Target::Point(_) => None,
                _ => self.liven(&o.target),
            },
            None => None,
        };
        let name = self.actor_name(who);
        let tname = self.thing_name(a);
        // 1. Food.
        if object.is_none() && self.things.get(a).is_some_and(|t| t.props[P_EDIBLE] > 0.0) {
            return self.eat(who, a);
        }
        // 2. Its own code.
        if self.run_use(a, b, who) {
            let what = match &object {
                Some(o) => format!("{name} uses the {tname} on {}", the(&o.name)),
                None => format!("{name} uses the {tname}"),
            };
            self.event("used", Some(who), Some(format!("thing:{a}")), what.clone(), Some(tool.pos), json!({ "on": b }));
            return Ok(Outcome::ok(what).thing(a));
        }
        // 3. The world's rules: bring the two into firm contact.
        if let Some(b) = b {
            if self.contact(a, b) {
                let oname = self.thing_name(b);
                let what = format!("{name} touches the {tname} to the {oname}");
                self.event("used", Some(who), Some(format!("thing:{a}")), what.clone(), Some(tool.pos), json!({ "on": b, "by": "rules" }));
                return Ok(Outcome::ok(what).thing(a));
            }
        }
        // 4. Something new: ask the world.
        let text = match &object {
            Some(o) => format!("use the {tname} on {}", the(&o.name)),
            None => format!("use the {tname}"),
        };
        let target = object.or(Some(tool));
        self.interpret(who, &text, target, at)
    }

    fn eat(&mut self, who: ActorId, id: ThingId) -> Result<Outcome, ActErr> {
        let t = self.things.get(id).cloned().ok_or(ActErr::Fail("it's gone".into()))?;
        let e = t.props[P_EDIBLE];
        let tname = self.thing_name(id);
        if e <= 0.0 {
            return fail(format!("the {tname} isn't something to eat"));
        }
        if t.mass() > 5.0 {
            return fail(format!("the {tname} is too big to eat"));
        }
        let name = self.actor_name(who);
        let pos = t.pos;
        self.release(id);
        self.things.remove(id);
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get_mut(c) {
                n.needs.hunger = (n.needs.hunger - e * 1.5).max(0.0);
            }
        }
        self.event("ate", Some(who), Some(format!("thing:{id}")), format!("{name} ate the {tname}"), Some(pos), json!({ "edible": e }));
        if who != ActorId::Player {
            self.note_near(pos, 15.0, Note::Info(format!("{} eats {} {tname}.", super::physics::cap(&name), article(&tname))));
        }
        Ok(Outcome::ok(format!("{name} eats the {tname}")))
    }

    /// Say something aloud: nearby people hear it.
    pub fn say(&mut self, who: ActorId, text: &str, to: Option<Target>) {
        let Some(at) = self.actor(who).map(|a| a.pos) else { return };
        let name = self.actor_name(who);
        let to_name = to.as_ref().and_then(|t| match t {
            Target::Actor(a) => Some(self.actor_name(*a)),
            _ => None,
        });
        if let ActorId::Npc(c) = who {
            if !self.speaks(who) {
                self.speak_as_animal(c, text);
                return;
            }
        }
        if who != ActorId::Player {
            self.note_near(at, 22.0, Note::Line { id: Some(who), who: name.clone(), text: text.to_string() });
        }
        let what = match &to_name {
            Some(n) => format!("{name} said to {n}: \"{text}\""),
            None => format!("{name} said: \"{text}\""),
        };
        self.event("said", Some(who), to.as_ref().and_then(|t| if let Target::Actor(a) = t { Some(a.key()) } else { None }), what.clone(), Some(at), json!({ "text": text }));
        self.witness(at, 14.0, &what, 0.3, &[who]);
        // The player speaking to a character starts a conversation.
        if who == ActorId::Player {
            if let Some(Target::Actor(ActorId::Npc(c))) = to {
                self.player_talks(c, text);
            }
        }
    }

    /// Contact between two things for a moment: the world's pair rules run
    /// both ways as if they touched for a second. True if anything changed.
    pub fn contact(&mut self, a: ThingId, b: ThingId) -> bool {
        let (Some(ta), Some(tb)) = (self.things.get(a).cloned(), self.things.get(b).cloned()) else { return false };
        let rules = self.rules.clone();
        let hour = self.hour();
        let night = self.night() as u32 as f32;
        let mut na = ta.props.clone();
        let mut nb = tb.props.clone();
        let va = super::rules::EntView { props: &ta.props, water: 0.0, held: 1.0, ground: 0.0 };
        let vb = super::rules::EntView { props: &tb.props, water: 0.0, held: 0.0, ground: 1.0 };
        let mut fired = Vec::new();
        for r in rules.iter().filter(|r| r.near.is_some()) {
            if super::rules::pair_self_ok(r, &va, 1.0, hour, night) && super::rules::run_pair(r, &va, &vb, 0.1, 1.0, hour, night, &mut na, &mut nb) {
                fired.push(r.spec.name.clone());
            }
            if super::rules::pair_self_ok(r, &vb, 1.0, hour, night) && super::rules::run_pair(r, &vb, &va, 0.1, 1.0, hour, night, &mut nb, &mut na) {
                fired.push(r.spec.name.clone());
            }
        }
        sanitize(&mut na);
        sanitize(&mut nb);
        let changed = |x: &Props, y: &Props| x.iter().zip(y).any(|(p, q)| (p - q).abs() > 1e-3);
        let any = changed(&na, &ta.props) || changed(&nb, &tb.props);
        let t = self.t;
        if let Some(x) = self.things.get_mut(a) {
            x.props = na;
            x.dirty = true;
            for f in &fired {
                x.note_rule(f, t);
            }
        }
        if let Some(x) = self.things.get_mut(b) {
            x.props = nb;
            x.dirty = true;
            for f in &fired {
                x.note_rule(f, t);
            }
            x.asleep = false;
        }
        any
    }

    /// Hand a held thing to someone (characters decide whether to take it).
    fn give(&mut self, who: ActorId, to: ActorId, id: ThingId) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let other = self.actor_name(to);
        let tname = self.thing_name(id);
        if self.actor(to).is_some_and(|a| a.held.is_some()) {
            return fail(format!("{other}'s hands are full"));
        }
        if let ActorId::Npc(c) = to {
            if !self.npc_accepts_gift(c, who, id) {
                self.event("refused", Some(to), Some(format!("thing:{id}")), format!("{other} wouldn't take the {tname} from {name}"), None, json!({}));
                return fail(format!("{other} won't take it"));
            }
        }
        self.release(id);
        if let Some(t) = self.things.get_mut(id) {
            t.holder = Some(to);
            t.asleep = false;
            t.dirty = true;
        }
        if let Some(a) = self.actor_mut(to) {
            a.held = Some(id);
        }
        let at = self.actor(to).map(|a| a.pos);
        self.event("gave", Some(who), Some(format!("thing:{id}")), format!("{name} gave the {tname} to {other}"), at, json!({ "to": to }));
        if let Some(p) = at {
            self.note_near(p, 20.0, Note::Info(format!("{} gives the {tname} to {other}.", super::physics::cap(&name))));
            self.witness(p, 15.0, &format!("{name} gave {other} {} {tname}.", article(&tname)), 0.4, &[]);
        }
        self.social.bond(who, to, 0.12, self.t);
        self.on_gift(to, who, id);
        self.need_given(who, to);
        Ok(Outcome::ok(format!("{name} gives the {tname} to {other}")).thing(id))
    }

    /// Layers someone wears, oldest first.
    pub fn worn_by(&self, who: ActorId) -> Vec<ThingId> {
        self.things.live().filter(|t| t.worn == Some(who)).map(|t| t.id).collect()
    }

    /// The name of the body an actor lives in.
    pub fn body_name(&self, who: ActorId) -> String {
        match who {
            ActorId::Npc(c) => self.cast.get(c).and_then(|n| self.snap.type_of(n.body_ty)).map(|t| t.name().to_string()).unwrap_or_else(|| "figure".into()),
            ActorId::Player => "figure".into(),
        }
    }

    /// Put a layer on someone (yourself, or someone who agrees).
    pub fn wear(&mut self, who: ActorId, wearer: ActorId, id: ThingId) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let wname = self.actor_name(wearer);
        let tname = self.thing_name(id);
        let t = self.things.get(id).cloned().ok_or(ActErr::Fail("it's gone".into()))?;
        let ty = self.type_entry(t.type_id).ok_or(ActErr::Fail("it's gone".into()))?;
        if !(ty.has_tag("layer") || ty.ct.meta.fits.is_some()) {
            return fail(format!("the {tname} isn't something to wear"));
        }
        if t.worn.is_some() {
            return fail(format!("the {tname} is already being worn"));
        }
        let body = self.body_name(wearer);
        if let Some(f) = ty.ct.meta.fits.as_deref().filter(|f| *f != body) {
            return fail(format!("the {tname} is made for a {f} body, not {wname}'s"));
        }
        if self.worn_by(wearer).len() >= 3 {
            return fail(format!("{wname} already wears three things"));
        }
        if t.holder.is_some_and(|h| h != who) {
            return fail(format!("{} is holding the {tname}", self.actor_name(t.holder.unwrap_or(who))));
        }
        if wearer != who {
            if let ActorId::Npc(c) = wearer {
                let aff = self.social.affection(wearer, who) - self.wariness(c);
                if aff < 0.15 {
                    self.event("refused", Some(wearer), Some(format!("thing:{id}")), format!("{wname} wouldn't let {name} put the {tname} on them"), self.actor(wearer).map(|a| a.pos), json!({}));
                    if !self.speaks(wearer) {
                        self.make_noise(c, false);
                    } else {
                        self.say_template(wearer, "no", &name);
                    }
                    return fail(format!("{wname} won't let you"));
                }
            }
        }
        self.release(id);
        let at = self.actor(wearer).map(|a| (a.pos, a.yaw));
        if let Some(t) = self.things.get_mut(id) {
            t.worn = Some(wearer);
            t.holder = None;
            t.co_holder = None;
            if let Some((p, yaw)) = at {
                t.pos = p;
                t.yaw = yaw;
            }
            t.vel = Vec3::ZERO;
            t.asleep = true;
            t.dirty = true;
        }
        let msg = if wearer == who { format!("{name} puts on the {tname}") } else { format!("{name} puts the {tname} on {wname}") };
        let p = at.map(|x| x.0);
        self.event("wore", Some(who), Some(format!("thing:{id}")), msg.clone(), p, json!({ "on": wearer }));
        if let Some(p) = p {
            self.note_near(p, 20.0, Note::Info(format!("{}.", super::physics::cap(&msg))));
            self.witness(p, 15.0, &msg, 0.3, &[]);
        }
        if wearer != who {
            self.social.bond(who, wearer, 0.06, self.t);
        }
        Ok(Outcome::ok(msg).thing(id))
    }

    /// Take a layer off (yourself, or someone who agrees): into the hands,
    /// or to the ground when they are full.
    pub fn take_off(&mut self, who: ActorId, wearer: ActorId, id: ThingId) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let wname = self.actor_name(wearer);
        let tname = self.thing_name(id);
        if self.things.get(id).is_none_or(|t| t.worn != Some(wearer)) {
            return fail(format!("{wname} isn't wearing the {tname}"));
        }
        if wearer != who {
            let Some(p) = self.actor(wearer).map(|a| a.pos) else { return fail("they're gone") };
            let me = self.actor(who).map(|a| a.pos).unwrap_or(p);
            if (p - me).length() > self.reach_of(who) + 0.6 {
                return Err(ActErr::TooFar { at: p, dist: (p - me).length() });
            }
            if let ActorId::Npc(c) = wearer {
                if self.social.affection(wearer, who) - self.wariness(c) < 0.25 {
                    self.event("refused", Some(wearer), Some(format!("thing:{id}")), format!("{wname} wouldn't let {name} take the {tname}"), Some(p), json!({}));
                    return fail(format!("{wname} won't let you"));
                }
            }
        }
        let hands_free = self.actor(who).is_some_and(|a| a.held.is_none());
        if let Some(t) = self.things.get_mut(id) {
            t.worn = None;
            t.asleep = false;
            t.dirty = true;
            if hands_free {
                t.holder = Some(who);
            }
        }
        if hands_free {
            if let Some(a) = self.actor_mut(who) {
                a.held = Some(id);
            }
        }
        let msg = if wearer == who { format!("{name} takes off the {tname}") } else { format!("{name} takes the {tname} off {wname}") };
        let p = self.actor(wearer).map(|a| a.pos);
        self.event("took_off", Some(who), Some(format!("thing:{id}")), msg.clone(), p, json!({ "from": wearer }));
        if let Some(p) = p {
            self.note_near(p, 20.0, Note::Info(format!("{}.", super::physics::cap(&msg))));
        }
        Ok(Outcome::ok(msg).thing(id))
    }

    /// Start a gesture, alone or with someone.
    fn gesture(&mut self, who: ActorId, k: GestureKind, to: Option<Resolved>) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let other = to.as_ref().and_then(|r| if let Target::Actor(a) = r.target { Some(a) } else { None });
        if let Some(r) = &to {
            if let Some(a) = self.actor_mut(who) {
                let d = r.pos - a.pos;
                a.face(d, 10.0);
            }
        }
        if k.contact() {
            let Some(o) = other else { return fail(format!("{} with whom?", k.name())) };
            return self.contact_gesture(who, o, k);
        }
        let t = self.t;
        if let Some(a) = self.actor_mut(who) {
            a.gesture = Some(GestureRun { kind: k, with: other, t0: t, dur: k.duration() });
        }
        let at = self.actor(who).map(|a| a.pos).unwrap_or_default();
        let msg = match other {
            Some(o) => format!("{name} {}s at {}", verb_s(k), self.actor_name(o)),
            None => format!("{name} {}s", verb_s(k)),
        };
        self.event("gesture", Some(who), other.map(|o| o.key()), msg.clone(), Some(at), json!({ "kind": k.name() }));
        if who != ActorId::Player {
            self.note_near(at, 25.0, Note::Info(format!("{}.", super::physics::cap(&msg))));
        }
        if let Some(o) = other {
            self.on_gestured(o, who, k);
        }
        Ok(Outcome::ok(msg))
    }

    /// The characters near enough to describe.
    pub fn npc_views(&self) -> Vec<crate::world::describe::NpcView> {
        self.cast.npcs.iter().filter(|n| !n.dead).map(|n| crate::world::describe::NpcView { id: n.def.id, name: n.name().to_string(), pos: n.a.pos }).collect()
    }
}

fn not_found(t: &Target) -> ActErr {
    ActErr::Fail(match t {
        Target::Name(n) => format!("there's nothing called '{n}' nearby"),
        Target::Thing(id) => format!("thing {id} is gone"),
        Target::Actor(a) => format!("{} isn't here", a.key()),
        Target::Instance(i) => format!("instance {i} doesn't exist"),
        Target::Cell(c) => format!("nothing grows in cell {},{}", c[0], c[1]),
        Target::Point(_) => "that point is nowhere".into(),
    })
}

pub fn the(name: &str) -> String {
    if name == "there" || name.starts_with("the ") || name.chars().next().is_some_and(|c| c.is_uppercase()) {
        name.to_string()
    } else {
        format!("the {name}")
    }
}

fn verb_s(k: GestureKind) -> &'static str {
    match k {
        GestureKind::Wave => "wave",
        GestureKind::Bow => "bow",
        GestureKind::Nod => "nod",
        GestureKind::Point => "point",
        GestureKind::Cheer => "cheer",
        GestureKind::Shrug => "shrug",
        GestureKind::Dance => "dance",
        GestureKind::Sit => "sit",
        _ => "gesture",
    }
}
