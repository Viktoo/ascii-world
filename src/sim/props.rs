//! The property vocabulary: a small shared set of numbers every thing has
//! (mass, burns, temp, wet, …). The LLM assigns them, the rules read and
//! change them, and the engine turns some of them into looks and physics.
//! A universe's bible can add its own properties on top of the built-ins.

use crate::lang::ir::{CTX_FIELDS, valid_prop_name};
use crate::world::TypeEntry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub const P_MASS: usize = 0;
pub const P_BOUNCE: usize = 1;
pub const P_FRICTION: usize = 2;
pub const P_SOLID: usize = 3;
pub const P_BURNS: usize = 4;
pub const P_TEMP: usize = 5;
pub const P_WET: usize = 6;
pub const P_LIGHT: usize = 7;
pub const P_EDIBLE: usize = 8;
pub const P_ALIVE: usize = 9;
pub const P_FRAGILE: usize = 10;
pub const P_CONDUCTS: usize = 11;
pub const P_FIRE: usize = 12;
pub const P_FUEL: usize = 13;
pub const P_HEAT: usize = 14;
pub const P_CHAR: usize = 15;
pub const P_HEALTH: usize = 16;
pub const P_GROWTH: usize = 17;
pub const P_STRANGE: usize = 18;
pub const P_TOY: usize = 19;
pub const P_CORRUPT: usize = 20;
pub const P_FORCE: usize = 21;
pub const P_MARK: usize = 22;

/// Act properties: what an action is doing right now, not what a thing is
/// (`force` while someone works a tool). The engine sets them for a moment;
/// rules may read them, but nothing generated (types, deeds, universe rules)
/// can write them, and they are never saved.
pub const ACT: &[usize] = &[P_FORCE];

pub const AMBIENT_TEMP: f32 = 15.0;
/// What one person can lift (kg); two together lift twice that.
pub const STRENGTH: f32 = 25.0;
/// Heavier than this never moves (buildings, boulders, big trees).
pub const ANCHOR_MASS: f32 = 2000.0;

/// (name, default, meaning). The order defines the P_* indices above.
pub const BUILTIN: &[(&str, f32, &str)] = &[
    ("mass", 1.0, "kilograms"),
    ("bounce", 0.3, "how much it bounces, 0..1"),
    ("friction", 0.5, "how grippy it is, 0..1"),
    ("solid", 1.0, "1 if things collide with it"),
    ("burns", 0.0, "how readily it catches fire, 0..1"),
    ("temp", AMBIENT_TEMP, "temperature, °C"),
    ("wet", 0.0, "how wet it is, 0..1"),
    ("light", 0.0, "how much light it gives off, 0..1"),
    ("edible", 0.0, "how filling it is to eat, 0..1"),
    ("alive", 0.0, "1 for living plants and creatures"),
    ("fragile", 0.0, "how easily it breaks when hit, 0..1"),
    ("conducts", 0.0, "how well it passes on heat, 0..1"),
    ("fire", 0.0, "how fiercely it is burning now, 0..1"),
    ("fuel", 1.0, "how much is left to burn, 0..1"),
    ("heat", 0.0, "a steady temperature it keeps itself at, °C (a stove, a lamp)"),
    ("char", 0.0, "how charred it is, 0..1"),
    ("health", 1.0, "how intact it is; at 0 it breaks"),
    ("growth", 1.0, "how grown it is, 0..1 (living things grow)"),
    ("strange", 0.0, "how out of place it is in this world, 0..1: 0 is everyday here, 1 unheard of (a motor car among horse carts 0.9, a glowing rune stone where there is no magic 0.8); people are surprised by strange things"),
    ("toy", 0.0, "how much people play with it, 0..1: toss, catch, kick or roll it about for fun (a ball 1, a hoop 0.8, knucklebones 0.6; whatever this people plays with)"),
    ("corruption", 0.0, "how much darkness has got into it, 0..1: corrupted things darken as if the night got into them, dim the light around them and work a little wrong"),
    ("force", 0.0, "how hard it is being worked against what it touches right now, 0..1: a beating, a rubbing, a pressing (someone applying it; nothing has it by itself)"),
    ("mark", 0.0, "how much people aim thrown things at it, 0..1: a hoop or a goal 1, a bell to ring or a bucket to toss into 0.8, a fence post 0.2"),
];

