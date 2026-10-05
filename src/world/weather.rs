//! Weather (docs/weather-plan.md). Each world has a climate: a short list of
//! weather kinds written at genesis (fair, drizzle, ash fall, golden haze,
//! a thunderstorm…), each a few numbers and, if anything falls, what it is
//! and what it does to what it lands on (as properties). The engine runs
//! them as a timeline of spells from the world's seed and the time, so
//! nothing needs saving and every part of the game (sky, light, rules,
//! characters, sound) reads the same moment. Nothing here knows what rain
//! is: rain, snow, ash and moon-rain are the same record.

use crate::noise::{h2, pcg, u2f, vnoise2};
use crate::render::sky::DAY_SECONDS;
use crate::terrain::{rgbv, smoothstep};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One in-game hour, in seconds of world time.
pub const HOUR: f64 = DAY_SECONDS / 24.0;
/// Clouds sit this high above the ground.
pub const CLOUD_HEIGHT: f32 = 420.0;

/// How falling stuff is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallLook {
    /// Fast thin streaks (rain).
    #[default]
    Streak,
    /// Slow drifting flakes (snow, ash, blossom).
    Flake,
    /// Specks that hang and wander (motes, sparks, spores).
    Mote,
}

/// What falls from the sky in a weather kind.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Falls {
    /// In words: "rain", "snow", "ash", "glimmering motes".
    #[serde(default)]
    pub what: String,
    /// How much, 0 (a few drops) .. 1 (a downpour).
    #[serde(default = "half")]
    pub amount: f32,
    #[serde(default = "rain_color")]
    pub color: [u8; 3],
    #[serde(default)]
    pub look: FallLook,
    /// What it does to whatever it lands on, out in the open: each property
    /// is drawn toward this value (rain { wet: 1 }, snow { wet: 0.4, temp: -2 }).
    #[serde(default)]
    pub props: BTreeMap<String, f32>,
}

fn half() -> f32 {
    0.5
}
fn rain_color() -> [u8; 3] {
    [170, 185, 205]
}
fn one() -> f32 {
    1.0
}
fn hours() -> [f32; 2] {
    [4.0, 16.0]
}

/// One kind of weather this world has.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Kind {
    #[serde(default)]
    pub name: String,
    /// How often it comes, relative to the others.
    #[serde(default = "one")]
    pub often: f32,
    /// How long a spell of it lasts, in game hours [shortest, longest].
    #[serde(default = "hours")]
    pub hours: [f32; 2],
    /// Cloud cover, 0 (clear) .. 1 (overcast).
    #[serde(default)]
    pub clouds: f32,
    /// The clouds' own colour (white by default; dark for storms, yellow for a sandstorm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_color: Option<[u8; 3]>,
    /// 0 (still) .. 1 (a gale).
    #[serde(default)]
    pub wind: f32,
    /// Haze, 1 (clear air) .. 4 (thick fog).
    #[serde(default = "one")]
    pub fog: f32,
    /// What colour it lends the light (a golden haze, a green storm), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<[u8; 3]>,
    /// Lightning, 0 (none) .. 1 (constant).
    #[serde(default)]
    pub storm: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub falls: Option<Falls>,
    /// How it feels, in a few words ("warm and close", "biting cold").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub feel: String,
}

impl Kind {
    fn sanitize(&mut self) {
        let u = |v: &mut f32| *v = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
        self.name = self.name.trim().to_lowercase().chars().take(40).collect();
        self.often = if self.often.is_finite() { self.often.clamp(0.0, 10.0) } else { 1.0 };
        for h in self.hours.iter_mut() {
            *h = if h.is_finite() { h.clamp(2.0, 96.0) } else { 6.0 };
        }
        if self.hours[1] < self.hours[0] {
            self.hours.swap(0, 1);
        }
        u(&mut self.clouds);
        u(&mut self.wind);
        u(&mut self.storm);
        self.fog = if self.fog.is_finite() { self.fog.clamp(0.5, 4.0) } else { 1.0 };
        self.feel = self.feel.trim().chars().take(80).collect();
        if let Some(f) = &mut self.falls {
            u(&mut f.amount);
            f.what = f.what.trim().to_lowercase().chars().take(40).collect();
            if f.what.is_empty() {
                f.what = "rain".into();
            }
            f.props.retain(|k, v| v.is_finite() && !k.trim().is_empty());
            while f.props.len() > 4 {
                let k = f.props.keys().next_back().cloned().unwrap_or_default();
                f.props.remove(&k);
            }
        }
    }

