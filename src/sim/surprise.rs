//! Surprise: how much something breaks what an onlooker thinks can happen,
//! one number 0..1. Every source of news (a thing made, appearing out of
//! nowhere, turning into another, a being brought into the world) says what
//! was seen as a `Sight`; each onlooker feels it by their own size and by how
//! used to wonders they already are. It sets how much they remember and
//! retell it, whether they drop what they are doing, and the feeling their
//! planner is told about. Their traits do the rest (the brave come closer,
//! the timid back away).

use super::props::P_STRANGE;
use super::{ActorId, Request, Sim};
use crate::render::sky::DAY_SECONDS;
use crate::world::TypeEntry;
use crate::world::species::Mind;
use glam::Vec3;

/// How it came about.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arrival {
    /// Made or done in plain view, by hands.
    Seen,
    /// Something already there turned into something else.
    Changed,
    /// Out of nowhere.
    FromNowhere,
}

/// What an onlooker saw.
#[derive(Clone, Copy, Debug)]
pub struct Sight {
    pub how: Arrival,
    /// Its biggest size (m).
    pub extent: f32,
    /// Its `strange` property: how out of place it is in this world.
    pub strange: f32,
}

impl Sight {
    /// Surprise to someone of `height` (m) who isn't used to wonders.
    pub fn raw(&self, height: f32) -> f32 {
        let how = match self.how {
            Arrival::Seen => 0.2,
            Arrival::Changed => 0.6,
            Arrival::FromNowhere => 1.0,
        };
        // A cup is small to a person and a house is not; a giant shrugs at a cart.
        let size = (self.extent / (3.5 * height.max(0.2))).sqrt().clamp(0.25, 1.0);
        (how * (0.25 + 0.75 * size) + 0.6 * self.strange.clamp(0.0, 1.0)).clamp(0.0, 1.0)
    }
}

/// Wonders seen wear off: half in this many game seconds.
const HABIT_HALF_LIFE: f64 = 2.0 * DAY_SECONDS;
/// From here the planner hears about it (when free to think).
pub const ASK_AT: f32 = 0.3;
/// From here they drop what they are doing (once per `SHOCK_GAP`).
pub const DROP_AT: f32 = 0.65;
const SHOCK_GAP: f64 = 30.0;

/// What is left of `habit` after `dt` game seconds.
pub fn habit_now(habit: f32, dt: f64) -> f32 {
    habit * 0.5f32.powf((dt.max(0.0) / HABIT_HALF_LIFE) as f32)
}

/// Surprise felt by someone with this much habit.
pub fn felt(raw: f32, habit: f32) -> f32 {
    raw / (1.0 + 0.5 * habit.max(0.0))
}

/// The feeling, in words (first person), or None when it is nothing much.
pub fn feeling(s: f32, brave: f32) -> Option<&'static str> {
    Some(match s {
        s if s < ASK_AT => return None,
        s if s < 0.55 => "It surprised me.",
        s if s < 0.8 && brave < 0.4 => "It startled me.",
        s if s < 0.8 => "I could hardly believe it.",
        _ if brave < 0.4 => "It frightened me; I have never seen anything like it.",
        _ => "I have never seen anything like it; I could hardly believe my eyes.",
    })
}

/// News for onlookers: what they remember, what the planner is told.
pub struct News<'a> {
    pub at: Vec3,
    pub sight: Sight,
    /// The memory, first person ("I saw Mara make a boat.").
    pub memory: &'a str,
    /// At least this important, however little it surprises.
    pub importance: f32,
    /// Planner event name and what it is told ("Mara just made a boat near you.").
    pub event: &'a str,
    pub tell: &'a str,
    /// What to run from, in words ("the boat").
    pub what: &'a str,
}

impl Sim {
    /// What seeing a thing of this type and scale arrive this way is like.
    pub fn sight_of(&mut self, ty: &TypeEntry, scale: f32, how: Arrival) -> Sight {
        let b = ty.ct.meta.bounds;
        let extent = 2.0 * b[0].max(b[1]).max(b[2]) * scale.max(0.01);
        let strange = self.type_props.get(&self.vocab, ty)[P_STRANGE];
        Sight { how, extent, strange }
    }