/// A threshold crossing worth telling ("the grass tuft caught fire").
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Crossing {
    /// The event kind ("ignited").
    pub kind: String,
    /// What it did, after its name ("caught fire").
    pub text: String,
    /// Only when this holds (a rule condition on `self`, after the change):
    /// "self.fuel <= 0" tells burning out apart from being put out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    /// It stopped before running its course: what was saved.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub saved: bool,
}

/// How an incident of a spreading property is told ("Wildfire from the kiln:
/// 40 burnt, 3 saved, 6 burning").
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Words {
    /// "fire".
    pub noun: String,
    /// When it has grown big: "wildfire".
    #[serde(default)]
    pub big: String,
    /// Parts it has now: "burning".
    #[serde(default)]
    pub active: String,
    /// Parts it ran its course on: "burnt".
    #[serde(default)]
    pub spent: String,
    /// Over by itself: "burnt itself out".
    #[serde(default)]
    pub ended: String,
    /// Stopped by people: "put out".
    #[serde(default)]
    pub stopped: String,
}

impl Words {
    fn or(&self, s: &str, d: &str) -> String {
        if s.trim().is_empty() { d.to_string() } else { s.to_string() }
    }
    pub fn big(&self) -> String {
        self.or(&self.big, &self.noun)
    }
    pub fn active(&self) -> String {
        self.or(&self.active, "going")
    }
    pub fn spent(&self) -> String {
        self.or(&self.spent, "done")
    }
    pub fn ended(&self) -> String {
        self.or(&self.ended, "ran its course")
    }
    pub fn stopped(&self) -> String {
        self.or(&self.stopped, "was stopped")
    }
}

/// What the engine needs to know about a property beyond its number: when a
/// change is news, whether it spreads as an incident, whether it harms.
/// Built-in physics has it written here; a universe's own properties get it
/// from genesis, so a curse is told, fought and fled like a fire.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PropMeta {
    /// The level that counts as having it (0: any at all).
    #[serde(default)]
    pub at: f32,
    /// Rising past `at`: first that holds is told.
    #[serde(default)]
    pub rises: Vec<Crossing>,
    /// Falling back to `at`.
    #[serde(default)]
    pub falls: Vec<Crossing>,
    /// Not news when this holds (the burning is the news, not the scorching).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quiet: Option<String>,
    /// It spreads: crossings with one cause are one incident, told so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident: Option<Words>,
    /// How much harm it does to be near or wear something that has it, per
    /// unit (0: harmless). People keep away and get it off them.
    #[serde(default)]
    pub hazard: f32,
    /// Kept when a thing turns into another (a cursed log becomes a cursed boat).
    #[serde(default)]
    pub keep: bool,
    /// Its range, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[f32; 2]>,
    /// Units and anchors ("0.2 a blessed candle, 1 a saint's relic"), shown
    /// wherever values are written, so every type and deed uses one scale.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scale: String,
}

fn cross(kind: &str, text: &str, when: Option<&str>, saved: bool) -> Crossing {
    Crossing { kind: kind.into(), text: text.into(), when: when.map(str::to_string), saved }
}

/// Anchors for the built-ins' scales: a few known things at known values.
fn builtin_scale(name: &str) -> &'static str {
    match name {
        "mass" => "an apple 0.2, a ball 0.6, a bucket of water 8, a person 70, a cart 300, a hut 20000",
        "bounce" => "a sandbag 0, a wooden block 0.3, a leather ball 0.8",
        "friction" => "ice 0.05, polished wood 0.3, a rope 0.8",
        "burns" => "stone 0, a wooden hut 0.4, a dry log 0.8, dry grass 0.9",
        "temp" => "snow -5, a summer day 25, a body 36, boiling water 100, a kiln 900",
        "wet" => "dry 0, damp cloth 0.3, a soaked cloak 1",
        "light" => "a candle 0.3, a lantern 1, a bonfire 1.5",
        "edible" => "a turnip 0.2, an apple 0.4, a loaf 0.6, a stew 0.9",
        "fragile" => "stone 0, a clay pot 0.6, glass 0.9",
        "conducts" => "wood 0.05, stone 0.2, iron 0.8",
        "heat" => "0 for most things, a lantern 120, a stove 300, a forge 900",
        _ => "",
    }
}

