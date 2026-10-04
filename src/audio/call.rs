//! What a sound is, as data: a call is 1–8 parts played in order, each one
//! of four kinds (a throat, a whistle, a noise, a knock) with a pitch, a
//! length and a few 0–1 sliders. The LLM writes calls for its species; for
//! everything else (old worlds, things, signs) a call is guessed from the
//! words of the sound. Material is how stuff sounds when it is struck,
//! brushed or stepped on, and comes from a thing's properties.
//!
//! Everything here is kept in good-sounding ranges whatever was written:
//! the worst a bad call can do is sound like the wrong animal.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A throat: buzzing folds through a vocal tract (calls, growls, groans,
    /// breath when `breath` is high).
    #[default]
    Voice,
    /// A pure tone (birdsong, hoots, insects, chimes).
    Whistle,
    /// Many tiny bursts of noise (hiss, rustle, splash, crackle).
    Noise,
    /// A strike that rings (clicks, taps, knocks, clatter, snaps).
    Knock,
}

impl Kind {
    fn parse(s: &str) -> Kind {
        match s.trim().to_lowercase().as_str() {
            "whistle" | "tone" | "chirp" | "song" | "pure" => Kind::Whistle,
            "noise" | "hiss" | "rustle" | "breath" | "texture" => Kind::Noise,
            "knock" | "hit" | "click" | "tap" | "strike" => Kind::Knock,
            _ => Kind::Voice,
        }
    }
}

/// One part of a call.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Part {
    pub kind: Kind,
    /// Pitch in Hz at the start, middle and end (0: from the body's size).
    pub hz: [f32; 3],
    /// Seconds per repeat.
    pub len: f32,
    /// Repeats and the silence between them.
    pub times: u8,
    pub gap: f32,
    /// Mouth shape: up to three of a e i o u m n, glided through.
    pub vowel: String,
    pub rough: f32,
    pub breath: f32,
    pub nasal: f32,
    pub wobble: f32,
    pub swell: f32,
    pub loud: f32,
    /// Material, for noises and knocks.
    pub hard: f32,
    pub dry: f32,
    pub ring: f32,
}

impl Default for Part {
    fn default() -> Part {
        Part { kind: Kind::Voice, hz: [0.0; 3], len: 0.4, times: 1, gap: 0.12, vowel: String::new(), rough: 0.1, breath: 0.15, nasal: 0.0, wobble: 0.1, swell: 0.2, loud: 0.5, hard: 0.5, dry: 0.5, ring: 0.2 }
    }
}

impl Part {
    /// Seconds from its start to the end of its last repeat.
    pub fn span(&self) -> f32 {
        self.len * self.times as f32 + self.gap * (self.times as f32 - 1.0)
    }

    /// Keep every number where it sounds right.
    pub fn sanitize(&mut self) {
        let u = |v: &mut f32, d: f32| *v = if v.is_finite() { v.clamp(0.0, 1.0) } else { d };
        let d = Part::default();
        for (v, dv) in [(&mut self.rough, d.rough), (&mut self.breath, d.breath), (&mut self.nasal, d.nasal), (&mut self.wobble, d.wobble), (&mut self.swell, d.swell), (&mut self.loud, d.loud), (&mut self.hard, d.hard), (&mut self.dry, d.dry), (&mut self.ring, d.ring)] {
            u(v, dv);
        }
        for h in self.hz.iter_mut() {
            *h = if h.is_finite() && *h > 0.0 { h.clamp(8.0, 12_000.0) } else { 0.0 };
        }
        // A glide given only partly: hold the last pitch given.
        if self.hz[0] > 0.0 {
            if self.hz[1] <= 0.0 {
                self.hz[1] = self.hz[0];
            }
            if self.hz[2] <= 0.0 {
                self.hz[2] = self.hz[1];
            }
        } else {
            self.hz = [0.0; 3];
        }
        self.len = if self.len.is_finite() { self.len.clamp(0.004, 4.0) } else { 0.4 };
        self.times = self.times.clamp(1, 40);
        self.gap = if self.gap.is_finite() { self.gap.clamp(0.0, 3.0) } else { 0.12 };
        self.vowel = self.vowel.to_lowercase().chars().filter(|c| "aeioumn".contains(*c)).take(3).collect();
        // Never a call that drones on: long repeats are cut short.
        while self.times > 1 && self.span() > 6.0 {
            self.times -= 1;
        }
    }

