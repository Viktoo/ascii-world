//! Random world prompts (`pocket new --random`): one plain, real-feeling place
//! from `builtin/places.txt`, one per line. Anything strange comes from play,
//! not from the prompt; genesis picks where in it the traveler begins.

const PLACES: &str = include_str!("builtin/places.txt");

fn places() -> Vec<&'static str> {
    PLACES.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
}

fn seed() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    crate::noise::pcg(t.subsec_nanos() ^ t.as_secs() as u32)
}

/// A new random world prompt.
pub fn prompt() -> String {
    let p = places();
    p[seed() as usize % p.len()].to_string()
}

/// Up to `n` different places, for reviewing the list.
pub fn sample(n: usize) -> Vec<String> {
    let mut p = places();
    let mut s = seed();
    // Shuffle, then take the first n.
    for i in (1..p.len()).rev() {
        s = crate::noise::pcg(s ^ 0x9e37_79b9);
        p.swap(i, s as usize % (i + 1));
    }
    p.into_iter().take(n).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_unique_sentences() {
        let p = places();
        assert!(p.len() > 50);
        for (i, l) in p.iter().enumerate() {
            assert!(l.ends_with('.'), "{l}");
            assert!(!p[..i].contains(l), "listed twice: {l}");
        }
    }

    #[test]
    fn samples_differ() {
        let s = sample(30);
        assert_eq!(s.len(), 30);
        assert!(s.iter().enumerate().all(|(i, x)| !s[..i].contains(x)));
    }
}