/// The built-in properties' metadata (fire spreads and harms; living things
/// die and grow up; wetness, heat and char carry over when things change).
fn builtin_meta(name: &str) -> PropMeta {
    let scale = builtin_scale(name).to_string();
    let m = builtin_meta_of(name);
    PropMeta { scale, ..m }
}

fn builtin_meta_of(name: &str) -> PropMeta {
    match name {
        "fire" => PropMeta {
            rises: vec![cross("ignited", "caught fire", None, false)],
            falls: vec![cross("burnt_out", "burnt out", Some("self.fuel <= 0"), false), cross("doused", "was put out", None, true)],
            incident: Some(Words { noun: "fire".into(), big: "wildfire".into(), active: "burning".into(), spent: "burnt".into(), ended: "burnt itself out".into(), stopped: "put out".into() }),
            hazard: 1.0,
            keep: true,
            ..Default::default()
        },
        "alive" => PropMeta { falls: vec![cross("died", "died", None, false)], quiet: Some("self.fire > 0 || (self.burns > 0 && self.temp > 100)".into()), ..Default::default() },
        "growth" => PropMeta { at: 0.999, rises: vec![cross("grown", "is fully grown", None, false)], ..Default::default() },
        "temp" | "wet" | "char" | "corruption" => PropMeta { keep: true, ..Default::default() },
        _ => PropMeta::default(),
    }
}

/// Light enough to toss, and something people play with: a ball, a hoop,
/// or whatever this people throws about for fun.
pub fn is_toy(p: &[f32]) -> bool {
    p[P_MASS] <= 5.0 && (p[P_TOY] >= 0.4 || (p[P_BOUNCE] >= 0.45 && p[P_MASS] <= 3.0))
}

/// Every property name this universe knows, with defaults.
#[derive(Clone, Debug)]
pub struct Vocab {
    pub names: Vec<String>,
    pub defaults: Vec<f32>,
    pub meanings: Vec<String>,
    pub meta: Vec<PropMeta>,
    index: HashMap<String, usize>,
    /// Near-miss names this world has mapped to one of its properties
    /// ("curse" → `cursed`), learned once and kept with the world.
    aliases: HashMap<String, usize>,
}

/// A property name as written, made comparable: "Is Cursed" → "is_cursed".
pub fn norm_name(s: &str) -> String {
    s.trim().to_lowercase().replace([' ', '-'], "_")
}

impl Default for Vocab {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Vocab {
    pub fn builtin() -> Vocab {
        let mut v = Vocab { names: Vec::new(), defaults: Vec::new(), meanings: Vec::new(), meta: Vec::new(), index: HashMap::new(), aliases: HashMap::new() };
        for (n, d, m) in BUILTIN {
            v.push(n, *d, m);
            v.meta.push(builtin_meta(n));
        }
        v
    }

    fn push(&mut self, name: &str, default: f32, meaning: &str) {
        self.index.insert(name.to_string(), self.names.len());
        self.names.push(name.to_string());
        self.defaults.push(default);
        self.meanings.push(meaning.to_string());
    }

    /// Add a universe property. Returns an error message for bad names.
    pub fn add(&mut self, name: &str, default: f32, meaning: &str) -> Result<usize, String> {
        if let Some(i) = self.index.get(name) {
            return Ok(*i);
        }
        if !valid_prop_name(name) || CTX_FIELDS.contains(&name) || crate::sim::rules::RESERVED.contains(&name) {
            return Err(format!("'{name}' is not a valid property name"));
        }
        if self.names.len() >= 64 {
            return Err("too many properties".into());
        }
        if !default.is_finite() {
            return Err(format!("{name}: default must be a number"));
        }
        self.push(name, default, meaning);
        // A universe's own property carries over when things change, unless told otherwise.
        self.meta.push(PropMeta { keep: true, ..Default::default() });
        Ok(self.names.len() - 1)
    }

    /// Set what the engine knows about a universe property (not the built-ins').
    pub fn set_meta(&mut self, name: &str, meta: PropMeta) {
        if let Some(i) = self.id(name).filter(|i| !self.is_builtin(*i)) {
            self.meta[i] = meta;
        }
    }

    /// Properties that spread as incidents.
    pub fn spreading(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.len()).filter(|i| self.meta[*i].incident.is_some())
    }

