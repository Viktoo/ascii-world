//! The weather in the living world (see `world::weather`, docs/weather-plan.md):
//! what falls on things out in the open, how bodies feel about it, the news
//! when it turns, and weather made on purpose. Nothing here knows what rain
//! is: a kind's falling stuff carries its own properties, a species its own
//! feelings by word.

use super::{Note, Sim, Target};
use crate::world::weather::{self, Kind, Now, Spell, HOUR};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Saved with the world (kv `sim.weather`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WeatherState {
    /// Weather made on purpose, while it lasts.
    #[serde(default)]
    pub spell: Option<Spell>,
    /// The weather last told, to tell when it turns.
    #[serde(default)]
    pub told: String,
    #[serde(default)]
    pub falling: bool,
    #[serde(default)]
    pub storm: bool,
}

/// What falls draws a thing toward its properties this fast (per second, at full amount).
const SOAK_RATE: f32 = 0.5;
/// How far from the traveler people hear of the weather turning.
const NEWS_RANGE: f32 = 90.0;

impl Sim {
    /// The weather now (called every step; cheap).
    pub fn step_weather(&mut self) {
        self.weather.spell = self.weather_st.spell.clone();
        self.wx = self.weather.at(self.t);
        if self.weather_st.spell.as_ref().is_some_and(|s| self.t > s.until + HOUR) {
            self.weather_st.spell = None;
        }
    }

    /// Whether the sky is open above a point (not inside anything).
    pub fn open_sky(&self, p: Vec3) -> bool {
        self.room_at(p).is_none()
    }

    /// What falls, as property ids and the values it draws things toward.
    pub fn falls_props(&self) -> Vec<(usize, f32)> {
        let Some(f) = &self.wx.falls else { return Vec::new() };
        f.props.iter().filter_map(|(k, v)| self.vocab.id(k).map(|i| (i, *v))).collect()
    }

    /// Draw a thing out in the open toward what falls on it.
    pub fn soak(props: &mut [f32], falls: &[(usize, f32)], amount: f32, dt: f32) {
        let k = (amount * SOAK_RATE * dt).min(1.0);
        for &(i, target) in falls {
            if let Some(v) = props.get_mut(i) {
                if (target > 0.0 && *v < target) || (target < 0.0 && *v > target) || (target == 0.0) {
                    *v += (target - *v) * k;
                }
            }
        }
    }

    /// The same for something that has stood out in it all along (the
    /// static land: grass, trees): as soaked as the ground is.
    pub fn soaked(props: &mut [f32], falls: &[(usize, f32)], soak: f32) {
        for &(i, target) in falls {
            if let Some(v) = props.get_mut(i) {
                if (target > *v && target > 0.0) || (target < *v && target < 0.0) {
                    *v += (target - *v) * soak.clamp(0.0, 1.0);
                }
            }
        }
    }

    /// How a being feels about the weather now, -1..1.
    pub fn weather_feeling(&self, cid: i64) -> f32 {
        let Some(n) = self.cast.get(cid) else { return 0.0 };
        weather::feeling(&n.species.weather, &self.wx)
    }

    /// One line for a character's planner: the weather, where they are in
    /// it, and how they feel about it.
    pub fn weather_line(&self, cid: i64) -> String {
        let Some(n) = self.cast.get(cid) else { return String::new() };
        let inside = !self.open_sky(n.a.pos);
        let f = self.weather_feeling(cid);
        let feel = match f {
            f if f < -0.6 => "you hate this weather",
            f if f < -0.25 => "you dislike this weather",
            f if f > 0.6 => "you love this weather",
            f if f > 0.25 => "you like this weather",
            _ => "the weather doesn't trouble you",
        };
        let falls = self.wx.falls.as_ref().filter(|f| f.amount > 0.05);
        let where_ = match (inside, falls) {
            (true, _) => "you are under a roof".to_string(),
            (false, Some(fl)) => format!("you are out in the {}", fl.what),
            (false, None) => "you are out of doors".to_string(),
        };
        format!("The weather: {}; {where_}; {feel}.", self.wx.words())
    }

    /// Once a second: the weather turning (to another kind) is news, and
    /// those near who mind it think again.
    pub fn tell_weather(&mut self) {
        let now = self.wx.clone();
        let falling = now.falls.as_ref().is_some_and(|f| f.amount > 0.08);
        let storm = now.storm > 0.2;
        let st = &self.weather_st;
        if st.told == now.name {
            return;
        }
        let line = if st.told.is_empty() { None } else { Some(turn_line(&now, st.falling, falling, st.storm, storm, &st.told)) };
        self.weather_st.told = now.name.clone();
        self.weather_st.falling = falling;
        self.weather_st.storm = storm;
        let Some(line) = line else { return };
        let me = self.player.pos;
        self.event("weather", None, None, line.clone(), Some(me), serde_json::json!({ "weather": now.name, "falling": falling, "storm": storm }));
        self.notes.push(Note::Ambient(line.clone()));
        // People nearby who are awake decide what to do about it.
        let near: Vec<i64> = self.cast.npcs.iter().filter(|n| n.here() && !n.a.asleep && (n.a.pos - me).length() < NEWS_RANGE).map(|n| n.def.id).collect();
        for cid in near {
            let f = self.weather_feeling(cid);
            if f.abs() > 0.25 {
                let ctx = format!("{line} {}", self.weather_line(cid));
                self.ask(cid, "weather", &ctx);
            }
            if let Some(n) = self.cast.get_mut(cid) {
                n.think_at = n.think_at.min(self.t + 0.5);
            }
        }
    }

    /// Weather made on purpose (the traveler's `/`, a spell that works):
    /// it comes now and holds for `hours`.
    pub fn make_weather(&mut self, mut kind: Kind, hours: f32, by: &str) {
        let mut c = weather::Climate { kinds: vec![kind.clone()] };
        c.sanitize();
        if let Some(k) = c.kinds.pop() {
            kind = k;
        }
        let hours = if hours.is_finite() { hours.clamp(0.5, 48.0) } else { 6.0 };
        self.weather_st.spell = Some(Spell { kind: kind.clone(), from: self.t, until: self.t + hours as f64 * HOUR, by: by.to_string() });
        self.step_weather();
        self.event("weather_made", None, None, format!("{by} made the weather {}", kind.name), Some(self.player.pos), serde_json::json!({ "weather": kind.name, "hours": hours }));
    }

    /// Where someone could get out of the weather: their own bed's house,
    /// the nearest way in, or under something big and solid (a tree).
    pub fn shelter_for(&mut self, cid: i64) -> Option<(Target, String)> {
        let pos = self.cast.get(cid)?.a.pos;
        if let Some(bed) = self.bed_for(cid).filter(|b| (*b - pos).length() < 160.0) {
            return Some((Target::Point(bed.to_array()), "home".into()));
        }
        let door = self.rooms.iter().filter_map(|r| r.doors.first().map(|d| d.1)).map(|p| ((p - pos).length(), p)).filter(|(d, _)| *d < 70.0).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, p)) = door {
            return Some((Target::Point(p.to_array()), "indoors".into()));
        }
        let snap = self.snap.clone();
        let cover = self.cache.items_near(&snap, pos, 30.0).into_iter().filter(|i| i.solid && i.inst.radius() > 1.2).map(|i| ((i.inst.pos() - pos).length(), i.inst.pos(), i.inst.info[0])).min_by(|a, b| a.0.total_cmp(&b.0));
        cover.map(|(_, p, ty)| {
            let name = snap.type_of(ty).map(|t| t.name().to_string()).unwrap_or_else(|| "a tree".into());
            let stand = p + (pos - p).normalize_or_zero() * 0.9;
            (Target::Point(stand.to_array()), format!("under the {name}"))
        })
    }
}

/// The weather turning, in a line.
fn turn_line(now: &Now, was_falling: bool, falling: bool, was_storm: bool, storm: bool, before: &str) -> String {
    let what = now.falls.as_ref().map(|f| f.what.clone()).unwrap_or_else(|| "rain".into());
    if storm && !was_storm {
        return "Thunder rolls; a storm is coming on.".into();
    }
    if falling && !was_falling {
        return format!("{} begins to fall.", cap(&what));
    }
    if !falling && was_falling {
        return format!("The {before} eases off.");
    }
    if was_storm && !storm {
        return "The storm passes.".into();
    }
    format!("The weather turns: {}.", now.words())
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
