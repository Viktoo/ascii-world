//! The clothes a body draws itself: look sliders that say how far a top,
//! a skirt, trousers, shoes, a hat or underwear cover (a person's top_cover,
//! skirt, legs, shoes, hat, underwear). They come off and go back on by
//! name, like a worn layer, and what came off is remembered with the person
//! (as `was_<slider>` in their look) so it can be put back as it was.

use super::actions::{fail, ActErr, Outcome};

use super::{ActorId, Sim};
use crate::world::species::LookMap;
use serde_json::json;

/// Outermost first: what comes off when nothing is named.
const GARMENTS: [(&str, &[&str]); 6] = [
    ("hat", &["hat", "cap", "kerchief", "headscarf", "scarf", "bonnet", "hood", "turban", "beret", "headdress"]),
    ("top_cover", &["shirt", "top", "blouse", "tunic", "bodice", "vest", "tank", "jersey", "sweater", "smock", "chemise", "jacket", "dress", "gown", "robe", "kimono", "clothes"]),
    ("skirt", &["skirt", "dress", "gown", "robe", "kilt", "sarong", "petticoat", "kimono", "clothes"]),
    ("legs", &["trousers", "pants", "breeches", "shorts", "hose", "leggings", "jeans", "clothes"]),
    ("shoes", &["shoes", "shoe", "boots", "boot", "sandals", "clogs", "slippers", "footwear"]),
    ("underwear", &["underwear", "undergarments", "underclothes", "briefs", "bra", "smallclothes", "loincloth", "drawers", "knickers", "naked", "everything"]),
];

/// A first guess at a person's face, hair and clothes from their words (no
/// LLM yet, or none): only what their look doesn't say already.
pub fn guess(p: &crate::world::Persona) -> LookMap {
    let text = format!(" {} {} {} ", p.appearance, p.personality, p.goals).to_lowercase();
    let words: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let has = |ws: &[&str]| ws.iter().any(|w| words.contains(w));
    let says = |s: &str| text.contains(s);
    let count = |ws: &[&str]| words.iter().filter(|w| ws.contains(w)).count();
    let her = count(&["she", "her", "hers", "woman", "girl", "mother", "daughter", "wife", "widow", "grandmother", "lady", "sister", "aunt", "maid"]);
    let him = count(&["he", "his", "him", "man", "boy", "father", "son", "husband", "widower", "grandfather", "lord", "brother", "uncle", "lad"]);
    let mut g = LookMap::new();
    let woman = her > him;
    if her != him {
        g.insert("shape".into(), if woman { 0.95 } else { 0.05 });
    }
    let colour = if has(&["grey", "gray", "white", "silver", "silvered", "grizzled"]) || p.age >= 66 {
        Some(1.0)
    } else if has(&["blond", "blonde", "golden", "flaxen", "fair"]) {
        Some(0.75)
    } else if says("black hair") || says("raven") {
        Some(0.02)
    } else if says("dark hair") || says("dark braid") || says("dark curl") || says("dark braided") {
        Some(0.25)
    } else if has(&["auburn", "ginger", "redhead"]) || says("red hair") || says("copper hair") {
        Some(0.6)
    } else if says("brown hair") {
        Some(0.45)
    } else {
        None
    };
    if let Some(c) = colour {
        g.insert("hair_color".into(), c);
    }
    let len = if has(&["bald", "shaven", "shaved"]) {
        0.0
    } else if has(&["braid", "braids", "braided", "plait", "plaits"]) {
        g.insert("hair_style".into(), 0.8);
        0.85
    } else if says("long hair") || has(&["mane", "tresses"]) {
        0.9
    } else if has(&["cropped", "crew"]) || says("short hair") {
        0.18
    } else if woman {
        0.75
    } else {
        0.25
    };
    g.insert("hair".into(), len);
    if has(&["curly", "curls", "curled", "ringlets"]) {
        g.insert("hair_style".into(), 0.5);
    } else if has(&["bun", "ponytail", "tied"]) {
        g.insert("hair_style".into(), 0.8);
    }
    if !woman {
        let beard = if says("long beard") || says("great beard") { 0.95 } else if has(&["beard", "bearded"]) { 0.6 } else if has(&["stubble", "unshaven"]) { 0.2 } else { 0.05 };
        g.insert("beard".into(), beard);
    }
    let hat = if says("straw hat") || has(&["sombrero"]) {
        Some(0.8)
    } else if has(&["kerchief", "headscarf", "scarf", "bandana", "headcloth"]) {
        Some(0.2)
    } else if has(&["cap", "beret"]) {
        Some(0.4)
    } else if has(&["hat"]) {
        Some(0.6)
    } else {
        None
    };
    if let Some(h) = hat {
        g.insert("hat".into(), h);
    }
    if has(&["dress", "gown", "robe", "robes", "habit", "cassock", "kimono"]) {
        g.insert("skirt".into(), 0.9);
        g.insert("legs".into(), 0.0);
    } else if has(&["skirt", "skirts"]) {
        g.insert("skirt".into(), 0.7);
        g.insert("legs".into(), 0.0);
    } else if woman {
        // Most of the world's past put women in skirts.
        g.insert("skirt".into(), 0.75);
        g.insert("legs".into(), 0.0);
    }
    if has(&["shorts"]) {
        g.insert("legs".into(), 0.25);
    }
    if has(&["shirtless", "topless"]) || says("bare-chested") || says("bare chested") {
        g.insert("top_cover".into(), 0.0);
    }
    if says("sleeves rolled") || says("rolled sleeves") || says("rolled-up sleeves") || says("rolled to") {
        g.insert("sleeves".into(), 0.45);
    }
    let shoes = if has(&["barefoot"]) || says("bare feet") { Some(0.0) } else if has(&["sandals", "sandal"]) { Some(0.2) } else if has(&["boots", "boot"]) { Some(0.75) } else { None };
    if let Some(s) = shoes {
        g.insert("shoes".into(), s);
    }
    g.retain(|k, _| crate::world::species::look_value(&p.look, k).is_none());
    g
}

