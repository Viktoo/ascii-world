//! Behaviour code: a thing's own `tick`, `use` and `touch` functions, run on
//! the bytecode VM with a fuel limit. They change the thing's state and
//! properties and queue effects (say, sound, spawn, transform, remove), which
//! are applied here with caps so no script can flood the world.

use super::props::*;
use super::things::{Origin, ThingId};
use super::{ActorId, Note, Request, Sim};
use crate::lang::BEHAVIOR_FUEL;
use crate::lang::ir::{Behavior, CTX_FIELDS, Effect};
use crate::lang::vm::{BehaviorIo, CTX_LEN};
use crate::terrain::WATER_LEVEL;
use crate::world::TypeEntry;
use glam::Vec3;
use serde_json::json;
use std::sync::Arc;

/// Spawns per thing per minute, at most.
const SPAWN_COOLDOWN: f64 = 6.0;

fn ctx_set(ctx: &mut [f32; CTX_LEN], name: &str, v: f32) {
    if let Some(i) = CTX_FIELDS.iter().position(|f| *f == name) {
        ctx[i] = v;
    }
}

impl Sim {
    /// Map a type's behaviour property names to vocabulary ids.
    fn prop_ids(&self, ty: &TypeEntry) -> Vec<Option<usize>> {
        ty.ct.prop_names.iter().map(|n| self.vocab.id(n)).collect()
    }

    fn ctx_for(&self, id: ThingId, dt: f32) -> [f32; CTX_LEN] {
        let mut c = [0.0; CTX_LEN];
        let Some(t) = self.things.get(id) else { return c };
        let near = self.actor_ids().iter().filter(|a| self.actor(**a).is_some_and(|x| (x.pos - t.pos).length() < 4.0)).count();
        ctx_set(&mut c, "dt", dt);
        ctx_set(&mut c, "hour", self.hour());
        ctx_set(&mut c, "age", (self.t - t.born) as f32);
        ctx_set(&mut c, "held", t.held() as u32 as f32);
        ctx_set(&mut c, "near", near as f32);
        ctx_set(&mut c, "speed", t.vel.length());
        ctx_set(&mut c, "ground", (!t.held() && t.in_contact) as u32 as f32);
        ctx_set(&mut c, "water", (t.pos.y < WATER_LEVEL) as u32 as f32);
        c
    }

    /// Run one behaviour entry point. Returns false if the type has none.
    fn run_entry(&mut self, id: ThingId, b: Behavior, mut ctx: [f32; CTX_LEN], other: Option<ThingId>) -> bool {
        let Some(t) = self.things.get(id) else { return false };
        let Some(ty) = self.snap.type_of(t.type_id).cloned() else { return false };
        if ty.ct.behavior(b).is_none() {
            return false;
        }
        let ids = self.prop_ids(&ty);
        let mut props: Vec<f32> = ids.iter().map(|i| i.map(|i| t.props[i]).unwrap_or(0.0)).collect();
        let mut oprops: Vec<f32> = match other.and_then(|o| self.things.get(o)) {
            Some(o) => ids.iter().map(|i| i.map(|i| o.props[i]).unwrap_or(0.0)).collect(),
            None => vec![0.0; ids.len()],
        };
        if other.is_some() {
            ctx_set(&mut ctx, "on", 1.0);
        }
        let mut k = [0.0f32; 16];
        k[..8].copy_from_slice(&t.params);
        k[1] = t.scale;
        k[8..].copy_from_slice(&t.state);
        let mut state = t.state;
        let mut effects = Vec::new();
        let r = ty.ct.run_behavior(b, BehaviorIo { k: &k, state: &mut state, ctx: &ctx, props: &mut props, other: &mut oprops, effects: &mut effects }, BEHAVIOR_FUEL);
        if let Some(Err(e)) = r {
            crate::log::error(format!("{}() of {} failed: {e:?}", b.name(), ty.name()));
            return true;
        }
        let tnow = self.t;
        if let Some(t) = self.things.get_mut(id) {
            for (v, s) in state.iter().zip(t.state.iter_mut()) {
                if v.is_finite() {
                    *s = v.clamp(-1e6, 1e6);
                }
            }
            for (i, pid) in ids.iter().enumerate() {
                if let Some(p) = pid {
                    t.props[*p] = props[i];
                }
            }
            sanitize(&mut t.props);
            t.dirty = true;
            t.last_tick = tnow;
        }
        if let Some(o) = other {
            if let Some(ot) = self.things.get_mut(o) {
                for (i, pid) in ids.iter().enumerate() {
                    if let Some(p) = pid {
                        ot.props[*p] = oprops[i];
                    }
                }
                sanitize(&mut ot.props);
                ot.dirty = true;
                ot.asleep = false;
            }
        }
        for (e, arg) in effects {
            self.apply_effect(id, &ty, e, arg);
        }
        true
    }

