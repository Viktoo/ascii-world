//! Between what a character says and what the voice model reads:
//! - the text: stage directions dropped, and the noises people make
//!   ("Hm.", "Mm-hm", "Shh!", "Brr") given sounds of their own instead of
//!   being spelled out letter by letter;
//! - the phonemes: misaki-rs writes IPA (e‍ɪ, o‍ʊ, t‍ʃ, length marks), and
//!   Kokoro was trained on misaki's own shorthand (A, O, ʧ, no length marks
//!   in American English), so they are rewritten the way it knows.

/// Noises, by their letters with repeats run together ("hmmm" is "hm").
const NOISES: &[(&str, &str)] = &[
    ("hm", "hmm"),
    ("m", "mm"),
    ("mhm", "mhˈm"),
    ("mhmh", "mhˈm"),
    ("hmph", "hmf"),
    ("hmf", "hmf"),
    ("sh", "ʃʃ"),
    ("br", "bɹɹ"),
    ("pf", "pf"),
    ("pft", "pft"),
    ("ps", "pst"),
    ("pst", "pst"),
    ("ts", "ts"),
    ("tsk", "tsk"),
    ("tut", "tʌt"),
    ("grr", "ɡɹɹ"),
    ("gr", "ɡɹɹ"),
    ("zz", "zzz"),
    ("z", "zzz"),
];

fn run_together(w: &str) -> String {
    let mut out = String::new();
    for c in w.chars().flat_map(|c| c.to_lowercase()) {
        if !out.ends_with(c) {
            out.push(c);
        }
    }
    out
}

fn noise(word: &str) -> Option<&'static str> {
    let key = run_together(word);
    NOISES.iter().find(|(k, _)| *k == key).map(|(_, p)| *p)
}

/// The text as it should be read aloud.
pub fn text(s: &str) -> String {
    // *sighs*, *looks away*: not words.
    let mut t = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '*' => inside = !inside,
            _ if !inside => t.push(c),
            _ => {}
        }
    }
    let t = t.replace(" — ", ", ").replace(" – ", ", ").replace(['—', '–'], ", ").replace("Mm-hm", "Mhm").replace("mm-hm", "mhm").replace("Uh-huh", "Mhm").replace("uh-huh", "mhm");
    // Noises get their sound written in, in misaki's [word](/phonemes/) form.
    let mut out = String::with_capacity(t.len());
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if word.is_empty() {
            return;
        }
        match noise(word) {
            Some(p) => out.push_str(&format!("[{word}](/{p}/)")),
            None => out.push_str(word),
        }
        word.clear();
    };
    for c in t.chars() {
        if c.is_alphabetic() || c == '\'' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// misaki-rs phonemes in Kokoro's own shorthand.
pub fn phonemes(ph: &str, british: bool) -> String {
    let zwj = '\u{200d}';
    let mut s = ph.to_string();
    let both: &[(&str, &str)] = &[("t\u{200d}ʃ", "ʧ"), ("d\u{200d}ʒ", "ʤ"), ("e\u{200d}ɪ", "A"), ("a\u{200d}ɪ", "I"), ("a\u{200d}ʊ", "W"), ("ɔ\u{200d}ɪ", "Y")];
    for (a, b) in both {
        s = s.replace(a, b);
    }
    if british {
        for (a, b) in [("ə\u{200d}ʊ", "Q"), ("o\u{200d}ʊ", "Q"), ("n\u{329}", "ᵊn"), ("l\u{329}", "ᵊl")] {
            s = s.replace(a, b);
        }
    } else {
        for (a, b) in [("o\u{200d}ʊ", "O"), ("ə\u{200d}ʊ", "O"), ("I\u{200d}ɚ", "Iəɹ"), ("ɚ", "əɹ"), ("ʔn\u{329}", "tn"), ("ʔ", "t"), ("ɾ", "T"), ("n\u{329}", "ᵊn"), ("l\u{329}", "ᵊl"), ("ɜːɹ", "ɜɹ"), ("ɜː", "ɜɹ"), ("ː", "")] {
            s = s.replace(a, b);
        }
    }
    s.retain(|c| c != zwj && c != '\u{329}');
    // No space before punctuation, single spaces.
    let mut out = String::with_capacity(s.len());
    for w in s.split_whitespace() {
        let punct = w.chars().all(|c| ".,!?;:…".contains(c));
        if !out.is_empty() && !punct {
            out.push(' ');
        }
        out.push_str(w);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noises_get_their_own_sound() {
        assert_eq!(text("Hm. Lake folk, always in a hurry."), "[Hm](/hmm/). Lake folk, always in a hurry.");
        assert_eq!(text("Hmmm... Shhh! Mm-hm."), "[Hmmm](/hmm/)... [Shhh](/ʃʃ/)! [Mhm](/mhˈm/).");
        assert_eq!(text("*sighs* Fine — go."), "Fine, go.");
        // Real words stay words.
        assert_eq!(text("Hmm, the mill."), "[Hmm](/hmm/), the mill.");
        assert_eq!(text("Ugh, my back."), "Ugh, my back.");
    }

    #[test]
    fn phonemes_in_kokoros_shorthand() {
        assert_eq!(phonemes("lˈe\u{200d}ɪk fˈo\u{200d}ʊk , ˈɔːlwe\u{200d}ɪz ɪn ɐ hˈɜːɹi . ", false), "lˈAk fˈOk, ˈɔlwAz ɪn ɐ hˈɜɹi.");
        assert_eq!(phonemes("lˈe\u{200d}ɪk fˈə\u{200d}ʊk , ˈɔːlwe\u{200d}ɪz", true), "lˈAk fˈQk, ˈɔːlwAz");
        assert_eq!(phonemes("t\u{200d}ʃˈa\u{200d}ɪld d\u{200d}ʒˈɔ\u{200d}ɪ", false), "ʧˈIld ʤˈY");
        assert_eq!(phonemes("t\u{200d}ʃˈɜːt\u{200d}ʃ bˈɜːd hˈɜːɹi", false), "ʧˈɜɹʧ bˈɜɹd hˈɜɹi");
    }
}
