//! Air: how much there is about a point to breathe and to burn, 0..1. The
//! world has its own out in the open (`Look::air`: Earth's 1, the Moon's 0);
//! a room whose building gives `air` (a sealed habitat, a dome) has that
//! inside, and what someone wears or holds that gives it (a space suit, an
//! air tank) is theirs wherever they go. Fire needs it (the rule variable
//! `air`); bodies that breathe suffocate without it.

use super::props::{P_AIR, P_HEALTH};
use super::{ActorId, Note, Sim};
use glam::Vec3;

/// Under this a body that breathes can't, and fire goes out.
pub const THIN: f32 = 0.2;
/// Health lost each second without air: about a minute to nothing.
const SUFFOCATE_PER_S: f32 = 1.0 / 60.0;

impl Sim {
    /// The air at `p`: the world's, or more inside a room that holds its own.
    pub fn air_at(&self, p: Vec3) -> f32 {
        let open = self.snap.look.air;
        if open >= 1.0 {
            return open;
        }
        self.room_at(p).map(|r| self.rooms[r].air.max(open)).unwrap_or(open)
    }

    /// The air a being has to breathe: about their head, or from what they
    /// wear or hold, or their own body's.
    pub fn air_for(&self, who: ActorId) -> f32 {
        let Some(a) = self.actor(who) else { return self.snap.look.air };
        let mut air = self.air_at(a.pos + Vec3::Y * a.dims.eye);
        if air >= 1.0 {
            return air;
        }
        for t in self.things.live().filter(|t| t.worn == Some(who) || t.holder == Some(who)) {
            air = air.max(t.props.get(P_AIR).copied().unwrap_or(0.0));
        }
        if let ActorId::Npc(c) = who {
            if let Some(n) = self.cast.get(c) {
                air = air.max(n.props.get(P_AIR).copied().unwrap_or(0.0));
            }
        }
        air
    }

    /// Beings that breathe and have no air suffocate: it hurts, and it
    /// wears their health away (to death, where things die). The traveler
    /// is spared for now.
    pub fn breathe(&mut self, dt: f32) {
        if self.snap.look.air >= THIN {
            for n in self.cast.npcs.iter_mut() {
                n.gasping = false;
            }
            return;
        }
        let breathers: Vec<i64> = self.cast.npcs.iter().filter(|n| n.here() && n.species.breathes && n.props.len() > P_HEALTH).map(|n| n.def.id).collect();
        for c in breathers {
            let gasping = self.air_for(ActorId::Npc(c)) < THIN;
            let Some(n) = self.cast.get_mut(c) else { continue };
            let began = gasping && !n.gasping;
            n.gasping = gasping;
            if !gasping {
                continue;
            }
            n.pain = n.pain.max(0.6);
            let (name, pos) = (n.name().to_string(), n.a.pos);
            if began {
                self.note_near(pos, 30.0, Note::Ambient(format!("{} gasps for air.", super::physics::cap(&name))));
            }
            let id = ActorId::Npc(c);
            if self.wound(id, SUFFOCATE_PER_S * dt, id, "suffocated for want of air") {
                self.kill(c, "suffocated for want of air", None);
            }
        }
    }
}
