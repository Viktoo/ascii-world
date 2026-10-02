//! Random world prompts (`pocket new --random`): one reviewed part from each
//! list in `builtin/prompts.json`, put together into a sentence. Variety
//! comes from the lists, not from a model's habits; genesis then splits the
//! sentence into the land and where the traveller begins.

use serde::Deserialize;

const PARTS: &str = include_str!("builtin/prompts.json");
/// How often a world has its own peoples or beasts, and its own force:
/// about half the worlds are plain people in a real-feeling place.
const WHO_CHANCE: f32 = 0.5;
const FORCE_CHANCE: f32 = 0.25;

#[derive(Deserialize)]
struct Part {
    text: String,
    #[serde(default)]
    has: Vec<String>,
    #[serde(default)]
    needs: Vec<String>,
    #[serde(default)]
    avoid: Vec<String>,
}

#[derive(Deserialize)]
struct Parts {
    land: Vec<Part>,
    who: Vec<Part>,
    force: Vec<Part>,
    mood: Vec<Part>,
    start: Vec<Part>,
}

struct Rng(u32);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = crate::noise::pcg(self.0);
        self.0
    }

    fn chance(&mut self, p: f32) -> bool {
        (self.next() as f32 / u32::MAX as f32) < p
    }

    /// A part that fits what the world has so far.
    fn pick<'a>(&mut self, list: &'a [Part], has: &[String]) -> Option<&'a Part> {
        let fits: Vec<&Part> = list.iter().filter(|p| p.needs.iter().all(|n| has.contains(n)) && !p.avoid.iter().any(|a| has.contains(a))).collect();
        (!fits.is_empty()).then(|| fits[self.next() as usize % fits.len()])
    }
}

fn parts() -> Parts {
    serde_json::from_str(PARTS).expect("builtin prompts.json")
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn roll(parts: &Parts, seed: u32) -> String {
    let mut rng = Rng(seed);
    let mut has = Vec::new();
    let mut out = Vec::new();
    let mut take = |p: &Part, has: &mut Vec<String>, line: String| {
        has.extend(p.has.iter().cloned());
        out.push(line);
    };
    let land = rng.pick(&parts.land, &has).expect("a land");
    take(land, &mut has, format!("{}.", capital(&land.text)));
    if rng.chance(WHO_CHANCE) {
        if let Some(p) = rng.pick(&parts.who, &has) {
            take(p, &mut has, format!("Home to {}.", p.text));
        }
    }
    if rng.chance(FORCE_CHANCE) {
        if let Some(p) = rng.pick(&parts.force, &has) {
            take(p, &mut has, format!("Here, {}.", p.text));
        }
    }
    if let Some(p) = rng.pick(&parts.mood, &has) {
        take(p, &mut has, format!("{}.", capital(&p.text)));
    }
    if let Some(p) = rng.pick(&parts.start, &has) {
        take(p, &mut has, format!("You arrive where {}.", p.text));
    }
    out.join(" ")
}

fn seed() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    crate::noise::pcg(t.subsec_nanos() ^ t.as_secs() as u32)
}

/// A new random world prompt.
pub fn prompt() -> String {
    roll(&parts(), seed())
}

/// `n` different prompts, for reviewing the lists.
pub fn sample(n: usize) -> Vec<String> {
    let parts = parts();
    let mut s = seed();
    let mut out: Vec<String> = Vec::new();
    for _ in 0..n * 20 {
        if out.len() >= n {
            break;
        }
        s = crate::noise::pcg(s ^ 0x9e37_79b9);
        let p = roll(&parts, s);
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_load_and_every_tag_is_used() {
        let p = parts();
        let has: Vec<&String> = p.land.iter().chain(&p.who).flat_map(|x| &x.has).collect();
        for x in p.land.iter().chain(&p.who).chain(&p.force).chain(&p.mood).chain(&p.start) {
            assert!(!x.text.trim().is_empty());
            for t in x.needs.iter().chain(&x.avoid) {
                assert!(has.contains(&t), "'{}' names a tag nothing has: {t}", x.text);
            }
        }
    }

    #[test]
    fn rolls_respect_tags() {
        let p = parts();
        for s in 0..2000 {
            let r = roll(&p, s);
            assert!(r.contains("You arrive where "), "{r}");
            if r.contains("lighthouse keeper") || r.contains("fish stopped") {
                let land = p.land.iter().find(|l| r.starts_with(&capital(&l.text))).unwrap();
                assert!(land.has.iter().any(|t| t == "water"), "{r}");
            }
            if r.contains("deep snow") {
                assert!(!["Sahel", "Polynesian", "Bengal", "A desert"].iter().any(|h| r.starts_with(h)), "{r}");
            }
            if r.contains("two peoples meet") {
                assert!(["hill folk", "elves", "orcs"].iter().any(|w| r.contains(w)), "{r}");
            }
        }
    }

    #[test]
    fn samples_differ() {
        let s = sample(30);
        assert_eq!(s.len(), 30);
    }
}
