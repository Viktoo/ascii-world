//! A character's voice, read off who they are: no extra LLM call. Sex comes
//! from how they are described, age and height set the pitch and the size of
//! the throat, and the words for how they speak ("slow, with long pauses",
//! "gruff", "breathless rushes") move the sliders. Each also gets their own
//! blend of model voices and a small offset of their own, so no two match.

use crate::world::Persona;

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// Model voices and their weights.
    pub blend: Vec<(&'static str, f32)>,
    /// British pronunciation (the blend's voices are British).
    pub british: bool,
    pub female: bool,
    /// Mean pitch, Hz.
    pub f0: f32,
    /// Pitch movement (1 = as the model speaks).
    pub range: f32,
    /// Vocal tract scale against an adult man (1.0; a woman ~1.17, a child ~1.3).
    pub throat: f32,
    pub breath: f32,
    pub rasp: f32,
    pub tremor: f32,
    pub trem_hz: f32,
    pub jitter: f32,
    pub speed: f32,
    /// -1 dark .. +1 bright.
    pub tilt: f32,
    pub seed: u64,
}

const F_US: &[&str] = &["af_heart", "af_bella", "af_nicole", "af_sarah", "af_sky", "af_nova", "af_aoede", "af_kore", "af_jessica", "af_river"];
const M_US: &[&str] = &["am_michael", "am_fenrir", "am_puck", "am_echo", "am_eric", "am_liam", "am_onyx", "am_adam"];
const F_GB: &[&str] = &["bf_emma", "bf_isabella", "bf_alice", "bf_lily"];
const M_GB: &[&str] = &["bm_george", "bm_lewis", "bm_daniel", "bm_fable"];

fn hash(s: &str, id: i64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    h ^ (h >> 29)
}

/// 0..1 from bits of `h`.
fn u(h: u64, k: u32) -> f32 {
    ((h.rotate_right(k * 11) >> 40) % 10_000) as f32 / 10_000.0
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).map(String::from).collect()
}

fn any(ws: &[String], keys: &[&str]) -> bool {
    ws.iter().any(|w| keys.iter().any(|k| w.starts_with(k)))
}

const SHE: &[&str] = &["woman", "women", "girl", "she", "her", "hers", "herself", "lady", "mother", "grandmother", "granny", "grandma", "wife", "daughter", "sister", "aunt", "widow", "queen", "maid", "matron", "nun", "priestess", "witch", "mistress", "niece", "princess", "duchess", "empress", "madam", "mum", "mom"];
const HE: &[&str] = &["man", "men", "boy", "he", "him", "his", "himself", "lord", "father", "grandfather", "grandpa", "husband", "son", "brother", "uncle", "king", "beard", "bearded", "moustache", "mustache", "monk", "priest", "widower", "nephew", "prince", "duke", "emperor", "sir", "dad", "lad", "stubble", "whiskers", "sideburns"];

impl Recipe {
    /// The formant scale of the blend's model voices (what `throat` is against).
    pub fn base_throat(&self) -> f32 {
        if self.female { 1.17 } else { 1.0 }
    }

    /// How fast they speak, as a share of an ordinary pace.
    pub fn pace(p: &Persona) -> f32 {
        let ws = words(&format!("{} {}", p.voice, p.personality));
        let mut s = 1.0;
        if any(&ws, &["slow", "pause", "drawl", "measured", "deliberat", "unhurried", "ponderous", "halting"]) {
            s *= 0.88;
        }
        if any(&ws, &["quick", "fast", "rush", "breathless", "chatter", "rapid", "babbl", "hurried", "tumbl", "excit"]) {
            s *= 1.14;
        }
        if p.age >= 65 {
            s *= 0.92;
        } else if p.age > 0 && p.age < 13 {
            s *= 1.06;
        }
        s
    }