/// Below this a garment isn't worn (a hat's smallest is a kerchief).
fn on(slider: &str, v: f32) -> bool {
    v > if slider == "hat" { 0.08 } else { 0.04 }
}

/// The garment sliders a word names ("his boots" → shoes; "her dress" → top
/// and skirt; "clothes" → all but underwear).
pub fn named(what: &str) -> Vec<&'static str> {
    let w = what.to_lowercase();
    let words: Vec<&str> = w.split(|c: char| !c.is_alphabetic()).filter(|s| !s.is_empty()).collect();
    GARMENTS.iter().filter(|(_, names)| names.iter().any(|n| words.contains(n))).map(|(s, _)| *s).collect()
}

/// What a garment is called, from how it is cut.
fn call(slider: &str, look: &dyn Fn(&str) -> f32) -> &'static str {
    match slider {
        "hat" => match look("hat") {
            v if v < 0.3 => "kerchief",
            v if v < 0.5 => "cap",
            _ => "hat",
        },
        "top_cover" if look("skirt") > 0.04 => "dress",
        "top_cover" => match look("top_cover") {
            v if v <= 0.28 => "band",
            v if v > 0.82 => "tunic",
            _ => "shirt",
        },
        "skirt" => "skirt",
        "legs" if look("legs") < 0.35 => "shorts",
        "legs" => "trousers",
        "shoes" => match look("shoes") {
            v if v < 0.3 => "sandals",
            v if v > 0.55 => "boots",
            _ => "shoes",
        },
        _ => "underwear",
    }
}

impl Sim {
    /// People whose look has no word yet on their face and clothes (made
    /// before bodies drew them): asked of the LLM, a few at a time.
    pub fn ask_wardrobe(&mut self) {
        if !self.has_llm {
            return;
        }
        let mut people = Vec::new();
        for n in self.cast.npcs.iter().filter(|n| !n.dead) {
            let id = n.def.id;
            if n.def.persona.look.contains_key("shape") || self.interp.dress_asked.contains(&id) {
                continue;
            }
            let Some(b) = self.snap.type_of(n.body_ty).and_then(|t| t.ct.meta.body.as_ref()) else { continue };
            if b.slider("shape").is_none() || b.slider("top_cover").is_none() {
                continue;
            }
            let p = &n.def.persona;
            let line = json!({ "id": id, "name": p.name, "age": p.age, "appearance": p.appearance, "work": p.goals, "species": if p.species.is_empty() { "human" } else { &p.species } });
            people.push((id, line.to_string()));
            if people.len() >= 5 {
                break;
            }
        }
        if people.is_empty() {
            return;
        }
        for (id, _) in &people {
            self.interp.dress_asked.insert(*id);
        }
        self.request_now(super::Request::Dress { people });
    }