    fn plain(name: &str, often: f32, clouds: f32, wind: f32, fog: f32) -> Kind {
        Kind { name: name.into(), often, hours: hours(), clouds, cloud_color: None, wind, fog, tint: None, storm: 0.0, falls: None, feel: String::new() }
    }
}

/// A world's weather: the kinds it has. Empty (worlds made before weather):
/// fair and cloudy days, nothing falling.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Climate {
    #[serde(default)]
    pub kinds: Vec<Kind>,
}

impl Climate {
    pub fn is_default(&self) -> bool {
        self.kinds.is_empty()
    }

    pub fn sanitize(&mut self) {
        self.kinds.retain(|k| !k.name.trim().is_empty());
        self.kinds.truncate(8);
        for k in &mut self.kinds {
            k.sanitize();
        }
        if self.kinds.iter().all(|k| k.often <= 0.0) {
            for k in &mut self.kinds {
                k.often = 1.0;
            }
        }
    }

    /// The kinds in use (the default ones when the world wrote none).
    pub fn kinds(&self) -> Vec<Kind> {
        if !self.kinds.is_empty() {
            return self.kinds.clone();
        }
        vec![Kind::plain("fair", 3.0, 0.25, 0.2, 1.0), Kind::plain("cloudy", 1.5, 0.55, 0.35, 1.0)]
    }
}

/// Plain weather by name, for trying the sky out (`pocket snapshot --weather`).
pub fn sample(name: &str) -> Option<Kind> {
    let rain = |amount: f32| Falls { what: "rain".into(), amount, color: rain_color(), look: FallLook::Streak, props: BTreeMap::from([("wet".to_string(), 1.0)]) };
    let mut k = match name {
        "clear" => Kind::plain("clear", 1.0, 0.0, 0.1, 1.0),
        "fair" => Kind::plain("fair", 1.0, 0.3, 0.2, 1.0),
        "cloudy" => Kind::plain("cloudy", 1.0, 0.6, 0.35, 1.0),
        "overcast" => Kind::plain("overcast", 1.0, 0.95, 0.3, 1.3),
        "fog" => Kind::plain("fog", 1.0, 0.7, 0.05, 3.2),
        "rain" => Kind { falls: Some(rain(0.6)), ..Kind::plain("rain", 1.0, 0.9, 0.35, 1.4) },
        "storm" => Kind { falls: Some(rain(1.0)), storm: 0.8, cloud_color: Some([120, 125, 140]), ..Kind::plain("thunderstorm", 1.0, 1.0, 0.85, 1.6) },
        "snow" => Kind { falls: Some(Falls { what: "snow".into(), amount: 0.7, color: [245, 248, 255], look: FallLook::Flake, props: BTreeMap::from([("wet".to_string(), 0.4)]) }), ..Kind::plain("snow", 1.0, 0.85, 0.2, 1.6) },
        "ash" => Kind { falls: Some(Falls { what: "ash".into(), amount: 0.6, color: [90, 85, 80], look: FallLook::Flake, props: BTreeMap::new() }), tint: Some([255, 200, 160]), cloud_color: Some([110, 95, 90]), ..Kind::plain("ash fall", 1.0, 0.9, 0.2, 2.0) },
        "motes" => Kind { falls: Some(Falls { what: "glimmering motes".into(), amount: 0.5, color: [255, 230, 140], look: FallLook::Mote, props: BTreeMap::new() }), tint: Some([255, 235, 200]), ..Kind::plain("golden haze", 1.0, 0.2, 0.05, 1.8) },
        _ => return None,
    };
    k.sanitize();
    Some(k)
}

/// Weather made on purpose (the traveler's `/`, a rain dance that works):
/// it holds until `until`, then the climate takes over again.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Spell {
    pub kind: Kind,
    pub from: f64,
    pub until: f64,
    /// Who made it, in words.
    #[serde(default)]
    pub by: String,
}