    /// Tick behaviour code of live things near the player (slower further out).
    pub fn step_behavior(&mut self, dt: f32) {
        let near = self.cfg.near;
        let medium = self.cfg.medium;
        let base = 1.0 / self.cfg.behavior_hz as f64;
        let ids: Vec<(ThingId, f64)> = self
            .things
            .live()
            .filter(|t| self.snap.type_of(t.type_id).is_some_and(|ty| ty.ct.tick.is_some()))
            .filter_map(|t| {
                let d = self.dist_to_player(t.pos);
                if d > medium {
                    return None;
                }
                let every = if d <= near { base } else { base * 4.0 };
                (self.t - t.last_tick >= every - 1e-6).then_some((t.id, self.t - t.last_tick))
            })
            .collect();
        for (id, since) in ids {
            let ctx = self.ctx_for(id, (since as f32).min(dt.max(since as f32)).min(5.0));
            self.run_entry(id, Behavior::Tick, ctx, None);
        }
    }

    /// Someone uses thing `a` (optionally on `b`). True if it has `use` code.
    pub fn run_use(&mut self, a: ThingId, b: Option<ThingId>, _who: ActorId) -> bool {
        let ctx = self.ctx_for(a, 0.0);
        self.run_entry(a, Behavior::Use, ctx, b)
    }

    /// Something hit thing `id` at `speed`.
    pub fn run_touch(&mut self, id: ThingId, speed: f32) {
        let mut ctx = self.ctx_for(id, 0.0);
        ctx_set(&mut ctx, "impact", speed);
        self.run_entry(id, Behavior::Touch, ctx, None);
    }

    fn apply_effect(&mut self, id: ThingId, ty: &Arc<TypeEntry>, e: Effect, arg: f32) {
        let Some(t) = self.things.get(id) else { return };
        let pos = t.pos;
        let i = if arg.is_finite() && arg >= 0.0 { arg.round() as usize } else { usize::MAX };
        let meta = &ty.ct.meta;
        let name = ty.name().to_string();
        match e {
            Effect::Say => {
                let Some(line) = meta.says.get(i).cloned() else { return };
                self.note_near(pos, 20.0, Note::Line { who: super::physics::cap(&name), text: line.clone() });
                self.event("said", None, Some(format!("thing:{id}")), format!("the {name} said: \"{line}\""), Some(pos), json!({}));
            }
            Effect::Sound => {
                let Some(s) = meta.sounds.get(i).cloned() else { return };
                self.note_near(pos, 25.0, Note::Info(format!("*{s}* (the {name})")));
                self.event("sound", None, Some(format!("thing:{id}")), format!("the {name} went \"{s}\""), Some(pos), json!({}));
            }
            Effect::Remove => {
                self.release(id);
                self.things.remove(id);
                self.event("vanished", None, Some(format!("thing:{id}")), format!("the {name} is gone"), Some(pos), json!({}));
            }
            Effect::Spawn | Effect::Transform => {
                let Some(what) = meta.spawns.get(i).cloned() else { return };
                if e == Effect::Spawn {
                    let ready = self.things.get(id).is_some_and(|t| self.t >= t.cooldown);
                    if !ready {
                        return;
                    }
                    if let Some(t) = self.things.get_mut(id) {
                        t.cooldown = self.t + SPAWN_COOLDOWN;
                    }
                }
                match self.type_by_name(&what) {
                    Some(nt) => self.spawn_or_transform(id, nt.id, e == Effect::Transform, &name),
                    None => self.build_type_for(&what, &format!("something a {name} makes or turns into"), Some((id, e == Effect::Transform))),
                }
            }
        }
    }