    /// Properties that harm.
    pub fn hazards(&self) -> impl Iterator<Item = (usize, f32)> + '_ {
        (0..self.len()).filter(|i| self.meta[*i].hazard > 0.0).map(|i| (i, self.meta[i].hazard))
    }

    /// How harmful something with these properties is to be near (0 = not at all).
    pub fn harm(&self, p: &[f32]) -> f32 {
        self.hazards().map(|(i, h)| p.get(i).copied().unwrap_or(0.0).max(0.0) * h).sum()
    }

    /// Keep universe properties in their ranges.
    pub fn clamp(&self, p: &mut [f32]) {
        for (i, m) in self.meta.iter().enumerate().skip(BUILTIN.len()) {
            if let (Some([lo, hi]), Some(v)) = (m.range, p.get_mut(i)) {
                *v = v.clamp(lo, hi);
            }
        }
    }

    pub fn id(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    /// A name as something generated wrote it: exact, then tidied, then a
    /// known alias of this world.
    pub fn lookup(&self, name: &str) -> Option<usize> {
        self.id(name).or_else(|| {
            let n = norm_name(name);
            self.id(&n).or_else(|| self.aliases.get(&n).copied())
        })
    }

    /// Remember that `from` means property `to` in this world.
    pub fn add_alias(&mut self, from: &str, to: &str) {
        if let Some(i) = self.id(to) {
            self.aliases.insert(norm_name(from), i);
        }
    }

    /// An act property (see `ACT`): read by rules, set only by the engine.
    pub fn is_act(&self, i: usize) -> bool {
        ACT.contains(&i)
    }

    /// A property generated things may give values to.
    pub fn writable(&self, i: usize) -> bool {
        i < self.len() && !self.is_act(i)
    }

    /// The names generated things may give values to.
    pub fn writable_names(&self) -> Vec<String> {
        (0..self.len()).filter(|i| self.writable(*i)).map(|i| self.names[i].clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_builtin(&self, i: usize) -> bool {
        i < BUILTIN.len()
    }

    /// One line per property generated things may set, with its scale's
    /// anchors, for every prompt that writes values.
    pub fn describe(&self) -> String {
        (0..self.len())
            .filter(|i| self.writable(*i))
            .map(|i| {
                let scale = &self.meta[i].scale;
                let anchors = if scale.trim().is_empty() { String::new() } else { format!("; for scale: {}", scale.trim()) };
                format!("- {}: {} (default {}{anchors})", self.names[i], self.meanings[i], self.defaults[i])
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Property values, indexed by vocabulary id.
pub type Props = Vec<f32>;

fn tag(ty: &TypeEntry, t: &str) -> bool {
    ty.has_tag(t)
}

/// Rough volume of a type at scale 1 (m³): the bounds box, partly filled.
pub fn volume(ty: &TypeEntry) -> f32 {
    let b = ty.ct.meta.bounds;
    (b[0] * b[1] * b[2] * 8.0 * 0.35).max(1e-4)
}

/// Initial properties of a type at scale 1: meta.props where given, otherwise
/// sensible defaults from its tags and size.
pub fn type_props(vocab: &Vocab, ty: &TypeEntry) -> Props {
    let mut p = vocab.defaults.clone();
    let structure = tag(ty, "building") || tag(ty, "landmark") || tag(ty, "bridge") || tag(ty, "pier") || tag(ty, "wall");
    let density = if tag(ty, "rock") || tag(ty, "stone") || tag(ty, "metal") {
        2500.0
    } else if tag(ty, "tree") || tag(ty, "pine") || tag(ty, "wood") || tag(ty, "stick") {
        600.0
    } else if tag(ty, "grass") || tag(ty, "flower") || tag(ty, "plant") {
        60.0
    } else {
        450.0
    };
    p[P_MASS] = if structure { ANCHOR_MASS * 10.0 } else { volume(ty) * density };
    if !ty.solid {
        p[P_SOLID] = 0.0;
    }
    let plant = ["tree", "pine", "bush", "grass", "flower", "plant", "reed", "crop"].iter().any(|t| tag(ty, t));
    if plant {
        p[P_ALIVE] = 1.0;
        p[P_BURNS] = if tag(ty, "grass") { 0.9 } else { 0.6 };
    }
    if tag(ty, "wood") || tag(ty, "stick") {
        p[P_BURNS] = 0.8;
    }
    if tag(ty, "food") || tag(ty, "fruit") || tag(ty, "mushroom") {
        p[P_EDIBLE] = 0.3;
    }
    if tag(ty, "light") || tag(ty, "lamp") || tag(ty, "lantern") {
        p[P_LIGHT] = 1.0;
    }
    if tag(ty, "toy") || tag(ty, "ball") || tag(ty, "game") {
        p[P_TOY] = 1.0;
    }
    // Without props from its maker, a name says what people aim at.
    let name = ty.name().to_lowercase();
    if ["hoop", "goal", "target", "basket", "net", "ring", "bell", "bucket", "bin", "barrel"].iter().any(|w| tag(ty, w) || name.contains(w)) {
        p[P_MARK] = 1.0;
    }
    if tag(ty, "glass") || tag(ty, "pottery") {
        p[P_FRAGILE] = 0.6;
    }
    for (name, v) in &ty.ct.meta.props {
        if let Some(i) = vocab.lookup(name).filter(|i| vocab.writable(*i)) {
            p[i] = *v;
        }
    }
    p[P_MASS] = p[P_MASS].max(0.001);
    p
}

/// Properties at a given scale (mass grows with volume).
pub fn scaled(mut p: Props, scale: f32) -> Props {
    p[P_MASS] *= scale.max(0.01).powi(3);
    p
}

/// Cache of per-type initial properties.
#[derive(Default)]
pub struct TypeProps {
    by_type: HashMap<u32, Arc<Props>>,
    vocab_len: usize,
}

impl TypeProps {
    pub fn get(&mut self, vocab: &Vocab, ty: &TypeEntry) -> Arc<Props> {
        if self.vocab_len != vocab.len() {
            self.by_type.clear();
            self.vocab_len = vocab.len();
        }
        self.by_type.entry(ty.id).or_insert_with(|| Arc::new(type_props(vocab, ty))).clone()
    }

    pub fn clear(&mut self) {
        self.by_type.clear();
    }
}

/// Properties that differ from `base`, by name (for saving).
pub fn diff(vocab: &Vocab, p: &Props, base: &Props) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    for (i, v) in p.iter().enumerate() {
        let b = base.get(i).copied().unwrap_or(0.0);
        if (v - b).abs() > 1e-4 && !vocab.is_act(i) {
            if let Some(n) = vocab.names.get(i) {
                m.insert(n.clone(), serde_json::json!((*v as f64 * 1000.0).round() / 1000.0));
            }
        }
    }
    m
}

/// Apply saved differences on top of `base`.
pub fn apply_diff(vocab: &Vocab, base: &Props, json: &str) -> Props {
    let mut p = base.clone();
    p.resize(vocab.len(), 0.0);
    if let Ok(serde_json::Value::Object(m)) = serde_json::from_str::<serde_json::Value>(json) {
        for (k, v) in m {
            if let (Some(i), Some(x)) = (vocab.id(&k), v.as_f64()) {
                if x.is_finite() {
                    p[i] = x as f32;
                }
            }
        }
    }
    p
}

/// All properties by name (for inspect).
pub fn named(vocab: &Vocab, p: &Props) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    for (i, v) in p.iter().enumerate() {
        if let Some(n) = vocab.names.get(i) {
            m.insert(n.clone(), serde_json::json!((*v as f64 * 1000.0).round() / 1000.0));
        }
    }
    m
}

/// Keep values in range after rules or behaviour code changed them.
pub fn sanitize(p: &mut Props) {
    for v in p.iter_mut() {
        if !v.is_finite() {
            *v = 0.0;
        }
        *v = v.clamp(-1e5, 1e5);
    }
    for i in [P_FIRE, P_WET, P_CHAR, P_GROWTH, P_BOUNCE, P_FRICTION, P_BURNS, P_FRAGILE, P_CONDUCTS, P_STRANGE, P_TOY, P_CORRUPT, P_FORCE, P_MARK] {
        p[i] = p[i].clamp(0.0, 1.0);
    }
    p[P_LIGHT] = p[P_LIGHT].clamp(0.0, 2.0);
    p[P_FUEL] = p[P_FUEL].max(0.0);
    p[P_HEALTH] = p[P_HEALTH].min(1.0);
    p[P_MASS] = p[P_MASS].max(0.001);
    p[P_TEMP] = p[P_TEMP].clamp(-80.0, 1500.0);
}
