//! Toy physics: gravity, velocity, bounce and friction for loose things.
//! Things are spheres against the terrain and against the signed distance
//! fields of nearby solids, so a ball bounces off any shape the LLM wrote
//! with no extra work. Things at rest sleep and cost nothing.

use super::props::*;
use super::things::ThingId;
use super::{ActorId, Note, Sim, Target};
use crate::terrain::WATER_LEVEL;
use crate::world::Solid;
use glam::Vec3;
use serde_json::json;

pub const GRAVITY: f32 = 9.81;
/// Below this impact speed (m/s) nothing gets hurt.
pub const IMPACT_SAFE: f32 = 2.5;

/// The outward normal of a solid's surface at `p` (finite differences).
fn solid_normal(s: &Solid, p: Vec3) -> Vec3 {
    let e = 0.02;
    let n = Vec3::new(
        s.sdf(p + Vec3::X * e) - s.sdf(p - Vec3::X * e),
        s.sdf(p + Vec3::Y * e) - s.sdf(p - Vec3::Y * e),
        s.sdf(p + Vec3::Z * e) - s.sdf(p - Vec3::Z * e),
    );
    n.try_normalize().unwrap_or(Vec3::Y)
}

/// Is `p` ringed by the shape: walking out horizontally in most directions,
/// do you run into it within `reach`?
fn enclosed(s: &Solid, p: Vec3, reach: f32) -> bool {
    let mut hits = 0;
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        let mut t = 0.02;
        for _ in 0..12 {
            let d = s.sdf(p + dir * t);
            if d < 0.01 {
                hits += 1;
                break;
            }
            t += d.max(0.01);
            if t > reach {
                break;
            }
        }
    }
    hits >= 6
}

/// Distance from a point to an actor's body (a vertical capsule).
fn body_dist(p: Vec3, feet: Vec3, height: f32) -> (f32, Vec3) {
    // A person's capsule (0.3 m round, from 0.3 m to 0.25 m below the top),
    // in proportion for smaller and bigger bodies.
    let s = height / 1.75;
    let y = p.y.clamp(feet.y + 0.3 * s, feet.y + height - 0.25 * s);
    let c = Vec3::new(feet.x, y, feet.z);
    let d = p - c;
    let l = d.length();
    (l - 0.3 * s, if l > 1e-5 { d / l } else { Vec3::Y })
}

impl Sim {
    pub fn step_physics(&mut self, dt: f32) {
        self.update_held();
        let mut ids: Vec<(f32, ThingId)> = self
            .things
            .live()
            .filter(|t| !t.asleep && !t.anchored && t.holder.is_none() && t.worn.is_none())
            .map(|t| (self.dist_to_player(t.pos), t.id))
            .collect();
        if ids.is_empty() {
            return;
        }
        ids.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let max = self.cfg.max_awake;
        let medium = self.cfg.medium;
        let sub = ((dt * 90.0).ceil() as usize).clamp(1, 8);
        let h = dt / sub as f32;
        for (i, (d, id)) in ids.into_iter().enumerate() {
            if i >= max || d > medium {
                // Too many in motion, or too far: freeze where it is.
                if let Some(t) = self.things.get_mut(id) {
                    t.asleep = true;
                    t.vel = Vec3::ZERO;
                }
                continue;
            }
            for _ in 0..sub {
                if !self.integrate(id, h) {
                    break;
                }
            }
        }
        self.things.moved();
    }

    /// Worn layers go where their wearer goes (drawn in the wearer's pose).
    fn update_worn(&mut self) {
        let worn: Vec<(ThingId, ActorId)> = self.things.live().filter_map(|t| t.worn.map(|w| (t.id, w))).collect();
        for (id, w) in worn {
            match self.actor(w).map(|a| (a.pos, a.yaw)) {
                Some((p, yaw)) => {
                    if let Some(t) = self.things.get_mut(id) {
                        t.pos = p;
                        t.yaw = yaw;
                        t.vel = Vec3::ZERO;
                        t.asleep = true;
                    }
                }
                None => {
                    if let Some(t) = self.things.get_mut(id) {
                        t.worn = None;
                        t.asleep = false;
                    }
                }
            }
        }
    }

