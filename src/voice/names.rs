//! Words no dictionary knows (most of all the world's own names: Ganzorig,
//! Lyyti, Harrowcove) read the way an English speaker would sound them out,
//! instead of spelled letter by letter.

use misaki_rs::fallback::{Fallback, FallbackError};

pub struct SoundOut;

const VOWELS: &str = "aeiouyäöüåæøéèáàíóú";

fn is_vowel(c: char) -> bool {
    VOWELS.contains(c)
}

impl Fallback for SoundOut {
    fn phonemize(&self, word: &str) -> Result<String, FallbackError> {
        let ph = sound_out(word);
        if ph.is_empty() { Err(FallbackError::NoPhonemes { word: word.to_string() }) } else { Ok(ph) }
    }
}

/// Letters to Kokoro's phonemes, stress on the first vowel.
pub fn sound_out(word: &str) -> String {
    let w: Vec<char> = word.to_lowercase().chars().filter(|c| c.is_alphabetic()).collect();
    let n = w.len();
    let mut out = String::new();
    let mut i = 0;
    let mut stressed = false;
    let at = |k: usize| w.get(k).copied().unwrap_or(' ');
    while i < n {
        let c = w[i];
        let next = at(i + 1);
        let two: String = [c, next].iter().collect();
        // Digraphs first.
        let (ph, step): (&str, usize) = match two.as_str() {
            "sh" => ("ʃ", 2),
            "ch" => ("ʧ", 2),
            "th" => ("θ", 2),
            "ph" => ("f", 2),
            "ck" => ("k", 2),
            "ng" => ("ŋ", 2),
            "qu" => ("kw", 2),
            "wh" => ("w", 2),
            "kh" => ("k", 2),
            "zh" => ("ʒ", 2),
            "gh" => ("ɡ", 2),
            "dj" => ("ʤ", 2),
            "tz" | "ts" => ("ts", 2),
            "aa" => ("ɑː", 2),
            "ee" | "ea" | "ie" => ("i", 2),
            "oo" => ("u", 2),
            "ou" => ("W", 2),
            "ai" | "ay" | "ei" | "ey" => ("A", 2),
            "oi" | "oy" => ("Y", 2),
            "au" | "aw" => ("ɔ", 2),
            "uu" => ("uː", 2),
            "yy" => ("yː", 2),
            "ii" => ("iː", 2),
            _ => ("", 0),
        };
        let (ph, step) = if step > 0 {
            (ph.to_string(), step)
        } else {
            // Doubled consonants sound once.
            if next == c && !is_vowel(c) {
                i += 1;
            }
            let last = i + 1 == n;
            let p = match c {
                'a' | 'á' | 'à' => "ɑ",
                'e' | 'é' | 'è' => {
                    if last && n > 3 && !is_vowel(at(i.wrapping_sub(1))) {
                        ""
                    } else {
                        "ɛ"
                    }
                }
                'i' | 'í' => {
                    if last { "i" } else { "ɪ" }
                }
                'o' | 'ó' => {
                    if last { "O" } else { "ɔ" }
                }
                'u' | 'ú' => "u",
                'y' => {
                    if i == 0 && is_vowel(next) {
                        "j"
                    } else {
                        "i"
                    }
                }
                'ä' | 'æ' => "æ",
                'ö' | 'ø' => "ɜ",
                'ü' => "u",
                'å' => "O",
                'c' => {
                    if "eiy".contains(next) {
                        "s"
                    } else {
                        "k"
                    }
                }
                'g' => "ɡ",
                'j' => "ʤ",
                'q' => "k",
                'r' => "ɹ",
                'x' => "ks",
                'b' | 'd' | 'f' | 'h' | 'k' | 'l' | 'm' | 'n' | 'p' | 's' | 't' | 'v' | 'w' | 'z' => {
                    match c {
                        'b' => "b",
                        'd' => "d",
                        'f' => "f",
                        'h' => "h",
                        'k' => "k",
                        'l' => "l",
                        'm' => "m",
                        'n' => "n",
                        'p' => "p",
                        's' => "s",
                        't' => "t",
                        'v' => "v",
                        'w' => "w",
                        _ => "z",
                    }
                }
                _ => "",
            };
            (p.to_string(), 1)
        };
        let vowel_ph = ph.chars().next().is_some_and(|f| "ɑɛɪiOɔuæɜAWYyɑː".contains(f));
        if vowel_ph && !stressed {
            out.push('ˈ');
            stressed = true;
        }
        out.push_str(&ph);
        i += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_sound_like_words() {
        assert_eq!(sound_out("Ganzorig"), "ɡˈɑnzɔɹɪɡ");
        assert_eq!(sound_out("Lyyti"), "lˈyːti");
        assert!(sound_out("Harrowcove").starts_with("hˈɑɹ"));
        assert!(!sound_out("Kettunen").contains("tt"));
    }
}