/// What is falling right now.
#[derive(Clone, Debug, PartialEq)]
pub struct FallsNow {
    pub what: String,
    pub amount: f32,
    pub color: Vec3,
    pub look: FallLook,
    pub props: BTreeMap<String, f32>,
}

/// The weather at one moment.
#[derive(Clone, Debug)]
pub struct Now {
    /// The kind it is (or is turning into).
    pub name: String,
    pub feel: String,
    pub clouds: f32,
    pub cloud_color: Vec3,
    pub wind: f32,
    /// The way the wind blows (unit, xz).
    pub wind_dir: Vec2,
    pub fog: f32,
    pub tint: Vec3,
    pub storm: f32,
    pub falls: Option<FallsNow>,
    /// How soaked the open ground is by what fell (0..1).
    pub soak: f32,
    /// How far the clouds have drifted (metres, xz).
    pub drift: Vec2,
    /// When this spell began and ends.
    pub since: f64,
    pub until: f64,
    /// Made on purpose, by whom.
    pub made_by: Option<String>,
}

impl Default for Now {
    fn default() -> Self {
        Now { name: "fair".into(), feel: String::new(), clouds: 0.0, cloud_color: Vec3::ONE, wind: 0.1, wind_dir: Vec2::X, fog: 1.0, tint: Vec3::ONE, storm: 0.0, falls: None, soak: 0.0, drift: Vec2::ZERO, since: 0.0, until: f64::MAX, made_by: None }
    }
}

impl Now {
    /// How much of something a word names there is now, 0..1: a kind's own
    /// name, what falls, or a plain word (rain, snow, storm, wind, fog,
    /// clouds, clear). Species feelings and hours are keyed by these.
    pub fn is(&self, word: &str) -> f32 {
        let w = word.trim().to_lowercase();
        if w.is_empty() {
            return 0.0;
        }
        let falls = self.falls.as_ref().map(|f| f.amount).unwrap_or(0.0);
        if self.name == w || (w.len() > 3 && self.name.contains(&w)) {
            return 1.0;
        }
        if let Some(f) = &self.falls {
            if f.what == w || (w.len() > 2 && (f.what.contains(&w) || w.contains(&f.what))) {
                return (f.amount * 2.0).min(1.0);
            }
        }
        match w.as_str() {
            "falls" | "falling" | "precipitation" | "wet" | "wet weather" => (falls * 2.0).min(1.0),
            "storm" | "storms" | "thunder" | "lightning" | "thunderstorm" => self.storm.max(((self.wind - 0.6) / 0.4).max(0.0) * 0.6),
            "wind" | "windy" | "gale" | "breeze" => smoothstep(0.35, 0.8, self.wind),
            "fog" | "mist" | "misty" | "foggy" | "haze" => smoothstep(1.3, 2.4, self.fog),
            "cloud" | "clouds" | "cloudy" | "overcast" | "grey" | "gray" => smoothstep(0.4, 0.85, self.clouds),
            "clear" | "sun" | "sunny" | "fair" | "fine" | "sunshine" => (1.0 - smoothstep(0.3, 0.7, self.clouds)) * (1.0 - (falls * 3.0).min(1.0)),
            _ => 0.0,
        }
    }

    /// The weather in a few plain words: "drizzle (rain falling, overcast, a breeze)".
    pub fn words(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(f) = &self.falls {
            let how = match f.amount {
                a if a < 0.2 => "a little ",
                a if a > 0.75 => "heavy ",
                _ => "",
            };
            parts.push(format!("{how}{} falling", f.what));
        }
        if self.storm > 0.15 {
            parts.push("thunder and lightning".into());
        }
        parts.push(
            match self.clouds {
                c if c < 0.12 => "clear sky",
                c if c < 0.4 => "a few clouds",
                c if c < 0.75 => "cloudy",
                _ => "overcast",
            }
            .into(),
        );
        match self.wind {
            w if w < 0.15 => parts.push("still air".into()),
            w if w < 0.45 => parts.push("a breeze".into()),
            w if w < 0.75 => parts.push("windy".into()),
            _ => parts.push("a gale".into()),
        }
        if self.fog > 1.6 {
            parts.push(if self.fog > 2.6 { "thick fog" } else { "misty" }.into());
        }
        if !self.feel.is_empty() {
            parts.push(self.feel.clone());
        }
        let made = self.made_by.as_ref().map(|b| format!(", made by {b}")).unwrap_or_default();
        format!("{} ({}{made})", self.name, parts.join(", "))
    }