    /// Held things follow their holders' hands.
    fn update_held(&mut self) {
        self.update_worn();
        let held: Vec<(ThingId, ActorId, Option<ActorId>)> = self.things.live().filter_map(|t| t.holder.map(|h| (t.id, h, t.co_holder))).collect();
        for (id, h, co) in held {
            let Some(t) = self.things.get(id) else { continue };
            let Some(ty) = self.snap.type_of(t.type_id).cloned() else { continue };
            // Too heavy for one: gripped, not lifted, until a second pair of hands comes.
            if co.is_none() && t.mass() > self.strength(h) {
                let tp = t.pos;
                if self.actor(h).is_none_or(|a| (a.pos - tp).length() > 3.0) {
                    self.release(id);
                }
                continue;
            }
            let big = ty.radius() * t.scale > 0.35 || co.is_some();
            let hand = |s: &Sim, a: ActorId| s.actor(a).map(|x| x.hand(big));
            let Some(mut p) = hand(self, h) else {
                self.release(id);
                continue;
            };
            let yaw = self.actor(h).map(|a| a.yaw).unwrap_or(0.0);
            if let Some(c) = co {
                match hand(self, c) {
                    Some(q) => p = (p + q) * 0.5,
                    None => {
                        if let Some(t) = self.things.get_mut(id) {
                            t.co_holder = None;
                        }
                    }
                }
            }
            if let Some(t) = self.things.get_mut(id) {
                let (c0, _) = t.proxy(&ty);
                let off = c0 - t.pos;
                let target = p - off;
                t.vel = (target - t.pos) / (1.0 / 60.0);
                t.vel = t.vel.clamp_length_max(30.0);
                t.pos = target;
                t.yaw = yaw;
                t.asleep = false;
                t.rest_t = 0.0;
            }
        }
        // Carriers who drift too far apart let go (a long thing can be held
        // by its two ends).
        let pairs: Vec<(ThingId, ActorId, ActorId, f32)> = self
            .things
            .live()
            .filter_map(|t| {
                let b = self.snap.type_of(t.type_id).map(|ty| ty.ct.meta.bounds).unwrap_or_default();
                Some((t.id, t.holder?, t.co_holder?, 3.0 + 2.0 * b[0].max(b[2]) * t.scale))
            })
            .collect();
        for (id, a, b, apart) in pairs {
            let (Some(pa), Some(pb)) = (self.actor(a).map(|x| x.pos), self.actor(b).map(|x| x.pos)) else { continue };
            if (pa - pb).length() > apart {
                let name = self.thing_name(id);
                self.drop_thing(id, Vec3::ZERO);
                self.event("dropped", Some(a), Some(format!("thing:{id}")), format!("{} and {} lost their grip on the {name}", self.actor_name(a), self.actor_name(b)), Some(pa), json!({}));
            }
        }
    }