    fn from_value(v: &Value) -> Option<Part> {
        let o = v.as_object()?;
        let mut p = Part::default();
        let num = |k: &str| o.get(k).and_then(Value::as_f64).map(|x| x as f32);
        if let Some(k) = o.get("kind").and_then(Value::as_str) {
            p.kind = Kind::parse(k);
        }
        match o.get("hz") {
            Some(Value::Number(n)) => p.hz = [n.as_f64().unwrap_or(0.0) as f32; 3],
            Some(Value::Array(a)) => {
                let v: Vec<f32> = a.iter().filter_map(Value::as_f64).map(|x| x as f32).collect();
                match v.len() {
                    0 => {}
                    1 => p.hz = [v[0]; 3],
                    2 => p.hz = [v[0], (v[0] + v[1]) * 0.5, v[1]],
                    _ => p.hz = [v[0], v[1], v[2]],
                }
            }
            _ => {}
        }
        if let Some(x) = num("len") {
            p.len = x;
        }
        if let Some(x) = o.get("times").and_then(Value::as_f64) {
            p.times = x.round().clamp(1.0, 40.0) as u8;
        }
        if let Some(x) = num("gap") {
            p.gap = x;
        }
        if let Some(s) = o.get("vowel").and_then(Value::as_str) {
            p.vowel = s.to_string();
        }
        for (k, f) in [("rough", &mut p.rough), ("breath", &mut p.breath), ("nasal", &mut p.nasal), ("wobble", &mut p.wobble), ("swell", &mut p.swell), ("loud", &mut p.loud), ("hard", &mut p.hard), ("dry", &mut p.dry), ("ring", &mut p.ring)] {
            if let Some(x) = o.get(k).and_then(Value::as_f64) {
                *f = x as f32;
            }
        }
        p.sanitize();
        Some(p)
    }
}

/// A whole call: parts in order.
#[derive(Clone, Debug, PartialEq, Default, Serialize)]
#[serde(transparent)]
pub struct Call(pub Vec<Part>);

impl Call {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// A call from what was written: one part or a list of them. Bad parts
    /// are left out; nothing here ever fails.
    pub fn from_value(v: &Value) -> Call {
        let parts: Vec<Part> = match v {
            Value::Array(a) => a.iter().filter_map(Part::from_value).take(8).collect(),
            Value::Object(_) => Part::from_value(v).into_iter().collect(),
            _ => Vec::new(),
        };
        Call(parts)
    }

    pub fn span(&self) -> f32 {
        self.0.iter().map(|p| p.span() + if p.kind == Kind::Knock { 0.0 } else { 0.06 }).sum()
    }
}

impl<'de> Deserialize<'de> for Call {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Call::from_value(&Value::deserialize(d)?))
    }
}

/// For `#[serde(deserialize_with)]`: a list of calls that never fails.
pub fn lenient_calls<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Call>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(match v {
        Value::Array(a) => a.iter().map(Call::from_value).collect(),
        _ => Vec::new(),
    })
}

// ------------------------------------------------------------ material

/// How stuff sounds when struck, brushed or stepped on (all 0–1).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    /// Soft (fur, moss, green blades) … crisp (stone, metal, dead leaves).
    pub hard: f32,
    /// Wet … brittle.
    pub dry: f32,
    /// How long it rings when struck (metal, glass, a bell).
    pub ring: f32,
    /// Made of leaves and stems (rustles as well as knocks).
    pub leafy: f32,
}

impl Default for Material {
    fn default() -> Material {
        Material { hard: 0.5, dry: 0.5, ring: 0.1, leafy: 0.0 }
    }
}

impl Material {
    pub const SOIL: Material = Material { hard: 0.3, dry: 0.45, ring: 0.0, leafy: 0.0 };
    pub const WATER: Material = Material { hard: 0.15, dry: 0.0, ring: 0.0, leafy: 0.0 };
    /// A living body (what walks into things).
    pub const FLESH: Material = Material { hard: 0.12, dry: 0.3, ring: 0.0, leafy: 0.0 };
    /// Fire: brittle, crackling stuff.
    pub const FIRE: Material = Material { hard: 0.85, dry: 1.0, ring: 0.0, leafy: 0.0 };

