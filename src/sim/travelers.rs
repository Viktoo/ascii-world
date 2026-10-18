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
    /// A touch's glow on them (fades).
    pub glow: f32,
    /// The being they are talking with (it stands and faces them).
    pub talking_to: Option<i64>,
}

/// How long one who fell lies before they get up at the world's beginning
/// (seconds of game time), when others play on.
pub const GET_UP_AFTER: f64 = 20.0;

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
        self.ease_npcs(dt);
        self.ease_me(dt);
        self.place_hands();
    }

    /// Someone else comes into the world, at `pos`, with what they held
    /// and wore when they left.
    pub fn add_traveler(&mut self, no: u8, name: &str, look: &LookMap, pos: Vec3, yaw: f32) {
        let mut a = Actor::new(pos, yaw);
        a.dims = self.player.dims;
        let t = Traveler { no, name: name.to_string(), a, own: OwnBody { look: look.clone() }, pitch: 0.0, wounds: 0.0, hurt_at: 0.0, fallen: None, fallen_at: 0.0, glow: 0.0, talking_to: None };
        self.travelers.insert(no, t);
        let who = ActorId::Traveler(no);
        for mut th in self.stowed.remove(&no).unwrap_or_default() {
            if th.holder == Some(who) && self.travelers.get(&no).is_some_and(|t| t.a.held.is_none()) {
                th.pos = pos + Vec3::Y;
                if let Some(t) = self.travelers.get_mut(&no) {
                    t.a.held = Some(th.id);
                }
            } else if th.holder == Some(who) || th.co_holder == Some(who) {
                // A hand already full (or carried with someone gone): set down.
                th.holder = None;
                th.co_holder = None;
                th.pos = pos + Vec3::Y * 0.3;
                th.asleep = false;
                th.dirty = true;
            }
            self.things.unstow(th);
        }
    }

    /// They leave: their body goes with them, and what they hold and wear
    /// (kept as it is, for when they come back).
    pub fn remove_traveler(&mut self, no: u8) -> Option<Traveler> {
        let who = ActorId::Traveler(no);
        // What it is now is what they come back with.
        for t in self.things.map.values_mut() {
            if t.holder == Some(who) || t.worn == Some(who) {
                t.dirty = true;
            }
        }
        super::persist::save(self);
        self.stow_with(no);
        // Anything held together with someone else is let go.
        let shared: Vec<_> = self.things.live().filter(|t| t.co_holder == Some(who)).map(|t| t.id).collect();
        for id in shared {
            if let Some(t) = self.things.get_mut(id) {
                t.co_holder = None;
                t.dirty = true;
            }
        }
        self.travelers.remove(&no)
    }

    /// Put away what traveler `no` holds and wears.
    fn stow_with(&mut self, no: u8) {
        let who = ActorId::Traveler(no);
        let ids: Vec<_> = self.things.live().filter(|t| t.holder == Some(who) || t.worn == Some(who)).map(|t| t.id).collect();
        for id in ids {
            if let Some(t) = self.things.stow(id) {
                self.stowed.entry(no).or_default().push(t);
            }
        }
    }

    /// Everything held or worn by a traveler who isn't here is put away
    /// for them (on loading: nobody else is here yet).
    pub(super) fn stow_absent(&mut self) {
        let away: std::collections::BTreeSet<u8> = self
            .things
            .live()
            .flat_map(|t| [t.holder, t.worn])
            .flatten()
            .filter_map(|a| match a {
                ActorId::Traveler(n) if !self.travelers.contains_key(&n) => Some(n),
                _ => None,
            })
            .collect();
        for n in away {
            self.stow_with(n);
        }
    }

    /// Where every traveler is: this game's own and the others.
    pub fn traveler_spots(&self) -> Vec<Vec3> {
        let mut v = vec![self.player.pos];
        v.extend(self.travelers.values().map(|t| t.a.pos));
        v
    }

    /// How far (across the ground) `p` is from the nearest traveler: how
    /// closely the world is simulated there.
    pub fn dist_to_travelers(&self, p: Vec3) -> f32 {
        let flat = |q: Vec3| ((p.x - q.x).powi(2) + (p.z - q.z).powi(2)).sqrt();
        self.travelers.values().map(|t| flat(t.a.pos)).fold(flat(self.player.pos), f32::min)
    }

    /// The nearest traveler to `p` still on their feet, and where.
    pub fn nearest_traveler(&self, p: Vec3) -> Option<(ActorId, Vec3)> {
        let me = (!self.fallen()).then_some((ActorId::Player, self.player.pos));
        me.into_iter()
            .chain(self.travelers.values().filter(|t| t.fallen.is_none()).map(|t| (ActorId::Traveler(t.no), t.a.pos)))
            .min_by(|a, b| (a.1 - p).length_squared().total_cmp(&(b.1 - p).length_squared()))
    }

    /// Whether a traveler lies fallen.
    pub fn traveler_fallen(&self, who: ActorId) -> bool {
        match who {
            ActorId::Player => self.fallen(),
            ActorId::Traveler(n) => self.travelers.get(&n).is_some_and(|t| t.fallen.is_some()),
            ActorId::Npc(_) => false,
        }
    }

    /// The others' own time going on (once a frame on the host): wounds
    /// mend, and one who fell gets up at the world's beginning a little
    /// later (the world goes on for everyone else: no night is skipped).
    pub(super) fn step_travelers(&mut self, dt: f32) {
        let t = self.t;
        let spawn = self.snap.spawn;
        let ground = self.snap.terrain.height(spawn.x, spawn.z);
        let mut woke = Vec::new();
        for tr in self.travelers.values_mut() {
            tr.glow = (tr.glow - dt / 120.0).max(0.0);
            match tr.fallen {
                Some(_) if t - tr.fallen_at > GET_UP_AFTER => {
                    tr.fallen = None;
                    tr.wounds = 0.0;
                    tr.a.pos = Vec3::new(spawn.x, ground, spawn.z);
                    woke.push((tr.no, tr.name.clone()));
                }
                Some(_) => {}
                None => tr.wounds = (tr.wounds - super::body::MEND_PER_DAY / crate::render::sky::DAY_SECONDS as f32 * dt).max(0.0),
            }
        }
        for (no, name) in woke {
            self.event("traveler_woke", Some(ActorId::Traveler(no)), None, format!("{name} woke where the world began"), Some(Vec3::new(spawn.x, ground, spawn.z)), serde_json::json!({}));
        }
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
        let (name, pos, fell, health) = (tr.name.clone(), tr.a.pos, tr.wounds >= 1.0, 1.0 - tr.wounds);
        if fell {
            tr.fallen = Some(by);
            tr.fallen_at = t;
            tr.a.task = None;
            tr.a.motion = None;
            tr.a.riding = None;
            tr.a.aboard = None;
            // What they held falls with them.
            let (held, fwd) = (tr.a.held, tr.a.forward());
            if let Some(h) = held {
                self.release(h);
                if let Some(th) = self.things.get_mut(h) {
                    th.pos = pos + fwd * 0.6 + Vec3::Y * 0.4;
                    th.asleep = false;
                    th.dirty = true;
                }
            }
        }
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
    pub(super) fn draw_travelers(&self, cam: Vec3, view: f32) -> Vec<(ActorId, GpuInst)> {
        let Some(body) = self.own_body_type() else { return Vec::new() };
        let mut out = Vec::new();
        for t in self.travelers.values() {
            if (t.a.pos - cam).length() > view.min(200.0) {
                continue;
            }
            let mut g = super::npc::body_inst(&t.a, body, &self.sliders_of(&t.own, &t.a.dims), 0.0);
            let shut = if t.a.asleep { 1.0 } else { super::render::blink(self.t, 1_000_000 + t.no as i64) };
            g.set_face(0.0, shut, (self.t % 3600.0) as f32 + t.no as f32);
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
