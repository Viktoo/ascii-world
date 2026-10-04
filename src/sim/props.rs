//! The property vocabulary: a small shared set of numbers every thing has
//! (mass, burns, temp, wet, …). The LLM assigns them, the rules read and
//! change them, and the engine turns some of them into looks and physics.
//! A universe's bible can add its own properties on top of the built-ins.

use crate::lang::ir::{CTX_FIELDS, valid_prop_name};
use crate::world::TypeEntry;
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
];

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
    index: HashMap<String, usize>,
}

impl Default for Vocab {
    fn default() -> Self {
        Self::builtin()
    }
}

impl Vocab {
    pub fn builtin() -> Vocab {
        let mut v = Vocab { names: Vec::new(), defaults: Vec::new(), meanings: Vec::new(), index: HashMap::new() };
        for (n, d, m) in BUILTIN {
            v.push(n, *d, m);
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
        Ok(self.names.len() - 1)
    }

    pub fn id(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_builtin(&self, i: usize) -> bool {
        i < BUILTIN.len()
    }

    /// One line per property, for prompts.
    pub fn describe(&self) -> String {
        self.names.iter().zip(&self.meanings).zip(&self.defaults).map(|((n, m), d)| format!("- {n}: {m} (default {d})")).collect::<Vec<_>>().join("\n")
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
    if tag(ty, "glass") || tag(ty, "pottery") {
        p[P_FRAGILE] = 0.6;
    }
    for (name, v) in &ty.ct.meta.props {
        if let Some(i) = vocab.id(name) {
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
        if (v - b).abs() > 1e-4 {
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
    for i in [P_FIRE, P_WET, P_CHAR, P_GROWTH, P_BOUNCE, P_FRICTION, P_BURNS, P_FRAGILE, P_CONDUCTS, P_STRANGE, P_TOY, P_CORRUPT] {
        p[i] = p[i].clamp(0.0, 1.0);
    }
    p[P_LIGHT] = p[P_LIGHT].clamp(0.0, 2.0);
    p[P_FUEL] = p[P_FUEL].max(0.0);
    p[P_HEALTH] = p[P_HEALTH].min(1.0);
    p[P_MASS] = p[P_MASS].max(0.001);
    p[P_TEMP] = p[P_TEMP].clamp(-80.0, 1500.0);
}