    /// Spawn a `type_id` next to thing `id`, or turn `id` into it.
    pub fn spawn_or_transform(&mut self, id: ThingId, type_id: u32, transform: bool, from_name: &str) {
        let Some(t) = self.things.get(id).cloned() else { return };
        let Some((nty, base)) = self.type_info(type_id) else { return };
        let nname = nty.name().to_string();
        if transform {
            let keep = [P_TEMP, P_WET, P_CHAR, P_FIRE];
            let mut props = scaled((*base).clone(), t.scale);
            for k in keep {
                props[k] = t.props[k];
            }
            let g = self.snap.terrain.height(t.pos.x, t.pos.z);
            if let Some(x) = self.things.get_mut(id) {
                x.type_id = type_id;
                x.props = props;
                x.state = [0.0; 8];
                x.anchored = x.props[P_MASS] >= ANCHOR_MASS || nty.has_tag("building");
                if !x.held() {
                    x.pos.y = x.rest_y(&nty, g);
                }
                x.origin.made_from = vec![from_name.to_string()];
                x.dirty = true;
            }
            self.event("transformed", None, Some(format!("thing:{id}")), format!("the {from_name} became {} {nname}", super::article(&nname)), Some(t.pos), json!({ "into": nname }));
            self.note_near(t.pos, 30.0, Note::Info(format!("The {from_name} becomes {} {nname}.", super::article(&nname))));
            self.witness(t.pos, 20.0, &format!("I saw the {from_name} turn into {} {nname}.", super::article(&nname)), 0.4, &[]);
        } else {
            let a = self.rand() * std::f32::consts::TAU;
            let at = t.pos + Vec3::new(a.cos(), 0.0, a.sin()) * (0.5 + t.scale * 0.3);
            let origin = Origin { made_from: vec![from_name.to_string()], ..Default::default() };
            if let Some(nid) = self.spawn_thing(type_id, at, a, 1.0, origin, true) {
                self.event("spawned", None, Some(format!("thing:{nid}")), format!("the {from_name} made {} {nname}", super::article(&nname)), Some(at), json!({ "from": id }));
                self.note_near(at, 20.0, Note::Info(format!("The {from_name} makes {} {nname}.", super::article(&nname))));
            }
        }
    }

    /// Ask the builder for a type nobody has written yet (once per name).
    pub fn build_type_for(&mut self, name: &str, description: &str, then: Option<(ThingId, bool)>) {
        let key = name.trim().to_lowercase();
        if key.is_empty() || !self.has_llm {
            return;
        }
        if let Some(rid) = self.interp.building_names.get(&key).copied() {
            if let (Some(t), Some(b)) = (then, self.interp.building.get_mut(&rid)) {
                b.then.push(t);
            }
            return;
        }
        let id = self.next_id();
        self.interp.building_names.insert(key.clone(), id);
        self.interp.building.insert(id, super::interp::PendingBuild { name: name.to_string(), then: then.into_iter().collect(), ..Default::default() });
        let at = then.and_then(|(t, _)| self.things.get(t).map(|x| x.pos)).unwrap_or(self.player.pos);
        self.request(Request::BuildType { id, name: name.to_string(), description: description.to_string(), size: [0.6, 0.6, 0.6], props: vec![], fits: None }, at);
    }
}