    /// The looks the LLM read for people (see `ask_wardrobe`): kept with
    /// them. Their height, build and skin stay as they were.
    pub fn on_dressed(&mut self, looks: Vec<(i64, LookMap)>) {
        for (id, look) in looks {
            self.change_persona(id, |p| {
                for (k, v) in look {
                    let k = crate::world::species::look_name(&k);
                    // Newer names stand for the old ones.
                    if k == "top" || k == "bottom" {
                        for old in if k == "top" { ["shirt", "shirt_hue", "top_hue"] } else { ["trousers", "trousers_hue", "bottom_hue"] } {
                            p.look.remove(old);
                        }
                    }
                    if v.is_finite() && !matches!(k.as_str(), "height" | "build" | "skin") && !k.starts_with("was_") {
                        p.look.insert(k, v);
                    }
                }
                p.look.entry("shape".into()).or_insert(0.5);
            });
        }
    }

    /// A being's look sliders by name, now (None: their body has no such slider).
    fn slider_now(&self, who: ActorId, name: &str) -> Option<f32> {
        match who {
            ActorId::Player => {
                let b = self.own_body_type()?.ct.meta.body.clone()?;
                b.slider(name).map(|i| self.own_sliders()[i])
            }
            ActorId::Npc(c) => {
                let n = self.cast.get(c)?;
                let b = self.snap.type_of(n.body_ty)?.ct.meta.body.clone()?;
                b.slider(name).map(|i| n.sliders[i])
            }
        }
    }

    /// Their own look as they keep it (what was taken off is in it).
    fn kept_look(&self, who: ActorId) -> LookMap {
        match who {
            ActorId::Player => self.own.look.clone(),
            ActorId::Npc(c) => self.cast.get(c).map(|n| n.def.persona.look.clone()).unwrap_or_default(),
        }
    }

    fn set_kept_look(&mut self, who: ActorId, set: LookMap, drop: &[String]) {
        match who {
            ActorId::Player => {
                for k in drop {
                    self.own.look.remove(k);
                }
                self.set_own_look(&set);
            }
            ActorId::Npc(c) => self.change_persona(c, |p| {
                for k in drop {
                    p.look.remove(k);
                }
                p.look.extend(set);
            }),
        }
    }

