//! Species: a body plus a row of numbers on shared axes (the taxonomy).
//! Everything that happens between species reads only these numbers, never
//! names: a wolf scares a goat because it eats meat and is bigger.

use crate::lang::ir::Body;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mind {
    /// Scoring only, never the LLM.
    Instinct,
    /// Rare LLM goals, no words.
    Simple,
    /// Full minds: plans, crafts, making things.
    #[default]
    Sapient,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Speech {
    None,
    /// Answers in noises and gestures.
    Sounds,
    #[default]
    Words,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Social {
    Solitary,
    Pair,
    Pack,
    Herd,
    #[default]
    Village,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Diet {
    #[serde(default)]
    pub plants: f32,
    #[serde(default)]
    pub meat: f32,
    /// Anything else they eat, by property (a dragon that eats `heat`).
    #[serde(default)]
    pub props: BTreeMap<String, f32>,
}

impl Default for Diet {
    fn default() -> Self {
        Diet { plants: 0.5, meat: 0.5, props: BTreeMap::new() }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Temper {
    #[serde(default = "half")]
    pub bold: f32,
    #[serde(default = "half")]
    pub wary: f32,
    #[serde(default = "half")]
    pub playful: f32,
    #[serde(default = "half")]
    pub tame: f32,
}

impl Default for Temper {
    fn default() -> Self {
        Temper { bold: 0.5, wary: 0.5, playful: 0.5, tame: 0.5 }
    }
}

fn half() -> f32 {
    0.5
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Moves {
    #[serde(default = "walk")]
    pub walk: f32,
    #[serde(default = "run")]
    pub run: f32,
    #[serde(default)]
    pub fly: f32,
    #[serde(default)]
    pub swim: f32,
}

fn walk() -> f32 {
    1.3
}

fn run() -> f32 {
    3.2
}

impl Default for Moves {
    fn default() -> Self {
        Moves { walk: 1.3, run: 3.2, fly: 0.0, swim: 0.0 }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Life {
    /// Hours of sleep: [from, to] (wrapping past midnight when from > to).
    #[serde(default = "sleep")]
    pub sleep: [f32; 2],
}

fn sleep() -> [f32; 2] {
    [21.0, 6.0]
}

impl Default for Life {
    fn default() -> Self {
        Life { sleep: sleep() }
    }
}

/// How fast each need drifts (1 = as for a person).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct NeedRates {
    #[serde(default = "one")]
    pub hunger: f32,
    #[serde(default = "one")]
    pub fatigue: f32,
    #[serde(default = "one")]
    pub social: f32,
    #[serde(default = "one")]
    pub fun: f32,
    #[serde(default = "one")]
    pub curiosity: f32,
}

impl Default for NeedRates {
    fn default() -> Self {
        NeedRates { hunger: 1.0, fatigue: 1.0, social: 1.0, fun: 1.0, curiosity: 1.0 }
    }
}

/// A named set of look ranges (and, later, default layers) within a species.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct Variety {
    pub name: String,
    #[serde(default)]
    pub look: BTreeMap<String, [f32; 2]>,
    #[serde(default)]
    pub layers: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Species {
    pub name: String,
    #[serde(default)]
    pub plural: String,
    /// Name of the body type.
    pub body: String,
    /// Instance scale: the body's size multiplier.
    #[serde(default = "one")]
    pub size: f32,
    #[serde(default)]
    pub mind: Mind,
    #[serde(default)]
    pub speech: Speech,
    #[serde(default)]
    pub diet: Diet,
    #[serde(default)]
    pub social: Social,
    #[serde(default)]
    pub temper: Temper,
    #[serde(rename = "move", default)]
    pub moves: Moves,
    #[serde(default)]
    pub life: Life,
    #[serde(default)]
    pub needs: NeedRates,
    /// Look ranges for this species (subsets of the body's slider ranges).
    #[serde(default)]
    pub look: BTreeMap<String, [f32; 2]>,
    #[serde(default)]
    pub varieties: Vec<Variety>,
    /// Noises it makes when it can't speak ("woof", "a low purr").
    #[serde(default)]
    pub sounds: Vec<String>,
    /// Mass in kg at full size.
    #[serde(default = "mass")]
    pub mass: f32,
    #[serde(default)]
    pub description: String,
}

fn mass() -> f32 {
    70.0
}

impl Species {
    pub fn is_human(&self) -> bool {
        self.name == "human"
    }
    pub fn plural(&self) -> String {
        if self.plural.is_empty() { format!("{}s", self.name) } else { self.plural.clone() }
    }
    pub fn group(&self) -> bool {
        matches!(self.social, Social::Pack | Social::Herd)
    }
    /// Asleep at this hour?
    pub fn sleeps_at(&self, hour: f32) -> bool {
        let [a, b] = self.life.sleep;
        if a <= b { hour >= a && hour < b } else { hour >= a || hour < b }
    }
}

/// All species of a universe: the built-ins plus its own.
#[derive(Clone, Debug, Default)]
pub struct SpeciesBook {
    pub list: Vec<Arc<Species>>,
    /// The universe's size multiplier per species ("everyone is a giant").
    pub sizes: BTreeMap<String, f32>,
    /// The traveller's height (m).
    pub traveller_height: Option<f32>,
}

pub const BUILTIN_SPECIES: &str = include_str!("../builtin/species.json");

impl SpeciesBook {
    pub fn builtin() -> SpeciesBook {
        let list: Vec<Species> = serde_json::from_str(BUILTIN_SPECIES).expect("builtin species.json");
        SpeciesBook { list: list.into_iter().map(Arc::new).collect(), ..Default::default() }
    }

    pub fn size_of(&self, species: &str) -> f32 {
        self.sizes.get(species).copied().filter(|v| v.is_finite()).unwrap_or(1.0).clamp(0.2, 6.0)
    }

    /// Add or replace a species by name.
    pub fn add(&mut self, s: Species) {
        let s = Arc::new(s);
        match self.list.iter().position(|x| x.name == s.name) {
            Some(i) => self.list[i] = s,
            None => self.list.push(s),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Arc<Species>> {
        let n = name.trim().to_lowercase();
        self.list.iter().find(|s| s.name == n || s.plural().to_lowercase() == n)
    }

    /// The species for a persona's word ("" is human).
    pub fn of(&self, name: &str) -> Arc<Species> {
        let n = if name.trim().is_empty() { "human" } else { name };
        self.get(n).or_else(|| self.get("human")).cloned().unwrap_or_else(|| Arc::new(human_fallback()))
    }
}

fn human_fallback() -> Species {
    serde_json::from_value(serde_json::json!({ "name": "human", "body": "figure" })).unwrap()
}

/// Character look sliders by name. Old personas used `shirt_hue` and
/// `trousers_hue`; both spellings are read.
pub type LookMap = BTreeMap<String, f32>;

pub fn look_value(look: &LookMap, name: &str) -> Option<f32> {
    let alias = match name {
        "shirt" => Some("shirt_hue"),
        "trousers" => Some("trousers_hue"),
        _ => None,
    };
    look.get(name).or_else(|| alias.and_then(|a| look.get(a))).copied().filter(|v| v.is_finite())
}

/// Size numbers for one body, at one look and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dims {
    pub height: f32,
    pub eye: f32,
    pub radius: f32,
    pub reach: f32,
    /// Local, scaled (x right, y up, z forward).
    pub grip: [f32; 3],
    pub seat: Option<[f32; 3]>,
    /// Body lengths per body-default length (look height × scale).
    pub ratio: f32,
    /// The instance scale the renderer uses.
    pub scale: f32,
    pub arms: bool,
    pub flies: bool,
    pub mass: f32,
}

impl Default for Dims {
    fn default() -> Self {
        Dims::of(&Body { arms: true, ..Body::default() }, 1.75, 1.0, 70.0)
    }
}

impl Dims {
    /// `height`: the look's height when the body has a height slider (the
    /// shape draws it itself), otherwise the body's own height.
    pub fn of(b: &Body, height: f32, scale: f32, mass: f32) -> Dims {
        let r = height / b.height.max(0.01) * scale;
        let s3 = |p: [f32; 3]| [p[0] * r, p[1] * r, p[2] * r];
        Dims {
            height: b.height * r,
            eye: b.eye * r,
            radius: b.radius * r,
            // Reach shrinks slower than size: a cat still reaches its toy.
            reach: (b.reach * r.sqrt()).max(0.6),
            grip: s3(b.grip),
            seat: b.seat.map(s3),
            ratio: r,
            scale,
            arms: b.arms,
            flies: b.flies,
            mass,
        }
    }
}

/// The k.a … k.e values of a character's body: their own sliders, else a
/// value chosen from the species (or variety) range by `seed`.
pub fn sliders(b: &Body, sp: &Species, variety: Option<&Variety>, look: &LookMap, seed: u32) -> [f32; 5] {
    let mut out = [0.5f32; 5];
    for (i, (name, lo, hi)) in b.look.iter().enumerate().take(5) {
        let range = variety.and_then(|v| v.look.get(name)).or_else(|| sp.look.get(name)).copied().unwrap_or([*lo, *hi]);
        let v = look_value(look, name).unwrap_or_else(|| {
            let h = crate::noise::u2f(crate::noise::pcg(seed ^ (i as u32 + 1).wrapping_mul(0x9E37_79B9)));
            range[0] + (range[1] - range[0]) * h
        });
        out[i] = v.clamp(lo.min(*hi), hi.max(*lo));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_species_parse_and_old_looks_read() {
        let b = SpeciesBook::builtin();
        for n in ["human", "dog", "cat", "horse", "wolf", "goat", "deer"] {
            assert!(b.get(n).is_some(), "{n}");
        }
        assert!(b.of("").is_human());
        assert_eq!(b.get("dogs").unwrap().name, "dog");
        let mut look = LookMap::new();
        look.insert("shirt_hue".into(), 0.25);
        assert_eq!(look_value(&look, "shirt"), Some(0.25));
        assert!(b.get("human").unwrap().sleeps_at(23.0) && !b.get("human").unwrap().sleeps_at(12.0));
    }
}