    /// One physics substep for one thing. Returns false once it sleeps.
    fn integrate(&mut self, id: ThingId, h: f32) -> bool {
        let Some(t) = self.things.get(id) else { return false };
        let Some(ty) = self.snap.type_of(t.type_id).cloned() else { return false };
        let (c, r) = t.proxy(&ty);
        let off = c - t.pos;
        let mut v = t.vel;
        let mass = t.mass();
        let bounce = t.props[P_BOUNCE];
        let friction = t.props[P_FRICTION];
        let thrown = t.thrown_by;
        let old_pos = t.pos;
        v.y -= GRAVITY * h;
        // Water: float if lighter than water, drag either way.
        if c.y < WATER_LEVEL {
            let vol = volume(&ty) * t.scale.powi(3);
            let density = mass / vol.max(1e-4);
            let depth = ((WATER_LEVEL - c.y) / r.max(0.05)).min(1.0);
            v.y += GRAVITY * depth * (1000.0 / density.max(50.0)).min(4.0) * h;
            v *= 1.0 - (2.5 * h).min(0.5);
        }
        let mut p = c + v * h;
        let mut contact = false;
        let mut impact = 0.0f32;
        let mut hit_name: Option<String> = None;
        // Terrain.
        let ground = self.snap.terrain.height(p.x, p.z);
        let mut grip = false;
        if p.y - r < ground {
            let n = self.snap.terrain.normal(p.x, p.z);
            p.y = ground + r;
            let vn = v.dot(n);
            if vn < 0.0 {
                impact = impact.max(-vn);
                // Slow landings don't bounce (no endless jitter).
                let e = if -vn < 1.0 { 0.0 } else { bounce };
                v -= (1.0 + e) * vn * n;
            }
            contact = true;
            // Things stay put on slopes gentler than their grip (round,
            // bouncy things roll on).
            let tan = (1.0 - n.y * n.y).max(0.0).sqrt() / n.y.max(0.05);
            grip = tan < friction * (1.25 - bounce).max(0.1);
        }
        // Solids.
        let solids = self.solids_near(p, r + 0.6);
        for s in &solids {
            // Its own shape is not an obstacle.
            if (s.inst.pos() - old_pos).length() < 1e-3 && s.ty.id == ty.id {
                continue;
            }
            let d = s.sdf(p);
            if d < r {
                let n = solid_normal(s, p);
                p += n * (r - d + 1e-3);
                let vn = v.dot(n);
                if vn < 0.0 {
                    if -vn > impact {
                        impact = -vn;
                        hit_name = Some(s.ty.name().to_string());
                    }
                    v -= (1.0 + bounce) * vn * n;
                }
                contact = true;
            } else if v.y < -0.5 && !s.ty.builtin && d > r && thrown.is_some() {
                // Falling through an opening of something (a hoop, a well, a basket…):
                // the thing is surrounded by the shape on most sides.
                let lp = s.inst.to_local(p);
                let b = s.ty.ct.meta.bounds;
                if lp[0].abs() < b[0] && lp[2].abs() < b[2] && lp[1].abs() < b[1] && self.things.get(id).is_some_and(|t| t.through != Some(s.ty.id)) {
                    let reach = (b[0].max(b[2]) * s.inst.pos_scale[3]).min(2.0);
                    if enclosed(s, p, reach) {
                        let name = s.ty.name().to_string();
                        if let Some(t) = self.things.get_mut(id) {
                            t.through = Some(s.ty.id);
                        }
                        self.event("through", thrown.map(|x| x.0), Some(format!("thing:{id}")), format!("the {} went through the {name}", ty.name()), Some(p), json!({ "target": name }));
                        self.note_near(p, 25.0, Note::Ambient(format!("The {} drops through the {name}!", ty.name())));
                        self.witness(p, 25.0, &format!("I saw the {} go through the {name}.", ty.name()), 0.35, &[]);
                        if let Some((who, _)) = thrown {
                            self.on_scored(who, &name);
                        }
                    }
                }
            }
        }
        // People.
        let actors = self.actor_ids();
        for a in actors {
            let Some(body) = self.actor(a).map(|x| (x.pos, x.catching)) else { continue };
            if self.things.get(id).is_some_and(|t| t.holder == Some(a)) {
                continue;
            }
            let height = self.actor_height(a);
            let (d, n) = body_dist(p, body.0, height);
            if d < r + 0.25 && body.1 > self.t && v.length() > 0.5 && self.actor(a).is_some_and(|x| x.held.is_none()) && mass <= self.strength(a) {
                // Caught.
                let thrower = thrown.map(|x| x.0);
                if let Some(t) = self.things.get_mut(id) {
                    t.holder = Some(a);
                    t.asleep = false;
                    t.thrown_by = None;
                    t.dirty = true;
                }
                if let Some(x) = self.actor_mut(a) {
                    x.held = Some(id);
                    x.catching = 0.0;
                }
                let name = ty.name().to_string();
                let who = self.actor_name(a);
                self.event("caught", Some(a), Some(format!("thing:{id}")), format!("{who} caught the {name}"), Some(p), json!({ "from": thrower }));
                self.note_near(p, 30.0, Note::Ambient(format!("{} caught the {name}.", cap(&who))));
                self.on_caught(a, thrower, id);
                return false;
            }
            if d < r {
                p += n * (r - d + 1e-3);
                let vn = v.dot(n);
                if vn < 0.0 {
                    if -vn > impact {
                        impact = -vn;
                        hit_name = Some(self.actor_name(a));
                    }
                    v -= (1.0 + bounce * 0.5) * vn * n;
                }
            }
        }
        // Rolling and sliding.
        if contact {
            let k = (friction * 6.0 * h).min(1.0);
            let vy = v.y;
            v *= 1.0 - k;
            v.y = vy;
            if grip && v.length() < 1.2 {
                v *= 1.0 - (14.0 * h).min(1.0);
            }
        }
        let speed = v.length();
        let mut sleep = false;
        if let Some(t) = self.things.get_mut(id) {
            t.pos = p - off;
            t.vel = v;
            t.in_contact = contact;
            if contact {
                t.spin = Vec3::new(v.x, 0.0, v.z).length() / r.max(0.05) * 0.3;
            }
            t.yaw += t.spin * h;
            t.dirty = true;
            if contact && speed < 0.15 {
                t.rest_t += h;
                if t.rest_t > 0.4 {
                    t.asleep = true;
                    t.vel = Vec3::ZERO;
                    t.spin = 0.0;
                    t.thrown_by = None;
                    let g = self.snap.terrain.height(t.pos.x, t.pos.z);
                    let ry = t.rest_y(&ty, g);
                    if (t.pos.y - ry).abs() < r + 0.1 && t.pos.y - ry < 0.3 {
                        t.pos.y = t.pos.y.max(ry);
                    }
                    sleep = true;
                }
            } else {
                t.rest_t = 0.0;
            }
        }
        if impact > IMPACT_SAFE {
            self.impact(id, impact, hit_name.clone());
        }
        self.push_neighbours(id, p, r, v);
        !sleep
    }

