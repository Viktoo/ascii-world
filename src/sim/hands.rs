//! The traveler's own body, kept as every other body is: a body type with
//! look sliders (skin, shirt, build…, kept with the world and changed by
//! deeds done to oneself), hands that move with what is done (a fist
//! thrown, a hand reaching to take, an arm throwing, both hands raised while
//! something is being conjured), and how it is seen from inside: the body
//! drawn about the eye, head and chest cut away, so arms come into view only
//! while they are at work.

use super::actions::Action;
use super::actor::{Actor, CROUCH, GestureKind, GestureRun, LEAN, L_FWD, NOD, Pose, R_FWD};
use super::{ActorId, Sim};
use crate::render::{Camera, GpuInst};
use crate::world::TypeEntry;
use crate::world::species::LookMap;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const KV: &str = "player.body";

/// What the traveler chose about their body; the rest is their people's.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct OwnBody {
    /// Look sliders by name (skin, shirt, build…).
    #[serde(default)]
    pub look: LookMap,
}

/// How far the body seen from inside is turned up about the eye (radians),
/// so hands held before it come into the lower part of the view.
const LIFT: f32 = 0.24;

/// A look slider's value for the traveler when they haven't chosen one.
fn usual(name: &str, lo: f32, hi: f32) -> f32 {
    match name {
        "build" => 1.0,
        "skin" => 0.3,
        "shirt" | "top" => 0.6,
        "trousers" | "bottom" => 0.1,
        "shape" => 0.2,
        "hair" => 0.3,
        _ => (lo + hi) * 0.5,
    }
}

/// Words that make a deed in words a blow with the fist.
const BLOWS: [&str; 9] = ["punch", "hit", "strike", "smash", "slap", "thump", "whack", "bash", "knock"];

/// The pose a body seen from inside is drawn in: an arm at work (holding,
/// reaching, swinging) is brought up before the eye, as one holds what one
/// looks at; an arm at rest stays down, out of sight. The body stays upright
/// under the eye (the view itself leans and crouches).
fn view_pose(p: &Pose) -> Pose {
    let mut v = *p;
    v[LEAN] = 0.0;
    v[NOD] = 0.0;
    v[CROUCH] = 0.0;
    for i in [L_FWD, R_FWD] {
        let r = p[i];
        let k = ((r - 0.15) / 0.3).clamp(0.0, 1.0);
        let k = k * k * (3.0 - 2.0 * k);
        v[i] = r + (0.9 - r).max(0.0) * k;
    }
    v
}

/// The traveler's body as seen from inside, this frame.
pub struct OwnView {
    pub inst: GpuInst,
    pub body: Arc<TypeEntry>,
    /// The body's posed stand-in (for where its hands are).
    actor: Actor,
    /// Body frame → world: turn, and where its origin is.
    rot: Quat,
    origin: Vec3,
}

impl OwnView {
    /// A hand (or both, for something big), in the world.
    pub fn hand(&self, big: bool) -> Vec3 {
        self.origin + self.rot * self.actor.hand_offset(big)
    }

    /// How far apart the two hands are.
    pub fn hand_gap(&self) -> f32 {
        let r = self.actor.hand_offset(false);
        Vec3::new(r.x * 2.0, 0.0, 0.0).length().max(0.1)
    }

    /// The way the body faces (its forward, turned with the view).
    pub fn forward(&self) -> Vec3 {
        self.rot * Vec3::Z
    }

    pub fn up(&self) -> Vec3 {
        self.rot * Vec3::Y
    }
}

impl Sim {
    /// Read the traveler's own body from the world's store.
    pub(super) fn load_own_body(&mut self) {
        if let Some(b) = self.db.kv_get(KV).and_then(|s| serde_json::from_str::<OwnBody>(&s).ok()) {
            self.own = b;
        }
    }

    /// The body type the traveler lives in: their people's.
    pub fn own_body_type(&self) -> Option<&Arc<TypeEntry>> {
        let human = self.snap.species.of("human");
        self.snap.body_type(&human.body)
    }

    /// The traveler's look sliders (k.a … k.e of their body).
    pub fn own_sliders(&self) -> [f32; crate::lang::ir::MAX_SLIDERS] {
        let mut out = [0.5f32; crate::lang::ir::MAX_SLIDERS];
        let Some(b) = self.own_body_type().and_then(|t| t.ct.meta.body.clone()) else { return out };
        let d = &self.player.dims;
        for (i, (name, lo, hi)) in b.look.iter().enumerate().take(crate::lang::ir::MAX_SLIDERS) {
            let v = crate::world::species::look_value(&self.own.look, name).unwrap_or_else(|| match name.as_str() {
                // People draw their own height: the traveler's, at the world's size.
                "height" => d.height / d.scale.max(1e-3),
                _ => b.usual(name).unwrap_or_else(|| usual(name, *lo, *hi)),
            });
            out[i] = v.clamp(lo.min(*hi), hi.max(*lo));
        }
        out
    }

