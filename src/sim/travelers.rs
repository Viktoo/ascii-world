//! The other travelers in the world, when people play together: each a
//! body of the traveler's people, in the look they chose, seen from
//! outside. Who moves them is the game's business (a guest moves their
//! own; the host passes them on); here they are bodies in the world.

use super::actor::Actor;
use super::hands::OwnBody;
use super::{ActorId, Sim};
use crate::render::GpuInst;
use crate::world::species::{Dims, LookMap};
use glam::Vec3;

pub struct Traveler {
    pub no: u8,
    pub name: String,
    pub a: Actor,
    pub own: OwnBody,
    /// Where their eyes look, up or down (radians).
    pub pitch: f32,
    /// How hurt they are (0 whole … 1 fallen), when last hurt, and who
    /// felled them (see `night` for the player's own).
    pub wounds: f32,
    pub hurt_at: f64,
    pub fallen: Option<ActorId>,
    pub fallen_at: f64,
}

impl Sim {
    /// One step of a guest's copy of the world: the host runs the world, so
    /// here only time goes on (the host's clock is followed elsewhere), the
    /// sky and weather with it, and the guest's own body stands, falls and
    /// swims under them.
    pub fn step_guest(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.25);
        self.t += dt as f64;
        self.snap.terrain.lay_rivers_soon(self.player.pos.x, self.player.pos.z);
        self.step_weather();
        self.cache.overlay.lamps = crate::render::sky::lighting(self.t, &self.snap.look.palette).night;
        self.step_footing(ActorId::Player, dt);
        let held = self.held_size(ActorId::Player);
        let t = self.t;
        self.player.update_pose(t, dt, held);
        self.place_hands();
    }

    /// Someone else comes into the world, at `pos`.
    pub fn add_traveler(&mut self, no: u8, name: &str, look: &LookMap, pos: Vec3, yaw: f32) {
        let mut a = Actor::new(pos, yaw);
        a.dims = self.player.dims;
        let t = Traveler { no, name: name.to_string(), a, own: OwnBody { look: look.clone() }, pitch: 0.0, wounds: 0.0, hurt_at: 0.0, fallen: None, fallen_at: 0.0 };
        self.travelers.insert(no, t);
    }

    /// They leave: their body goes with them.
    pub fn remove_traveler(&mut self, no: u8) -> Option<Traveler> {
        self.travelers.remove(&no)
    }

    /// Another traveler is hurt (see `wound`): their own wounds, and a fall
    /// at none left. True when they fell.
    pub(super) fn wound_traveler(&mut self, no: u8, hurt: f32, by: ActorId, how: &str) -> bool {
        let t = self.t;
        let Some(tr) = self.travelers.get_mut(&no) else { return false };
        if tr.fallen.is_some() {
            return false;
        }
        tr.wounds = (tr.wounds + hurt).min(1.0);
        tr.hurt_at = t;
        let (name, pos, fell) = (tr.name.clone(), tr.a.pos, tr.wounds >= 1.0);
        if fell {
            tr.fallen = Some(by);
            tr.fallen_at = t;
        }
        let health = 1.0 - tr.wounds;
        self.event("hurt", Some(by), Some(ActorId::Traveler(no).key()), format!("{name} was {how}"), Some(pos), serde_json::json!({ "hurt": (hurt * 100.0).round() / 100.0, "health": (health * 100.0).round() / 100.0 }));
        if fell {
            self.event("traveler_fell", Some(by), Some(ActorId::Traveler(no).key()), format!("{name} fell"), Some(pos), serde_json::Value::Null);
        }
        fell
    }

    /// The look sliders (k.a … k.e of the traveler's body) for one who
    /// chose `own`, at their size.
    pub fn sliders_of(&self, own: &OwnBody, d: &Dims) -> [f32; crate::lang::ir::MAX_SLIDERS] {
        let mut out = [0.5f32; crate::lang::ir::MAX_SLIDERS];
        let Some(b) = self.own_body_type().and_then(|t| t.ct.meta.body.clone()) else { return out };
        for (i, (name, lo, hi)) in b.look.iter().enumerate().take(crate::lang::ir::MAX_SLIDERS) {
            let v = crate::world::species::look_value(&own.look, name).unwrap_or_else(|| match name.as_str() {
                // People draw their own height: the traveler's, at the world's size.
                "height" => d.height / d.scale.max(1e-3),
                _ => b.usual(name).unwrap_or_else(|| super::hands::usual(name, *lo, *hi)),
            });
            out[i] = v.clamp(lo.min(*hi), hi.max(*lo));
        }
        out
    }

    /// The other travelers' bodies, near `cam`, as the world sees them.
    pub(super) fn draw_travelers(&self, cam: Vec3, view: f32, hover: Option<ActorId>) -> Vec<(ActorId, GpuInst)> {
        let Some(body) = self.own_body_type() else { return Vec::new() };
        let mut out = Vec::new();
        for t in self.travelers.values() {
            if (t.a.pos - cam).length() > view.min(200.0) {
                continue;
            }
            let mut g = super::npc::body_inst(&t.a, body, &self.sliders_of(&t.own, &t.a.dims), 0.0);
            let shut = if t.a.asleep { 1.0 } else { super::render::blink(self.t, 1_000_000 + t.no as i64) };
            g.set_face(0.0, shut, (self.t % 3600.0) as f32 + t.no as f32);
            if hover == Some(ActorId::Traveler(t.no)) {
                g.fx[crate::render::FX_HIGHLIGHT] = 0.5;
            }
            out.push((ActorId::Traveler(t.no), g));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::sim::ActorId;

    #[test]
    fn travelers_keep_their_own_codes() {
        for id in [ActorId::Player, ActorId::Traveler(0), ActorId::Traveler(1), ActorId::Traveler(200), ActorId::Npc(7)] {
            assert_eq!(ActorId::from_code(id.code()), id);
            assert_eq!(ActorId::parse(&id.key()), Some(id));
            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(serde_json::from_str::<ActorId>(&json).unwrap(), id, "{json}");
        }
        assert_eq!(ActorId::Traveler(3).key(), "player:3");
    }
}