    /// Surprise to an ordinary person who isn't used to wonders (for the log).
    pub fn plain_surprise(&self, sight: Sight) -> f32 {
        sight.raw(1.75)
    }

    /// Characters within `range` who can see it take in the news.
    pub fn startle(&mut self, range: f32, news: &News, except: &[ActorId]) {
        let ids: Vec<i64> = self.cast.npcs.iter().filter(|n| !n.dead && !n.a.asleep && (n.a.pos - news.at).length() < range && !except.contains(&ActorId::Npc(n.def.id))).map(|n| n.def.id).collect();
        for cid in ids {
            self.startle_one(cid, news);
        }
    }

    /// One onlooker takes in the news: remembers it as much as it surprised
    /// them, tells their planner when it is worth a thought (dropping what
    /// they do when it is a shock), and the timid without a planner run.
    pub fn startle_one(&mut self, cid: i64, news: &News) {
        let t = self.t;
        let Some(n) = self.cast.get_mut(cid) else { return };
        if n.recently_witnessed(news.memory, t) {
            return;
        }
        let habit = habit_now(n.habit, t - n.habit_at);
        let s = felt(news.sight.raw(n.a.dims.height), habit);
        n.habit = habit + if s > 0.2 { s } else { 0.0 };
        n.habit_at = t;
        let importance = news.importance.max(s).min(1.0);
        n.curiosity_bump(importance);
        let brave = n.traits.brave;
        let shock = s >= DROP_AT && t - n.shock_at > SHOCK_GAP;
        if shock {
            n.shock_at = t;
        }
        let memory = match feeling(s, brave) {
            Some(f) => format!("{} {f}", news.memory),
            None => news.memory.to_string(),
        };
        self.out.push(Request::Witness { cid, text: memory, importance });
        let mut thought = false;
        if s >= ASK_AT && self.mind_of(ActorId::Npc(cid)) == Mind::Sapient {
            let how = feeling(s, brave).unwrap_or("");
            let tell = format!("{}\nHow surprising this is to you: {s:.1} of 1. {how}", news.tell);
            thought = self.ask_planner(cid, news.event, &tell, shock);
        }
        // Beasts, and people with no thought to spare, act on the fright itself.
        if shock && !thought && brave < 0.4 {
            self.run_from(cid, news.what, None, news.at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigger_stranger_and_out_of_nowhere_surprise_more() {
        let cup = Sight { how: Arrival::FromNowhere, extent: 0.3, strange: 0.0 };
        let house = Sight { how: Arrival::FromNowhere, extent: 8.0, strange: 0.0 };
        let cart = Sight { how: Arrival::Seen, extent: 3.0, strange: 0.0 };
        let car = Sight { how: Arrival::Seen, extent: 4.0, strange: 0.9 };
        let h = 1.75;
        assert!(house.raw(h) > 0.95);
        assert!(cup.raw(h) > ASK_AT && cup.raw(h) < DROP_AT);
        assert!(cart.raw(h) < ASK_AT);
        assert!(car.raw(h) > DROP_AT);
        // A giant shrugs at what a person stares at.
        assert!(house.raw(8.0) < house.raw(h));
    }

    #[test]
    fn wonders_wear_off_and_come_back() {
        let mut habit = 0.0;
        let mut felt_seq = Vec::new();
        for _ in 0..4 {
            let s = felt(1.0, habit);
            felt_seq.push(s);
            habit += s;
        }
        assert!(felt_seq.windows(2).all(|w| w[1] < w[0]));
        // The second is still a shock; by the third they are getting used to it.
        assert!(felt_seq[1] >= DROP_AT && felt_seq[2] < DROP_AT);
        let rested = habit_now(habit, 20.0 * DAY_SECONDS);
        assert!(felt(1.0, rested) > 0.9);
    }
}