    /// The sky's light is dimmed this much by cloud and storm (0..1).
    pub fn gloom(&self) -> f32 {
        (self.clouds * self.clouds * 0.75 + self.storm * 0.15 + self.falls.as_ref().map(|f| f.amount * 0.1).unwrap_or(0.0)).min(0.9)
    }
}

/// The weather of one world over time. Cheap to ask every frame: it walks
/// the spells forward from where it last was.
#[derive(Clone, Debug)]
pub struct Weather {
    pub kinds: Vec<Kind>,
    seed: u32,
    /// Weather made on purpose, if any.
    pub spell: Option<Spell>,
    /// Where the walk is: spell number, its start, its kind, the kind before
    /// it, and the clouds' drift at its start.
    at: (u64, f64, usize, usize, Vec2),
}

const SALT: u32 = 0x5EA7_4E12;

impl Weather {
    pub fn new(climate: &Climate, seed: u32) -> Weather {
        let kinds = climate.kinds();
        let first = Self::pick(&kinds, seed, 0, usize::MAX);
        Weather { kinds, seed, spell: None, at: (0, 0.0, first, first, Vec2::ZERO) }
    }

    /// The same weather for a new climate (a world version flip), if it changed.
    pub fn refresh(&mut self, climate: &Climate, seed: u32) {
        let kinds = climate.kinds();
        if kinds != self.kinds || seed != self.seed {
            let spell = self.spell.take();
            *self = Weather::new(climate, seed);
            self.spell = spell;
        }
    }

    fn pick(kinds: &[Kind], seed: u32, n: u64, prev: usize) -> usize {
        let total: f32 = kinds.iter().map(|k| k.often).sum::<f32>().max(1e-3);
        let draw = |salt: u32| {
            let mut r = u2f(h2(n as i32, (n >> 31) as i32, seed ^ SALT ^ salt)) * total;
            for (i, k) in kinds.iter().enumerate() {
                if r < k.often {
                    return i;
                }
                r -= k.often;
            }
            kinds.len().saturating_sub(1)
        };
        let k = draw(0);
        // The same twice running is less likely (a long spell already says it).
        if k == prev && kinds.len() > 1 { draw(0x77) } else { k }
    }

    fn len(&self, n: u64, k: usize) -> f64 {
        let h = self.kinds.get(k).map(|k| k.hours).unwrap_or([4.0, 12.0]);
        let r = u2f(h2(n as i32, 7, self.seed ^ SALT)) as f64;
        (h[0] as f64 + (h[1] - h[0]) as f64 * r) * HOUR
    }

    fn dir(&self, n: u64) -> Vec2 {
        let a = u2f(h2(n as i32, 11, self.seed ^ SALT)) * std::f32::consts::TAU;
        Vec2::new(a.cos(), a.sin())
    }

    fn velocity(&self, n: u64, k: usize) -> Vec2 {
        let w = self.kinds.get(k).map(|k| k.wind).unwrap_or(0.2);
        self.dir(n) * (2.0 + 22.0 * w)
    }

    /// Walk to the spell holding `t`: (number, start, length, kind, kind before, drift at start).
    fn spell_at(&mut self, t: f64) -> (u64, f64, f64, usize, usize, Vec2) {
        let t = t.max(0.0);
        if t < self.at.1 {
            let first = Self::pick(&self.kinds, self.seed, 0, usize::MAX);
            self.at = (0, 0.0, first, first, Vec2::ZERO);
        }
        loop {
            let (n, start, k, prev, drift) = self.at;
            let len = self.len(n, k);
            if t < start + len {
                return (n, start, len, k, prev, drift);
            }
            let drift = drift + self.velocity(n, k) * len as f32;
            let next = Self::pick(&self.kinds, self.seed, n + 1, k);
            self.at = (n + 1, start + len, next, k, drift);
        }
    }

