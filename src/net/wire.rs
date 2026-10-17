//! What host and guests say to each other. Plain data only (postcard can't
//! carry self-describing values such as JSON), so the game's own types are
//! turned into these at the edge.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A traveler's number in a world: 0 is the world's owner (the host), the
/// rest are guests, each kept by their token across visits.
pub type TravelerNo = u8;

/// The first thing a guest says.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Hello {
    pub protocol: u32,
    pub release: String,
    pub commit: String,
    pub name: String,
    pub token: String,
    pub code: String,
}

/// Guest → host.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Up {
    Hello(Hello),
    /// Where the guest's body is and how it stands: they move it
    /// themselves (the host checks it), about 20 times a second.
    Me(Body),
    /// They changed how they look.
    Look(BTreeMap<String, f32>),
    /// Leaving.
    Bye,
}

/// Host → guest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Down {
    /// Not let in, and why (a wrong code, another build).
    Refused(String),
    /// Let in: who they are, and the world follows in chunks.
    Welcome(Welcome),
    /// A piece of the world's file (deflated), in order.
    WorldChunk(Vec<u8>),
    /// The last piece came.
    WorldDone,
    /// Who else is here now (sent on joining and whenever it changes).
    Roster(Vec<Traveler>),
    /// The others' bodies, about 20 times a second (not the guest's own),
    /// and the host's game time.
    Bodies { t: f64, bodies: Vec<(TravelerNo, Body)> },
    /// The host moved the guest (pulled out of the ground, carried, put
    /// back): take this place.
    Place(Body),
    /// A line for the log.
    Say(String),
    /// The host is leaving or closed the game.
    Bye(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Welcome {
    pub you: TravelerNo,
    /// The world's name, as the host's worlds list shows it.
    pub world: String,
    /// The host's name.
    pub host: String,
    /// The world's file, before deflating, and deflated (for progress).
    pub bytes: u64,
    pub packed: u64,
    /// Where the guest stands and faces (where they left, or the start).
    pub body: Body,
    pub look: BTreeMap<String, f32>,
    /// How hurt they are (0 whole … 1 fallen).
    pub wounds: f32,
    /// The host's game time.
    pub t: f64,
}

/// Someone in the world.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Traveler {
    pub no: TravelerNo,
    pub name: String,
    pub look: BTreeMap<String, f32>,
}

/// A body as it is this moment: enough to draw it and to move it.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Body {
    pub pos: [f32; 3],
    pub yaw: f32,
    /// Where the eyes look, up or down.
    pub pitch: f32,
    pub pose: [f32; 9],
    /// Walk phase.
    pub phase: f32,
    pub moved: f32,
    pub crouch: f32,
    pub alt: f32,
    pub flags: u8,
}

pub const RUNNING: u8 = 1;
pub const CROUCHING: u8 = 2;
pub const SWIMMING: u8 = 4;
pub const GROUNDED: u8 = 8;
pub const ASLEEP: u8 = 16;
pub const CONJURING: u8 = 32;

impl Body {
    pub fn of(a: &crate::sim::actor::Actor, pitch: f32) -> Body {
        let mut flags = 0;
        for (on, f) in [(a.running, RUNNING), (a.crouching, CROUCHING), (a.swimming, SWIMMING), (a.grounded, GROUNDED), (a.asleep, ASLEEP), (a.conjuring, CONJURING)] {
            if on {
                flags |= f;
            }
        }
        Body { pos: a.pos.to_array(), yaw: a.yaw, pitch, pose: a.pose, phase: a.phase, moved: a.moved, crouch: a.crouch, alt: a.alt, flags }
    }

    /// Put this onto an actor (not its place in seats or hands: those are
    /// the world's).
    pub fn onto(&self, a: &mut crate::sim::actor::Actor) {
        a.pos = glam::Vec3::from_array(self.pos);
        a.yaw = self.yaw;
        a.pose = self.pose;
        a.phase = self.phase;
        a.moved = self.moved;
        a.crouch = self.crouch;
        a.alt = self.alt;
        a.running = self.flags & RUNNING != 0;
        a.crouching = self.flags & CROUCHING != 0;
        a.swimming = self.flags & SWIMMING != 0;
        a.grounded = self.flags & GROUNDED != 0;
        a.asleep = self.flags & ASLEEP != 0;
        a.conjuring = self.flags & CONJURING != 0;
    }

    /// Part way from `self` to `b` (0..1), for drawing between two.
    pub fn lerp(&self, b: &Body, k: f32) -> Body {
        let mix = |x: f32, y: f32| x + (y - x) * k;
        let mut out = *b;
        for i in 0..3 {
            out.pos[i] = mix(self.pos[i], b.pos[i]);
        }
        let dy = (b.yaw - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        out.yaw = self.yaw + dy * k;
        out.pitch = mix(self.pitch, b.pitch);
        for i in 0..9 {
            out.pose[i] = mix(self.pose[i], b.pose[i]);
        }
        out.crouch = mix(self.crouch, b.crouch);
        out.alt = mix(self.alt, b.alt);
        // The walk phase runs on; a wrap shouldn't spin the legs backwards.
        out.phase = if (b.phase - self.phase).abs() < 3.0 { mix(self.phase, b.phase) } else { b.phase };
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_go_through_whole() {
        let mut look = BTreeMap::new();
        look.insert("skin".to_string(), 0.4);
        let m = Down::Welcome(Welcome { you: 2, world: "a bay".into(), host: "Hal".into(), bytes: 10, packed: 5, body: Body { pos: [1.0, 2.0, 3.0], flags: RUNNING | GROUNDED, ..Default::default() }, look, wounds: 0.0, t: 12.5 });
        let back: Down = crate::net::decode(&crate::net::encode(&m)).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn a_body_turns_the_short_way_between_two() {
        let a = Body { yaw: 3.0, ..Default::default() };
        let b = Body { yaw: -3.0, ..Default::default() };
        let mid = a.lerp(&b, 0.5);
        assert!((mid.yaw.sin() - std::f32::consts::PI.sin()).abs() < 0.05, "{}", mid.yaw);
    }
}