    /// Change how the traveler looks (sliders by name, within their ranges);
    /// kept with the world.
    pub fn set_own_look(&mut self, look: &LookMap) {
        for (k, v) in look {
            if v.is_finite() {
                self.own.look.insert(k.trim().to_lowercase(), *v);
            }
        }
        if let Ok(s) = serde_json::to_string(&self.own) {
            let _ = self.db.kv_set(KV, &s);
        }
    }

    /// The traveler's look sliders as the interpreter is shown them.
    pub(super) fn own_look_sliders(&self) -> serde_json::Value {
        let now = self.own_sliders();
        let Some(b) = self.own_body_type().and_then(|t| t.ct.meta.body.clone()) else { return serde_json::Value::Null };
        let m: serde_json::Map<String, serde_json::Value> = b.look.iter().enumerate().take(crate::lang::ir::MAX_SLIDERS).map(|(i, (name, lo, hi))| (name.clone(), serde_json::json!({ "now": now[i], "range": [lo, hi] }))).collect();
        serde_json::Value::Object(m)
    }

    /// What the hands do for an action, if anything shows.
    pub(super) fn hand_move(&self, who: ActorId, a: &Action) -> Option<GestureKind> {
        let empty = self.actor(who).is_some_and(|x| x.held.is_none());
        Some(match a {
            Action::Hold { .. } | Action::Eat { .. } => GestureKind::Grab,
            Action::Throw { .. } => GestureKind::Throw,
            // A tool's own motion moves the arm.
            Action::Use { .. } if self.held_tool(who).is_some() => return None,
            Action::Use { .. } | Action::Apply { .. } | Action::Give { .. } | Action::Open { .. } => GestureKind::Touch,
            Action::Do { text, .. } => {
                let first = text.split_whitespace().next().unwrap_or("").to_lowercase();
                if empty && BLOWS.iter().any(|b| first.starts_with(b)) { GestureKind::Punch } else { GestureKind::Touch }
            }
            _ => return None,
        })
    }

    /// Play a hand move, unless the body is in the middle of something
    /// more (a hug, a dance, sitting).
    pub(super) fn play_hands(&mut self, who: ActorId, k: GestureKind) {
        let t = self.t;
        let Some(a) = self.actor_mut(who) else { return };
        let busy = a.gesture.as_ref().is_some_and(|g| !matches!(g.kind, GestureKind::Punch | GestureKind::Grab | GestureKind::Throw | GestureKind::Touch));
        if !busy {
            a.gesture = Some(GestureRun { kind: k, with: None, t0: t, dur: k.duration() });
        }
    }

    /// Who is conjuring holds their hands up to it until it comes.
    pub(super) fn step_hands(&mut self) {
        let mut who: Vec<ActorId> = self.work().into_iter().filter(|w| w.appears).filter_map(|w| w.who).collect();
        who.dedup();
        self.player.conjuring = who.contains(&ActorId::Player);
        for n in self.cast.npcs.iter_mut() {
            n.a.conjuring = who.contains(&ActorId::Npc(n.def.id));
        }
    }

    /// The traveler's body seen from the eye at `cam`: drawn about the
    /// eye, turned with the view, head and chest cut away.
    pub fn own_view(&self, cam: &Camera) -> Option<OwnView> {
        if self.fallen() {
            return None;
        }
        let body = self.own_body_type()?.clone();
        let mut a = self.player.clone();
        a.pose = view_pose(&a.pose);
        a.yaw = cam.yaw;
        let d = a.dims;
        let rot = Quat::from_rotation_y(cam.yaw) * Quat::from_rotation_x(-(cam.pitch + LIFT));
        let origin = cam.pos - rot * (Vec3::Y * d.eye);
        a.pos = origin;
        let mut g = super::npc::body_inst(&a, &body, &self.own_sliders(), 0.0);
        g.set_tilt(Quat::from_rotation_x(-(cam.pitch + LIFT)));
        // Cut in the shape's own units: the head about the eye, and the
        // chest and shoulders below it (an arm at rest is there too).
        let eye = d.eye / d.scale.max(1e-3);
        let k = eye / 1.65;
        g.cuts[0] = [0.0, eye, 0.0, 0.27 * k];
        g.cuts[1] = [0.0, eye * 0.76, -0.2 * k, -0.42 * k];
        Some(OwnView { inst: g, body, actor: a, rot, origin })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arms_at_work_come_up_and_arms_at_rest_stay_down() {
        let mut p = [0.0f32; 9];
        p[R_FWD] = 0.55;
        let v = view_pose(&p);
        assert!((v[R_FWD] - 0.9).abs() < 1e-4 && v[L_FWD] == 0.0);
        p[R_FWD] = 1.0;
        assert_eq!(view_pose(&p)[R_FWD], 1.0);
    }
}