    pub fn of(p: &Persona, id: i64) -> Recipe {
        let h = hash(&p.name, id);
        let desc = words(&format!("{} {} {}", p.appearance, p.personality, p.voice));
        let how = words(&p.voice);
        // Fisherwoman, seamstress, fisherman, horseman count too.
        let is_she = |w: &str| SHE.contains(&w) || w.ends_with("woman") || w.ends_with("women") || w.ends_with("wife") || w.ends_with("ess") && w.len() > 5;
        let is_he = |w: &str| !is_she(w) && (HE.contains(&w) || (w.ends_with("man") || w.ends_with("men")) && w.len() > 4);
        let she = desc.iter().filter(|w| is_she(w)).count();
        let he = desc.iter().filter(|w| is_he(w)).count();
        let female = if she != he { she > he } else { h & 1 == 0 };
        let age = if p.age == 0 { 35 } else { p.age };
        let british = (h >> 1) & 1 == 0;
        let child = age < 13;

        // Two model voices of the right kind, in their own mix.
        let pool: &[&str] = match (female || child, british) {
            (true, false) => F_US,
            (true, true) => F_GB,
            (false, false) => M_US,
            (false, true) => M_GB,
        };
        let a = (h >> 8) as usize % pool.len();
        let b = (a + 1 + (h >> 16) as usize % (pool.len() - 1)) % pool.len();
        let wa = 0.5 + 0.3 * u(h, 1);
        let blend = vec![(pool[a], wa), (pool[b], 1.0 - wa)];

        let height = p.look.get("height").copied().unwrap_or(0.0);
        let build = p.look.get("build").copied().unwrap_or(1.0);
        let mut r = Recipe {
            blend,
            british,
            female: female || child,
            f0: if female { 200.0 } else { 115.0 },
            range: 1.0,
            throat: if female { 1.17 } else { 1.0 },
            breath: 0.1,
            rasp: 0.0,
            tremor: 0.0,
            trem_hz: 5.5,
            jitter: 0.1,
            speed: Self::pace(p),
            tilt: 0.0,
            seed: h,
        };
        // Age: children small and high; the old shakier, rougher, breathier.
        if child {
            r.f0 = if female { 285.0 } else { 260.0 };
            r.throat = 1.28;
            r.range = 1.3;
            r.tilt = 0.25;
        } else if age < 18 {
            r.f0 = if female { 225.0 } else { 140.0 };
            r.throat = if female { 1.2 } else { 1.06 };
        } else if age >= 55 {
            let old = ((age as f32 - 55.0) / 30.0).clamp(0.0, 1.0);
            r.f0 *= if female { 1.0 - 0.1 * old } else { 1.0 + 0.08 * old };
            r.tremor = 0.75 * old;
            r.rasp = 0.15 + 0.3 * old;
            r.jitter = 0.15 + 0.25 * old;
            r.breath += 0.25 * old;
            r.tilt -= 0.2 * old;
        }
        // Size: taller, a longer throat and a lower voice.
        if height > 0.5 && !child {
            let ref_h = if female { 1.65 } else { 1.78 };
            let k = (ref_h / height).clamp(0.8, 1.25);
            r.throat *= k.powf(0.6);
            r.f0 *= k.powf(0.5);
        }
        if build > 1.1 {
            r.f0 *= 0.96;
            r.tilt -= 0.1;
        }
        // How they speak.
        if any(&how, &["gruff", "rough", "hoarse", "gravel", "rasp", "growl", "harsh", "croak", "husky", "smok"]) {
            r.rasp += 0.35;
            r.tilt -= 0.15;
        }
        if any(&how, &["soft", "gentle", "quiet", "whisper", "murmur", "hush", "breathy", "low"]) {
            r.breath += 0.3;
            r.tilt -= 0.15;
        }
        if any(&how, &["breathless"]) {
            r.breath += 0.2;
        }
        if any(&how, &["deep", "boom", "rumbl", "bass", "resonant"]) {
            r.f0 *= 0.86;
            r.throat *= 0.95;
        }
        if any(&how, &["high", "squeak", "shrill", "piping", "reedy", "thin"]) {
            r.f0 *= 1.14;
            r.tilt += 0.1;
        }
        if any(&how, &["loud", "bellow", "shout", "bold", "brash"]) {
            r.tilt += 0.2;
            r.range *= 1.1;
        }
        if any(&how, &["flat", "dry", "monoton", "stern", "terse", "short", "clipped", "curt", "blunt"]) {
            r.range *= 0.7;
        }
        if any(&how, &["sing", "lilt", "melod", "hum", "cheer", "jok", "laugh", "bright", "merry", "warm", "lively"]) {
            r.range *= 1.25;
            r.tilt += 0.1;
        }
        if any(&how, &["nervous", "stammer", "stutter", "anxious", "shaky", "timid", "fret", "worri", "jitter"]) {
            r.jitter += 0.3;
            r.tremor = r.tremor.max(0.25);
            r.trem_hz = 7.5;
        }
        if any(&how, &["calm", "patient", "steady", "kind", "slow"]) {
            r.jitter *= 0.7;
        }
        // Their own: no two the same.
        r.f0 *= 0.93 + 0.14 * u(h, 2);
        r.throat *= 0.97 + 0.06 * u(h, 3);
        r.range *= 0.9 + 0.2 * u(h, 4);
        r.breath = (r.breath + 0.1 * u(h, 5)).clamp(0.0, 0.85);
        r.rasp = r.rasp.clamp(0.0, 0.8);
        r.tremor = r.tremor.clamp(0.0, 0.8);
        r.jitter = r.jitter.clamp(0.0, 0.7);
        r.tilt = r.tilt.clamp(-0.8, 0.6);
        r.speed = r.speed.clamp(0.75, 1.3);
        // Keep pitch and throat within what reshaping does cleanly.
        let base_f0 = if r.female { 190.0 } else { 112.0 };
        r.f0 = r.f0.clamp(base_f0 / 1.6, base_f0 * 1.6);
        r.throat = r.throat.clamp(r.base_throat() * 0.85, r.base_throat() * 1.18);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, age: u32, appearance: &str, voice: &str, height: f32) -> Persona {
        let mut p = Persona { name: name.into(), age, appearance: appearance.into(), voice: voice.into(), ..Default::default() };
        p.look.insert("height".into(), height);
        p
    }

    #[test]
    fn reads_who_they_are() {
        let nan = Recipe::of(&p("Aune", 72, "Broad, weathered woman with grey braids", "Short, flat sentences", 1.62), 1);
        let girl = Recipe::of(&p("Lyyti", 11, "Wiry girl with tangled hair", "breathless rushes", 1.42), 2);
        let big = Recipe::of(&p("Bram", 40, "Huge man with a black beard", "deep and gruff", 1.98), 3);
        assert!(nan.female && nan.tremor > 0.3 && nan.range < 0.9);
        assert!(girl.f0 > 250.0 && girl.throat > 1.22 && girl.speed > 1.1);
        assert!(!big.female && big.f0 < 105.0 && big.rasp > 0.3);
        assert!(big.blend.iter().all(|(v, _)| v.starts_with("am_") || v.starts_with("bm_")));
    }

    #[test]
    fn same_character_same_voice_others_differ() {
        let a = p("Veikko", 31, "Tall, rangy man", "Quick and joking", 1.86);
        assert_eq!(Recipe::of(&a, 7), Recipe::of(&a, 7));
        let mut b = a.clone();
        b.name = "Olli".into();
        assert_ne!(Recipe::of(&a, 7), Recipe::of(&b, 8));
    }
}