    /// From a thing's properties (see `sim::props`), its tags and any
    /// `meta.sound` the type gives.
    pub fn of(props: &[f32], tags: &[String], over: Option<&Material>) -> Material {
        use crate::sim::props::*;
        if let Some(m) = over {
            return *m;
        }
        let g = |i: usize| props.get(i).copied().unwrap_or(0.0);
        let tag = |t: &str| tags.iter().any(|x| x == t);
        let alive = g(P_ALIVE).clamp(0.0, 1.0);
        let burns = g(P_BURNS).clamp(0.0, 1.0);
        let wet = g(P_WET).clamp(0.0, 1.0);
        let fragile = g(P_FRAGILE).clamp(0.0, 1.0);
        let charred = g(P_CHAR).clamp(0.0, 1.0);
        let plant = alive * burns;
        let leafy = if tag("grass") || tag("bush") || tag("flower") || tag("reed") || tag("tree") || tag("pine") { 1.0 } else { plant };
        // Organic and burnable: as soft as it is green; stone and metal: hard.
        let organic = burns.max(alive);
        let hard = (0.85 - 0.55 * organic + 0.4 * (1.0 - alive) * burns * 0.5 + 0.1 * fragile + 0.3 * charred).clamp(0.05, 1.0);
        let hard = if tag("grass") { hard.min(0.2) } else { hard };
        let dry = ((1.0 - wet) * (1.0 - 0.75 * alive) + 0.4 * charred).clamp(0.0, 1.0);
        // Only brittle, hollow things ring by nature (glass, pottery); a bell,
        // a pan or a sheet of metal says so in its `meta.sound`.
        let ring = (0.35 * fragile).clamp(0.0, 1.0);
        Material { hard, dry, ring, leafy: leafy.clamp(0.0, 1.0) }
    }

    /// Ground underfoot from a biome's name.
    pub fn of_ground(biome: &str) -> Material {
        let b = biome.to_lowercase();
        let has = |ws: &[&str]| ws.iter().any(|w| b.contains(w));
        if has(&["snow", "ice", "frost", "glacier", "tundra"]) {
            Material { hard: 0.35, dry: 0.9, ring: 0.0, leafy: 0.0 }
        } else if has(&["sand", "desert", "dune", "beach", "ash"]) {
            Material { hard: 0.25, dry: 1.0, ring: 0.0, leafy: 0.0 }
        } else if has(&["rock", "stone", "cliff", "crag", "mountain", "highland", "peak", "scree", "city", "street", "ruin"]) {
            Material { hard: 0.75, dry: 0.7, ring: 0.05, leafy: 0.0 }
        } else if has(&["marsh", "swamp", "bog", "fen", "mud", "lake", "shore", "wet"]) {
            Material { hard: 0.15, dry: 0.1, ring: 0.0, leafy: 0.0 }
        } else if has(&["forest", "wood", "grove", "jungle", "heath", "autumn"]) {
            Material { hard: 0.4, dry: 0.65, ring: 0.0, leafy: 0.5 }
        } else {
            Material::SOIL
        }
    }

    /// A type's own `meta.sound` (hard, dry, ring), when it gives one.
    pub fn from_meta(v: &[(String, f32)]) -> Option<Material> {
        if v.is_empty() {
            return None;
        }
        let mut m = Material::default();
        for (k, x) in v {
            let x = if x.is_finite() { x.clamp(0.0, 1.0) } else { continue };
            match k.as_str() {
                "hard" => m.hard = x,
                "dry" => m.dry = x,
                "ring" => m.ring = x,
                "leafy" => m.leafy = x,
                _ => {}
            }
        }
        Some(m)
    }
}

// ------------------------------------------------------------ register

/// The pitch a throat of this mass makes when nothing says (Hz).
pub fn voice_hz(mass: f32) -> f32 {
    (120.0 * (70.0 / mass.max(0.05)).powf(0.4)).clamp(12.0, 1800.0)
}

/// The pitch a whistle of this mass makes when nothing says (Hz).
pub fn whistle_hz(mass: f32) -> f32 {
    (2600.0 * (0.1 / mass.max(0.01)).powf(0.18)).clamp(250.0, 8000.0)
}