    /// Small things in the way get knocked.
    fn push_neighbours(&mut self, id: ThingId, p: Vec3, r: f32, v: Vec3) {
        let near = self.things.near(p, r + 1.5);
        for o in near {
            if o == id {
                continue;
            }
            let Some(ot) = self.things.get(o) else { continue };
            if ot.anchored || ot.held() {
                continue;
            }
            let Some(oty) = self.snap.type_of(ot.type_id).cloned() else { continue };
            let (oc, or) = ot.proxy(&oty);
            let d = (oc - p).length();
            if d >= r + or || d < 1e-5 {
                continue;
            }
            let n = (oc - p) / d;
            let my_mass = self.things.get(id).map(|t| t.mass()).unwrap_or(1.0);
            let om = ot.mass();
            let rel = v.dot(n) - ot.vel.dot(n);
            let push = r + or - d;
            let share = my_mass / (my_mass + om);
            if let Some(t) = self.things.get_mut(o) {
                t.pos += n * push * share;
                if rel > 0.0 {
                    t.vel += n * rel * share * 1.6;
                }
                if t.vel.length() > 0.2 {
                    t.asleep = false;
                    t.rest_t = 0.0;
                }
                t.dirty = true;
            }
            if let Some(t) = self.things.get_mut(id) {
                t.pos -= n * push * (1.0 - share);
                if rel > 0.0 {
                    t.vel -= n * rel * (1.0 - share) * 1.6;
                }
            }
        }
    }

    /// Something hit something at `speed` m/s.
    pub fn impact(&mut self, id: ThingId, speed: f32, with: Option<String>) {
        let Some(t) = self.things.get(id) else { return };
        let fragile = t.props[P_FRAGILE];
        let pos = t.pos;
        let (type_id, props) = (t.type_id, t.props.clone());
        let name = self.thing_name(id);
        if fragile > 0.0 {
            if let Some(t) = self.things.get_mut(id) {
                t.props[P_HEALTH] -= fragile * (speed - IMPACT_SAFE) * 0.4;
            }
        }
        if speed > 1.2 {
            if let Some(ty) = self.snap.type_of(type_id).cloned() {
                let mass = props[P_MASS];
                let mat = crate::audio::call::Material::of(&props, &ty.ct.meta.tags, crate::audio::call::Material::from_meta(&ty.ct.meta.sound).as_ref());
                self.cue(pos, None, crate::audio::Heard::Hit { mat, mass, speed, by: None });
            }
        }
        self.run_touch(id, speed);
        if speed > 4.0 {
            let what = with.clone().unwrap_or_else(|| "the ground".into());
            let by = self.things.get(id).and_then(|t| t.thrown_by.map(|x| x.0));
            self.event("hit", by, Some(format!("thing:{id}")), format!("the {name} hit {what} at {speed:.0} m/s"), Some(pos), json!({ "with": with, "speed": (speed as f64 * 10.0).round() / 10.0 }));
        }
        if self.things.get(id).is_some_and(|t| t.props[P_HEALTH] <= 0.0 && t.props[P_FRAGILE] > 0.0) {
            self.break_thing(id);
        }
    }