    /// The weather at world time `t`.
    pub fn at(&mut self, t: f64) -> Now {
        if self.kinds.is_empty() {
            return Now::default();
        }
        let (n, start, len, k, prev, drift0) = self.spell_at(t);
        let since = t - start;
        let fade = (len * 0.8).min(2.0 * HOUR).max(1.0);
        let b = smoothstep(0.0, 1.0, (since / fade) as f32);
        let drift = drift0 + self.velocity(n, k) * since as f32;
        let (cur, before) = (self.kinds[k].clone(), self.kinds[prev].clone());
        let dir = (self.dir(n.saturating_sub(1)) * (1.0 - b) + self.dir(n) * b).normalize_or(Vec2::X);
        let mut now = blend(&before, &cur, b, dir);
        now.since = start;
        now.until = start + len;
        // Showers come and go within a spell.
        let gust = vnoise2((t / 37.0) as f32, n as f32 * 3.1, self.seed ^ 0x51);
        if let Some(f) = &mut now.falls {
            f.amount *= 0.6 + 0.55 * gust;
        }
        now.wind = (now.wind * (0.8 + 0.4 * gust)).min(1.0);
        // The ground soaks while it falls and dries after.
        let wet_now = cur.falls.as_ref().map(|f| f.amount).unwrap_or(0.0) * smoothstep(0.0, 1.0, (since / (2.5 * HOUR)) as f32);
        let wet_before = before.falls.as_ref().map(|f| f.amount).unwrap_or(0.0) * (1.0 - smoothstep(0.0, 1.0, (since / (4.0 * HOUR)) as f32));
        now.soak = if n == 0 { wet_now } else { wet_now.max(wet_before) };
        now.drift = drift;
        // Weather made on purpose takes over while it lasts, coming and going gently.
        if let Some(s) = &self.spell {
            if t >= s.from && t < s.until + 0.5 * HOUR {
                let w = smoothstep(0.0, 1.0, ((t - s.from) / (0.25 * HOUR)) as f32) * (1.0 - smoothstep(0.0, 1.0, ((t - s.until) / (0.5 * HOUR)) as f32));
                let mut made = blend(&kind_of(&now), &s.kind, w, dir);
                made.since = s.from;
                made.until = s.until;
                made.drift = drift;
                made.soak = now.soak.max(s.kind.falls.as_ref().map(|f| f.amount).unwrap_or(0.0) * smoothstep(0.0, 1.0, ((t - s.from) / (1.5 * HOUR)) as f32) * w);
                if w > 0.5 {
                    made.made_by = Some(s.by.clone()).filter(|b| !b.is_empty());
                }
                return made;
            }
        }
        now
    }

    /// Lightning: how bright a flash is at `t` (0 none), given the storm.
    pub fn flash(&self, t: f64, storm: f32) -> f32 {
        if storm <= 0.01 {
            return 0.0;
        }
        let mut out = 0.0f32;
        for (tf, _, _) in self.strikes(t - 1.5, t, storm) {
            let a = (t - tf) as f32;
            // A bright crack, a flicker, a fade.
            out += (-a * 7.0).exp() + 0.5 * (-((a - 0.18) * 30.0).powi(2)).exp();
        }
        out.min(1.5)
    }

    /// Lightning strikes in [t0, t1): when, which way (unit xz), how far (m).
    pub fn strikes(&self, t0: f64, t1: f64, storm: f32) -> Vec<(f64, Vec2, f32)> {
        const CELL: f64 = 2.0;
        let mut out = Vec::new();
        if storm <= 0.01 || t1 <= t0 {
            return out;
        }
        let (a, b) = ((t0 / CELL).floor() as i64, (t1 / CELL).floor() as i64);
        for i in a..=b {
            let h = pcg(h2(i as i32, (i >> 31) as i32, self.seed ^ 0x7B0B));
            if u2f(h) >= storm * 0.3 {
                continue;
            }
            let tf = (i as f64 + u2f(pcg(h ^ 1)) as f64) * CELL;
            if tf < t0 || tf >= t1 {
                continue;
            }
            let ang = u2f(pcg(h ^ 2)) * std::f32::consts::TAU;
            let dist = 250.0 + 2800.0 * u2f(pcg(h ^ 3)).powi(2);
            out.push((tf, Vec2::new(ang.cos(), ang.sin()), dist));
        }
        out
    }
}