/// How much a vocal tract of this mass scales a person's formants.
pub fn formant_scale(mass: f32) -> f32 {
    (70.0 / mass.max(0.05)).powf(1.0 / 3.0).clamp(0.18, 4.5)
}

// ------------------------------------------------------------ guessing

/// A call from the words of a sound ("a low growl", "two sharp chirps").
/// `None` when nothing in the words is a sound.
pub fn guess(text: &str, mass: f32, flies: bool) -> Option<Call> {
    let t = format!(" {} ", text.to_lowercase().replace(|c: char| !c.is_alphanumeric(), " "));
    let has = |ws: &[&str]| ws.iter().any(|w| t.contains(w));
    let v = voice_hz(mass);
    let w = whistle_hz(mass);
    let mut p = Part::default();
    // Words that bend whatever it is.
    let low = has(&[" low", "deep", "rumbl", "bass"]);
    let high = has(&[" high", "shrill", "thin ", "squeak", "tiny"]);
    let soft = has(&["soft", "quiet", "faint", "gentle", "whisper", "murmur"]);
    let loud = has(&["loud", "roar", "scream", "shriek", "blast", "bellow", "howl", "trumpet", "boom"]);
    let many = has(&["chatter", "trill", "rattl", "clatter", "giggl", "laugh", "cackl", "purr", "chitter"]);
    if has(&["bark", "woof", "yap", "arf", "ruff"]) {
        p = Part { kind: Kind::Voice, hz: [v * 2.4, v * 2.8, v * 1.9], len: 0.16, times: if has(&["two", "twice", "yap"]) { 2 } else { 1 }, gap: 0.14, vowel: "ao".into(), rough: 0.45, breath: 0.2, swell: 0.0, loud: 0.8, ..p };
    } else if has(&["growl", "snarl", "grumbl", "grunt"]) {
        p = Part { kind: Kind::Voice, hz: [v * 0.9, v, v * 0.85], len: if has(&["grunt"]) { 0.25 } else { 1.1 }, vowel: "ou".into(), rough: 0.85, breath: 0.25, swell: 0.3, loud: 0.5, ..p };
    } else if has(&["purr"]) {
        p = Part { kind: Kind::Voice, hz: [26.0, 27.0, 25.0], len: 0.7, times: 3, gap: 0.12, vowel: "m".into(), rough: 0.3, breath: 0.45, swell: 0.4, loud: 0.15, ..p };
    } else if has(&["meow", "mew", "miao", "miaow", "mrr"]) {
        p = Part { kind: Kind::Voice, hz: [v * 1.6, v * 2.1, v * 1.3], len: 0.6, vowel: "iau".into(), rough: 0.08, breath: 0.12, nasal: 0.35, swell: 0.15, loud: 0.5, ..p };
    } else if has(&["whine", "whimper", "keen"]) {
        p = Part { kind: Kind::Voice, hz: [v * 3.5, v * 3.9, v * 3.0], len: 0.8, vowel: "i".into(), nasal: 0.7, breath: 0.25, wobble: 0.3, swell: 0.3, loud: 0.3, ..p };
    } else if has(&["howl", "wail", "ululat"]) {
        p = Part { kind: Kind::Voice, hz: [v * 2.6, v * 3.6, v * 3.0], len: 2.2, vowel: "ou".into(), rough: 0.05, breath: 0.15, wobble: 0.35, swell: 0.6, loud: 0.95, ..p };
    } else if has(&["whinn", "neigh", "nicker"]) {
        p = Part { kind: Kind::Voice, hz: [v * 5.0, v * 5.5, v * 3.5], len: 1.1, vowel: "ie".into(), rough: 0.3, nasal: 0.5, wobble: 0.9, swell: 0.1, loud: 0.7, ..p };
    } else if has(&["moo", " low ", "bleat", "baa", "maa"]) {
        let bleat = has(&["bleat", "baa", "maa"]);
        p = Part { kind: Kind::Voice, hz: if bleat { [v * 3.0, v * 3.1, v * 2.8] } else { [v * 1.1, v * 1.2, v * 0.9] }, len: if bleat { 0.7 } else { 1.4 }, vowel: if bleat { "ea" } else { "mou" }.into(), rough: 0.2, wobble: if bleat { 0.9 } else { 0.1 }, nasal: 0.3, swell: 0.3, loud: 0.7, ..p };
    } else if has(&["trumpet", "blare", "honk"]) {
        p = Part { kind: Kind::Voice, hz: [v * 9.0, v * 14.0, v * 12.0], len: 1.4, vowel: "ae".into(), rough: 0.6, breath: 0.25, nasal: 0.9, wobble: 0.2, swell: 0.05, loud: 1.0, ..p };
    } else if has(&["roar", "bellow"]) {
        p = Part { kind: Kind::Voice, hz: [v * 1.3, v * 1.6, v * 1.0], len: 1.3, vowel: "ao".into(), rough: 0.9, breath: 0.3, swell: 0.2, loud: 1.0, ..p };
    } else if has(&["groan", "moan"]) {
        p = Part { kind: Kind::Voice, hz: [v * 1.1, v * 1.2, v * 0.8], len: 1.8, vowel: "ou".into(), rough: 0.55, breath: 0.35, swell: 0.6, loud: 0.5, ..p };
    } else if has(&["scream", "shriek", "screech", "squeal", "squawk"]) {
        p = Part { kind: Kind::Voice, hz: [v * 6.0, v * 7.0, v * 5.0], len: 0.7, vowel: "ie".into(), rough: 0.7, breath: 0.2, nasal: 0.4, swell: 0.05, loud: 1.0, ..p };
    } else if has(&["caw", "croak", "crow"]) {
        p = Part { kind: Kind::Voice, hz: [v * 4.0, v * 4.2, v * 3.4], len: 0.35, times: if has(&["croak"]) { 1 } else { 2 }, gap: 0.2, vowel: "a".into(), rough: 0.8, nasal: 0.5, swell: 0.0, loud: 0.8, ..p };
    } else if has(&["laugh", "giggl", "cackl", "chuckl"]) {
        p = Part { kind: Kind::Voice, hz: [v * 1.6, v * 1.7, v * 1.4], len: 0.12, times: 5, gap: 0.07, vowel: "ae".into(), rough: 0.2, breath: 0.35, swell: 0.0, loud: 0.6, ..p };
    } else if has(&["hoot", "coo", "owl"]) {
        p = Part { kind: Kind::Whistle, hz: [w * 0.18, w * 0.19, w * 0.16], len: 0.45, times: if has(&["coo"]) { 3 } else { 2 }, gap: 0.25, breath: 0.35, wobble: 0.05, swell: 0.3, loud: 0.6, ..p };
    } else if has(&["chirp", "tweet", "peep", "cheep", "twitter", "chirrup"]) {
        p = Part { kind: Kind::Whistle, hz: [w, w * 1.3, w * 0.9], len: 0.07, times: if has(&["twitter"]) { 6 } else { 3 }, gap: 0.06, breath: 0.05, swell: 0.1, loud: 0.5, ..p };
    } else if has(&["trill", "warbl", "sing", "song", "whistl"]) {
        p = Part { kind: Kind::Whistle, hz: [w * 1.2, w, w * 1.1], len: 0.05, times: 14, gap: 0.03, rough: if has(&["warbl"]) { 0.3 } else { 0.0 }, wobble: 0.3, swell: 0.1, loud: 0.6, ..p };
    } else if has(&["buzz", "hum", "drone", "whine of"]) {
        p = Part { kind: Kind::Voice, hz: [v * 2.0; 3], len: 1.2, vowel: "mn".into(), rough: 0.4, breath: 0.2, wobble: 0.2, swell: 0.4, loud: 0.3, ..p };
    } else if has(&["hiss"]) {
        p = Part { kind: Kind::Noise, len: 0.8, hard: 0.95, dry: 0.9, swell: 0.1, loud: 0.5, ..p };
    } else if has(&["breath", "pant", "huff", "sniff", "snort", "snuffl", "wheez", "sigh", "exhal", "inhal", "rasp"]) {
        let short = has(&["sniff", "snort", "huff"]);
        p = Part { kind: Kind::Voice, hz: [v * 0.9; 3], len: if short { 0.18 } else { 1.4 }, times: if has(&["sniff", "pant"]) { 3 } else { 1 }, gap: 0.12, vowel: if has(&["snort", "sniff"]) { "n" } else { "ao" }.into(), rough: if has(&["rasp", "wheez", "wet"]) { 0.6 } else { 0.2 }, breath: 1.0, swell: if short { 0.05 } else { 0.4 }, loud: 0.35, ..p };
    } else if has(&["click", "tap", "tick", "clack", "knock", "rap", "clatter", "rattl", "snap", "crack", "creak"]) {
        let wood = has(&["wood", "twig", "branch", "snap", "crack", "creak", "knock"]);
        p = Part { kind: Kind::Knock, hz: [0.0; 3], len: 0.08, times: if many { 6 } else if has(&["click"]) { 2 } else { 1 }, gap: if many { 0.05 } else { 0.18 }, hard: if has(&["wet"]) { 0.35 } else { 0.8 }, dry: if has(&["wet"]) { 0.05 } else if wood { 0.95 } else { 0.6 }, ring: if has(&["metal", "bell", "chime", "glass"]) { 0.85 } else { 0.1 }, loud: 0.5, ..p };
    } else if has(&["ring", "chime", "bell", "ding", "clang", "gong"]) {
        p = Part { kind: Kind::Knock, hz: [if has(&["gong", "clang"]) { 220.0 } else { 880.0 }; 3], len: 1.5, hard: 0.9, dry: 0.5, ring: 1.0, loud: 0.7, ..p };
    } else if has(&["rustl", "crunch", "crackl", "scrap", "drag", "shuffl", "step", "footfall", "slither", "splash", "drip", "squelch"]) {
        let wet = has(&["splash", "drip", "squelch", "wet"]);
        let steps = has(&["step", "footfall"]);
        p = Part { kind: Kind::Noise, len: if steps { 0.15 } else { 0.9 }, times: if steps { 3 } else { 1 }, gap: 0.55, hard: if has(&["crunch", "crackl", "scrap"]) { 0.7 } else { 0.25 }, dry: if wet { 0.0 } else { 0.7 }, swell: 0.2, loud: 0.35, ..p };
    } else if has(&["thud", "thump", "stomp", "boom", "pound", "drum"]) {
        p = Part { kind: Kind::Knock, hz: [60.0; 3], len: 0.3, times: if has(&["drum", "pound"]) { 4 } else { 1 }, gap: 0.3, hard: 0.2, dry: 0.6, ring: 0.05, loud: 0.8, ..p };
    } else if has(&["call", "cry", "sound", "voice", "noise", "word", "name", "speak", "say", "whisper", "mutter", "singing", "yell", "shout"]) {
        p = Part { kind: Kind::Voice, hz: [v * 1.6, v * 1.9, v * 1.3], len: 0.6, vowel: "ao".into(), breath: if has(&["whisper"]) { 1.0 } else { 0.25 }, rough: 0.2, swell: 0.2, loud: 0.5, ..p };
        if flies {
            p.kind = Kind::Whistle;
            p.hz = [w, w * 1.2, w * 0.9];
        }
    } else {
        return None;
    }
    if low {
        p.hz = p.hz.map(|h| h * 0.7);
    }
    if high {
        p.hz = p.hz.map(|h| h * 1.4);
    }
    if soft {
        p.loud *= 0.5;
    }
    if loud {
        p.loud = p.loud.max(0.85);
    }
    if has(&["wet", "gurgl"]) {
        p.rough = p.rough.max(0.5);
        p.breath = p.breath.max(0.4);
    }
    if has(&["long", "slow", "drawn"]) {
        p.len *= 1.6;
        p.gap *= 1.5;
    }
    if has(&["short", "sharp", "quick"]) {
        p.len *= 0.6;
        p.swell = 0.0;
    }
    if has(&[" two ", "twice", "double"]) {
        p.times = p.times.max(2);
    } else if has(&["three", "series", "string of"]) {
        p.times = p.times.max(3);
    }
    if many && p.times < 4 && p.kind != Kind::Knock {
        p.times = 6;
        p.len = p.len.min(0.15);
        p.gap = p.gap.min(0.08);
    }
    p.sanitize();
    Some(Call(vec![p]))
}