    /// A fragile thing breaks into pieces that keep its properties (a broken
    /// lamp is still hot and still full of oil).
    pub fn break_thing(&mut self, id: ThingId) {
        let Some(t) = self.things.get(id).cloned() else { return };
        let Some(ty) = self.type_entry(t.type_id) else { return };
        self.release(id);
        self.things.remove(id);
        let n = 2 + (self.rand() * 2.0) as usize;
        for i in 0..n {
            if self.things.len() >= self.cfg.max_things {
                break;
            }
            let a = i as f32 * 2.4 + self.rand();
            let nid = self.things.alloc();
            let mut p = t.props.clone();
            p[P_MASS] /= n as f32;
            p[P_FRAGILE] = 0.0;
            p[P_HEALTH] = 0.0;
            let mut piece = super::things::Thing::new(nid, &ty, t.pos + Vec3::new(a.cos() * 0.15, 0.05, a.sin() * 0.15), t.yaw + a, t.scale * 0.55, t.params, p, self.t);
            piece.params[0] = t.params[0] + i as f32 + 1.0;
            piece.vel = t.vel * 0.4 + Vec3::new(a.cos() * 1.2, 1.5, a.sin() * 1.2);
            piece.asleep = false;
            piece.anchored = false;
            piece.origin.made_from = vec![ty.name().to_string()];
            // Whoever threw it threw its pieces (a smashed lamp's fire is theirs).
            piece.thrown_by = t.thrown_by.or(t.holder.map(|h| (h, self.t)));
            self.things.insert(piece);
        }
        let name = ty.name().to_string();
        self.event("broke", None, Some(format!("thing:{id}")), format!("the {name} broke into {n} pieces"), Some(t.pos), json!({ "pieces": n }));
        self.note_near(t.pos, 30.0, Note::Info(format!("The {name} breaks!")));
        self.witness(t.pos, 25.0, &format!("I saw the {name} break."), 0.4, &[]);
    }

    /// Let go of a thing with a velocity (0 = just drop it).
    pub fn drop_thing(&mut self, id: ThingId, vel: Vec3) {
        self.release(id);
        if let Some(t) = self.things.get_mut(id) {
            t.vel = vel;
            t.asleep = false;
            t.rest_t = 0.0;
            t.dirty = true;
        }
        self.things.moved();
    }

    /// Walking into small loose things nudges them along.
    pub fn kick_things(&mut self, who: ActorId, step: Vec3) {
        let Some(p) = self.actor(who).map(|a| a.pos) else { return };
        if step.length() < 1e-4 {
            return;
        }
        // Not what they are walking up to (to pick it up).
        let aim = match self.actor(who).and_then(|a| a.task.clone()) {
            Some(super::actor::Task::Goto { target: Target::Point(q), .. }) => Some(Vec3::from_array(q)),
            _ => None,
        };
        let strength = self.strength(who);
        for id in self.things.near(p, 0.9) {
            let Some(t) = self.things.get(id) else { continue };
            if t.anchored || t.held() || t.mass() > strength * 0.4 {
                continue;
            }
            let Some(ty) = self.snap.type_of(t.type_id).cloned() else { continue };
            let (c, r) = t.proxy(&ty);
            let d = Vec3::new(c.x - p.x, 0.0, c.z - p.z);
            if d.length() > r + 0.35 || c.y > p.y + 1.0 {
                continue;
            }
            if aim.is_some_and(|q| Vec3::new(q.x - t.pos.x, 0.0, q.z - t.pos.z).length() < 1.2) {
                continue;
            }
            let dir = (d.normalize_or_zero() + step.normalize_or_zero()).normalize_or_zero();
            if let Some(t) = self.things.get_mut(id) {
                t.vel = dir * (2.5 + step.length() * 30.0).min(6.0) + Vec3::Y * 1.2;
                t.asleep = false;
                t.rest_t = 0.0;
                t.thrown_by = Some((who, self.t));
                t.dirty = true;
            }
        }
    }

    /// Whether `a` can reach point `p`.
    pub fn in_reach(&self, a: ActorId, p: Vec3) -> bool {
        self.actor(a).is_some_and(|x| {
            let d = p - x.pos;
            Vec3::new(d.x, 0.0, d.z).length() <= x.dims.reach && d.y > -1.5 && d.y < x.dims.height * 1.5
        })
    }
}

pub fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