/// A moment's weather as a kind (to blend made weather in from it).
fn kind_of(n: &Now) -> Kind {
    Kind {
        name: n.name.clone(),
        often: 1.0,
        hours: hours(),
        clouds: n.clouds,
        cloud_color: Some(to_u8(n.cloud_color)),
        wind: n.wind,
        fog: n.fog,
        tint: Some(to_u8(n.tint)),
        storm: n.storm,
        falls: n.falls.as_ref().map(|f| Falls { what: f.what.clone(), amount: f.amount, color: to_u8(f.color), look: f.look, props: f.props.clone() }),
        feel: n.feel.clone(),
    }
}

fn to_u8(v: Vec3) -> [u8; 3] {
    let c = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    [c(v.x), c(v.y), c(v.z)]
}

/// Between two kinds, `b` of the way to the second.
fn blend(a: &Kind, c: &Kind, b: f32, dir: Vec2) -> Now {
    let l = |x: f32, y: f32| x + (y - x) * b;
    let col = |k: &Kind| k.cloud_color.map(rgbv).unwrap_or(Vec3::ONE);
    let tint = |k: &Kind| k.tint.map(rgbv).unwrap_or(Vec3::ONE);
    let fa = a.falls.as_ref().map(|f| f.amount * (1.0 - b)).unwrap_or(0.0);
    let fc = c.falls.as_ref().map(|f| f.amount * b).unwrap_or(0.0);
    let falls = match (&a.falls, &c.falls) {
        (_, Some(f)) if fc >= fa => Some((f, fc + if a.falls.as_ref().is_some_and(|x| x.what == f.what) { fa } else { 0.0 })),
        (Some(f), _) => Some((f, fa + if c.falls.as_ref().is_some_and(|x| x.what == f.what) { fc } else { 0.0 })),
        (None, Some(f)) => Some((f, fc)),
        (None, None) => None,
    }
    .filter(|(_, amt)| *amt > 0.01)
    .map(|(f, amount)| FallsNow { what: f.what.clone(), amount: amount.min(1.0), color: rgbv(f.color), look: f.look, props: f.props.clone() });
    let named = if b >= 0.5 { c } else { a };
    Now {
        name: named.name.clone(),
        feel: named.feel.clone(),
        clouds: l(a.clouds, c.clouds),
        cloud_color: col(a).lerp(col(c), b),
        wind: l(a.wind, c.wind),
        wind_dir: dir,
        fog: l(a.fog, c.fog),
        tint: tint(a).lerp(tint(c), b),
        storm: l(a.storm, c.storm),
        falls,
        ..Now::default()
    }
}

/// How a body feels about the weather now, -1 (hates it) .. 1 (loves it),
/// from its species' own feelings by word; with none, living things mind
/// what falls on them, storms and gales, and like a fine day.
pub fn feeling(likes: &BTreeMap<String, f32>, now: &Now) -> f32 {
    let falls = now.falls.as_ref().map(|f| f.amount).unwrap_or(0.0);
    let plain = -0.9 * falls.sqrt() - 0.7 * now.storm - 0.6 * smoothstep(0.6, 1.0, now.wind) + 0.15 * now.is("clear");
    // Its own feelings where they name the weather; the plain ones for the rest.
    let (mut sum, mut named) = (0.0f32, 0.0f32);
    for (w, v) in likes {
        let k = now.is(w);
        sum += v.clamp(-1.0, 1.0) * k;
        named = named.max(k);
    }
    (sum + plain * (1.0 - named.min(1.0))).clamp(-1.0, 1.0)
}

