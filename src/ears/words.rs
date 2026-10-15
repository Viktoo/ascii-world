//! What the recogniser heard, made into the words of the input line.

/// The recogniser's text, tidied: no sound notes (`[BLANK_AUDIO]`,
/// `(wind blowing)`, `*laughs*`), no stray marks, single spaces. Its
/// well-known line for silence ("Thanks for watching!") is nothing.
pub fn clean(raw: &str) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    let mut star = false;
    for c in raw.chars() {
        match c {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = (depth - 1).max(0),
            '*' => star = !star,
            '♪' | '>' | '<' | '_' => {}
            c if depth == 0 && !star => out.push(c),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let bare: String = out.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect::<String>().to_lowercase();
    let bare = bare.trim();
    if bare.is_empty() || ["thanks for watching", "thank you for watching", "please subscribe", "subtitles by"].iter().any(|h| bare.starts_with(h)) {
        return String::new();
    }
    // A lone "." or "-" is no words either.
    if !out.chars().any(|c| c.is_alphanumeric()) {
        return String::new();
    }
    out
}

/// The recogniser sometimes gets stuck saying the same thing over (mostly
/// on a quick pass): a run said again, straight after itself, is kept once.
pub fn unrepeat(text: &str) -> String {
    unrepeat_runs(text, 3)
}

/// Live words, from a quick pass: stuck even on one word ("Hello. Hello.
/// Hello."). Said three times over, a run of one or two words is kept once.
pub fn unrepeat_live(text: &str) -> String {
    let w: Vec<&str> = text.split_whitespace().collect();
    let key = |s: &str| s.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    let same = |a: usize, b: usize, n: usize| b + n <= w.len() && (0..n).all(|k| key(w[a + k]) == key(w[b + k]));
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < w.len() {
        let mut step = 1;
        for n in 1..=2 {
            let mut reps = 1;
            while same(i, i + reps * n, n) {
                reps += 1;
            }
            if reps >= 3 {
                out.extend(&w[i..i + n]);
                step = reps * n;
                break;
            }
        }
        if step == 1 {
            out.push(w[i]);
        }
        i += step;
    }
    unrepeat(&out.join(" "))
}

fn unrepeat_runs(text: &str, shortest: usize) -> String {
    let mut w: Vec<&str> = text.split_whitespace().collect();
    let key = |s: &str| s.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    let mut i = 0;
    while i < w.len() {
        let mut cut = false;
        // Runs of 3 words or more (shorter ones are real: "no, no, no").
        for n in (shortest..=(w.len() - i) / 2).rev() {
            if (0..n).all(|k| key(w[i + k]) == key(w[i + n + k])) {
                w.drain(i + n..i + 2 * n);
                cut = true;
                break;
            }
        }
        if !cut {
            i += 1;
        }
    }
    w.join(" ")
}

/// The input line with what was heard added after what was already there
/// (`/` for a deed, or words typed before).
pub fn merge(base: &str, heard: &str) -> String {
    let heard = heard.trim();
    if heard.is_empty() {
        return base.to_string();
    }
    if base.is_empty() || base.ends_with(char::is_whitespace) || base == "/" {
        return format!("{base}{heard}");
    }
    format!("{base} {heard}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_notes_and_silence_lines_go() {
        assert_eq!(clean(" Hello there, Mara."), "Hello there, Mara.");
        assert_eq!(clean("[BLANK_AUDIO]"), "");
        assert_eq!(clean(" (wind blowing) Is the  ice thin? *coughs*"), "Is the ice thin?");
        assert_eq!(clean(" Thanks for watching!"), "");
        assert_eq!(clean("♪ ♪"), "");
        assert_eq!(clean(" ..."), "");
        assert_eq!(clean(">> Where is the mill?"), "Where is the mill?");
        assert_eq!(clean(" Thank you."), "Thank you.", "said to someone, it's real");
    }

    #[test]
    fn a_stuck_recogniser_is_heard_once() {
        let stuck = "Have you seen the fox? Have you seen the fox? Have you seen the fox?";
        assert_eq!(unrepeat(stuck), "Have you seen the fox?");
        assert_eq!(unrepeat("Hello, Mara. Have you seen it? Hello, Mara. Have you seen it?"), "Hello, Mara. Have you seen it?");
        assert_eq!(unrepeat("No, no, no. Go away."), "No, no, no. Go away.");
        assert_eq!(unrepeat("the fox and the fox and the hen"), "the fox and the hen");
        assert_eq!(unrepeat(""), "");
        assert_eq!(unrepeat_live("Hello. Hello. Hello. Hello. Hello"), "Hello.");
        assert_eq!(unrepeat_live("Hello Mara Hello Mara Hello Mara"), "Hello Mara");
        assert_eq!(unrepeat_live("Hello Mara, have you seen it"), "Hello Mara, have you seen it");
    }

    #[test]
    fn heard_words_join_what_was_typed() {
        assert_eq!(merge("/", " pick up the stick"), "/pick up the stick");
        assert_eq!(merge("", "Hello."), "Hello.");
        assert_eq!(merge("Hello", "how are you?"), "Hello how are you?");
        assert_eq!(merge("Hello ", "how are you?"), "Hello how are you?");
        assert_eq!(merge("/", ""), "/");
    }
}