/// The call for a species' `i`th sound: its own, written with it, or a
/// guess from the words.
pub fn species_call(sp: &crate::world::species::Species, i: usize) -> Option<Call> {
    if let Some(c) = sp.voice.get(i).filter(|c| !c.is_empty()) {
        return Some(c.clone());
    }
    let text = sp.sounds.get(i)?;
    guess(text, sp.mass, sp.moves.fly > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_calls_are_kept_in_range() {
        let c = Call::from_value(&serde_json::json!([
            { "kind": "voice", "hz": [600, 900, 400], "len": 0.6, "vowel": "iau", "rough": 7, "times": 300 },
            { "kind": "whistle", "hz": 3000, "len": -1 },
            "nonsense",
            { "kind": "wobbly", "hz": "high" }
        ]));
        assert_eq!(c.0.len(), 3);
        assert_eq!(c.0[0].rough, 1.0);
        assert!(c.0[0].span() <= 6.0);
        assert_eq!(c.0[1].hz, [3000.0; 3]);
        assert!(c.0[1].len > 0.0);
        // Unknown kind: a throat, at its body's own pitch.
        assert_eq!(c.0[2].kind, Kind::Voice);
        assert_eq!(c.0[2].hz, [0.0; 3]);
    }

    #[test]
    fn a_species_without_calls_still_parses_and_one_with_bad_calls_too() {
        let sp: crate::world::species::Species = serde_json::from_value(serde_json::json!({
            "name": "x", "body": "quadruped", "sounds": ["a low growl", "zzz"], "voice": [ 5, { "kind": "noise" } ]
        }))
        .unwrap();
        assert_eq!(sp.voice.len(), 2);
        // The first one was nonsense: the words decide.
        let c = species_call(&sp, 0).unwrap();
        assert_eq!(c.0[0].kind, Kind::Voice);
        assert!(c.0[0].rough > 0.5);
        assert_eq!(species_call(&sp, 1).unwrap().0[0].kind, Kind::Noise);
    }

    #[test]
    fn guesses_follow_the_words_and_the_body() {
        let cat = guess("meow", 4.5, false).unwrap();
        let lion = guess("meow", 190.0, false).unwrap();
        assert!(cat.0[0].hz[0] > lion.0[0].hz[0] * 2.0, "a bigger body, a lower voice");
        assert_eq!(guess("a sharp chirp", 0.1, true).unwrap().0[0].kind, Kind::Whistle);
        assert_eq!(guess("a wet click", 60.0, false).unwrap().0[0].kind, Kind::Knock);
        assert!(guess("a long, slow breath", 60.0, false).unwrap().0[0].breath > 0.9);
        assert!(guess("The air smells of copper.", 60.0, false).is_none());
    }

    #[test]
    fn material_follows_properties() {
        use crate::sim::props::*;
        let mut grass = vec![0.0; 32];
        grass[P_ALIVE] = 1.0;
        grass[P_BURNS] = 0.9;
        let g = Material::of(&grass, &["grass".into()], None);
        let mut leaves = grass.clone();
        leaves[P_ALIVE] = 0.0;
        let l = Material::of(&leaves, &[], None);
        let stone = Material::of(&vec![0.0; 32], &[], None);
        let mut metal = vec![0.0; 32];
        metal[P_CONDUCTS] = 1.0;
        let m = Material::of(&metal, &[], None);
        let mut glass = vec![0.0; 32];
        glass[P_FRAGILE] = 1.0;
        let gl = Material::of(&glass, &[], None);
        assert!(g.hard < 0.25 && g.dry < 0.4, "green grass is soft: {g:?}");
        assert!(l.dry > 0.8 && l.hard > g.hard, "dead leaves are dry and crisper: {l:?}");
        assert!(stone.hard > 0.7);
        // Conducting (heat, magic) is no reason to ring; brittle things do.
        assert!(m.ring < 0.1 && stone.ring < 0.1, "{m:?}");
        assert!(gl.ring > 0.25);
        let bell = Material::of(&vec![0.0; 32], &[], Material::from_meta(&[("ring".into(), 1.0), ("hard".into(), 0.9)]).as_ref());
        assert_eq!(bell.ring, 1.0);
    }
}