/// Whether a being that only comes out in some weather (`comes_with`:
/// "rain", "fog", a kind's name) is about now. Empty: any weather.
pub fn suits(comes_with: &[String], now: &Now) -> bool {
    comes_with.is_empty() || comes_with.iter().any(|w| now.is(w) > 0.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wet() -> Climate {
        let mut c: Climate = serde_json::from_value(serde_json::json!({ "kinds": [
            { "name": "fair", "often": 2, "hours": [4, 10], "clouds": 0.2, "wind": 0.2 },
            { "name": "drizzle", "often": 1, "hours": [2, 6], "clouds": 0.9, "wind": 0.3, "falls": { "what": "rain", "amount": 0.5, "props": { "wet": 1 } } },
            { "name": "thunderstorm", "often": 0.5, "hours": [1, 3], "clouds": 1.0, "wind": 0.8, "storm": 0.8, "falls": { "what": "rain", "amount": 1.0 } }
        ] }))
        .unwrap();
        c.sanitize();
        c
    }

    #[test]
    fn same_seed_same_weather_whatever_the_order() {
        let c = wet();
        let mut a = Weather::new(&c, 7);
        let mut b = Weather::new(&c, 7);
        let late = a.at(50_000.0);
        let _ = b.at(10.0);
        let _ = b.at(30_000.0);
        let again = b.at(50_000.0);
        assert_eq!(late.name, again.name);
        assert_eq!(late.drift, again.drift);
        // Going back in time still works.
        assert_eq!(a.at(10.0).name, b.at(10.0).name);
    }

    #[test]
    fn every_kind_comes_and_spells_change_gently() {
        let c = wet();
        let mut w = Weather::new(&c, 3);
        let mut seen = std::collections::BTreeSet::new();
        let mut last: Option<Now> = None;
        let mut t = 0.0;
        while t < 40.0 * DAY_SECONDS {
            let n = w.at(t);
            seen.insert(n.name.clone());
            if let Some(l) = &last {
                assert!((n.clouds - l.clouds).abs() < 0.2, "clouds jump at {t}: {} → {}", l.clouds, n.clouds);
                assert!((n.drift - l.drift).length() < 2000.0, "the clouds don't jump");
            }
            last = Some(n);
            t += 10.0;
        }
        assert_eq!(seen.len(), 3, "{seen:?}");
    }

    #[test]
    fn words_and_feelings() {
        let c = wet();
        let mut w = Weather::new(&c, 3);
        let mut t = 0.0;
        let rain = loop {
            let n = w.at(t);
            if n.falls.as_ref().is_some_and(|f| f.amount > 0.3) && n.name == "drizzle" {
                break n;
            }
            t += 30.0;
        };
        assert!(rain.words().contains("rain falling"), "{}", rain.words());
        assert!(rain.is("rain") > 0.5 && rain.is("drizzle") == 1.0 && rain.is("clear") < 0.1);
        assert!(feeling(&BTreeMap::new(), &rain) < -0.4, "most bodies mind the rain");
        let frog = BTreeMap::from([("rain".to_string(), 0.9)]);
        assert!(feeling(&frog, &rain) > 0.4, "a frog loves it");
        assert!(suits(&["rain".into()], &rain) && !suits(&["snow".into()], &rain));
    }

    #[test]
    fn lightning_only_in_storms() {
        let w = Weather::new(&wet(), 1);
        assert!(w.strikes(0.0, 600.0, 0.0).is_empty());
        let s = w.strikes(0.0, 600.0, 0.8);
        assert!(s.len() > 20 && s.len() < 150, "{}", s.len());
        let (tf, ..) = s[0];
        assert!(w.flash(tf + 0.01, 0.8) > 0.8 && w.flash(tf - 0.01, 0.8) < w.flash(tf + 0.01, 0.8));
    }

    #[test]
    fn made_weather_takes_over_then_leaves() {
        let mut w = Weather::new(&Climate::default(), 1);
        let snow = Kind { name: "snow".into(), clouds: 0.8, falls: Some(Falls { what: "snow".into(), amount: 0.7, color: [255; 3], look: FallLook::Flake, props: BTreeMap::new() }), ..Kind::plain("snow", 1.0, 0.8, 0.2, 1.0) };
        w.spell = Some(Spell { kind: snow, from: 100.0, until: 100.0 + 3.0 * HOUR, by: "the traveler".into() });
        assert!(w.at(90.0).falls.is_none());
        let n = w.at(100.0 + HOUR);
        assert_eq!(n.name, "snow");
        assert!(n.falls.is_some() && n.made_by.as_deref() == Some("the traveler"));
        assert!(w.at(100.0 + 5.0 * HOUR).falls.is_none(), "and goes again");
    }
}