    /// The garments a being has on that `what` names (or the outermost one).
    pub fn garments_on(&self, who: ActorId, what: Option<&str>) -> Vec<&'static str> {
        let want = what.map(named);
        let worn: Vec<&'static str> = GARMENTS.iter().map(|g| g.0).filter(|s| self.slider_now(who, s).is_some_and(|v| on(s, v))).collect();
        match want {
            Some(w) => worn.into_iter().filter(|s| w.contains(s)).collect(),
            // Underwear only when asked for by name.
            None => worn.into_iter().filter(|s| *s != "underwear").take(1).collect(),
        }
    }

    /// Take off garments their body draws (see the module): `who` undresses
    /// `wearer`, who must be willing.
    pub fn undress(&mut self, who: ActorId, wearer: ActorId, what: Option<&str>) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let wname = self.actor_name(wearer);
        let off = self.garments_on(wearer, what);
        if off.is_empty() {
            return fail(match what {
                Some(w) => format!("{wname} isn't wearing {w}"),
                None => format!("{wname} isn't wearing anything to take off"),
            });
        }
        let look = |s: &str| self.slider_now(wearer, s).unwrap_or(0.0);
        let thing = call(off[0], &look);
        if wearer != who {
            let Some(p) = self.actor(wearer).map(|a| a.pos) else { return fail("they're gone") };
            let me = self.actor(who).map(|a| a.pos).unwrap_or(p);
            if (p - me).length() > self.reach_of(who) + 0.6 {
                return Err(ActErr::TooFar { at: p, dist: (p - me).length() });
            }
            if let ActorId::Npc(c) = wearer {
                if self.social.affection(wearer, who) - self.wariness(c) < 0.25 {
                    self.event("refused", Some(wearer), Some(who.key()), format!("{wname} wouldn't let {name} take off their {thing}"), Some(p), json!({}));
                    return fail(format!("{wname} won't let you"));
                }
            }
        }
        let mut set = LookMap::new();
        for s in &off {
            set.insert(format!("was_{s}"), look(s));
            set.insert(s.to_string(), 0.0);
        }
        self.set_kept_look(wearer, set, &[]);
        let msg = if wearer == who { format!("{name} takes off their {thing}") } else { format!("{name} takes {wname}'s {thing} off") };
        let p = self.actor(wearer).map(|a| a.pos);
        self.event("took_off", Some(who), Some(wearer.key()), msg.clone(), p, json!({ "from": wearer, "garments": off }));
        if let Some(p) = p {
            self.note_near(p, 20.0, super::Note::seen(format!("{}.", super::physics::cap(&msg)), wearer == ActorId::Player || who == ActorId::Player));
        }
        Ok(Outcome::ok(msg))
    }

    /// Put back on what `wearer` took off (what `what` names, or all of it).
    pub fn redress(&mut self, who: ActorId, wearer: ActorId, what: Option<&str>) -> Result<Outcome, ActErr> {
        let name = self.actor_name(who);
        let wname = self.actor_name(wearer);
        let kept = self.kept_look(wearer);
        let want = what.map(named);
        let back: Vec<(&str, f32)> = GARMENTS.iter().map(|g| g.0).filter(|s| want.as_ref().is_none_or(|w| w.contains(s))).filter_map(|s| kept.get(&format!("was_{s}")).map(|v| (s, *v))).collect();
        if back.is_empty() {
            return fail(format!("{wname} has nothing taken off to put back on"));
        }
        if wearer != who {
            if let ActorId::Npc(c) = wearer {
                if self.social.affection(wearer, who) - self.wariness(c) < 0.15 {
                    return fail(format!("{wname} won't let you"));
                }
            }
        }
        let mut set = LookMap::new();
        let mut drop = Vec::new();
        for (s, v) in &back {
            set.insert(s.to_string(), *v);
            drop.push(format!("was_{s}"));
        }
        let look = |s: &str| set.get(s).copied().unwrap_or(0.0);
        let thing = call(back[0].0, &look);
        self.set_kept_look(wearer, set, &drop);
        let msg = if wearer == who { format!("{name} puts their {thing} back on") } else { format!("{name} puts {wname}'s {thing} back on them") };
        let p = self.actor(wearer).map(|a| a.pos);
        self.event("wore", Some(who), Some(wearer.key()), msg.clone(), p, json!({ "on": wearer }));
        if let Some(p) = p {
            self.note_near(p, 20.0, super::Note::seen(format!("{}.", super::physics::cap(&msg)), wearer == ActorId::Player || who == ActorId::Player));
        }
        Ok(Outcome::ok(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::named;

    #[test]
    fn a_first_guess_from_words() {
        let p = crate::world::Persona { name: "Bortolo".into(), age: 61, appearance: "Broad and stooped, with a grey beard full of stone dust. He wears a leather apron and a battered felt cap.".into(), ..Default::default() };
        let g = super::guess(&p);
        assert_eq!(g.get("shape"), Some(&0.05));
        assert_eq!(g.get("hair_color"), Some(&1.0));
        assert_eq!(g.get("beard"), Some(&0.6));
        assert_eq!(g.get("hat"), Some(&0.4));
        let p = crate::world::Persona { name: "Nina".into(), age: 24, appearance: "Tall and sunburnt, with dark braided hair under a red kerchief. Her palms are calloused.".into(), ..Default::default() };
        let g = super::guess(&p);
        assert_eq!(g.get("shape"), Some(&0.95));
        assert_eq!(g.get("hair_style"), Some(&0.8));
        assert_eq!(g.get("hat"), Some(&0.2));
        assert_eq!(g.get("hair_color"), Some(&0.25), "dark, not the kerchief's red");
    }

    #[test]
    fn garments_by_name() {
        assert_eq!(named("his heavy boots"), vec!["shoes"]);
        assert_eq!(named("her red dress"), vec!["top_cover", "skirt"]);
        assert_eq!(named("the straw hat"), vec!["hat"]);
        assert!(named("a lantern").is_empty());
        assert_eq!(named("clothes"), vec!["top_cover", "skirt", "legs"]);
    }
}
