//! How far along world-making is, for the loading screen: a streamed reply's
//! length against what such replies usually run to, and the names it brings
//! (biomes, species, things, people) as they arrive.

use crate::brain::Event;
use crossbeam_channel::Sender;
use std::collections::HashSet;

/// A piece of world-making the loading screen waits on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Task {
    Genesis,
    Region((i32, i32)),
}

/// Typical reply lengths in characters (measured from past universes).
pub const GENESIS_CHARS: usize = 30_000;
pub const REGION_CHARS: usize = 12_500;

/// Progress through a stream: linear, then easing towards 1 if the reply
/// runs longer than expected, so the bar never stops dead or overshoots.
pub fn ease(t: f32) -> f32 {
    const K: f32 = 0.85;
    if t < K { t.max(0.0) } else { K + (1.0 - K) * (1.0 - (-(t - K) / (1.0 - K)).exp()) }
}

pub fn step(events: &Sender<Event>, task: Task, frac: f32) {
    let _ = events.send(Event::Progress { task, frac });
}

/// Feeds a streamed reply into progress (from `lo` to `hi`) and names.
pub struct Pacer {
    events: Sender<Event>,
    task: Task,
    lo: f32,
    hi: f32,
    expect: usize,
    chars: usize,
    sent: f32,
    scan: Scanner,
}

impl Pacer {
    pub fn new(events: Sender<Event>, task: Task, expect: usize, lo: f32, hi: f32) -> Pacer {
        Pacer { events, task, lo, hi, expect: expect.max(1), chars: 0, sent: lo, scan: Scanner::default() }
    }

    pub fn text(&mut self, t: &str) {
        self.chars += t.chars().count();
        let made = self.scan.feed(t);
        self.send(made);
    }

    pub fn finish(&mut self) {
        let made = self.scan.flush();
        self.send(made);
        step(&self.events, self.task, self.hi);
    }

    fn send(&mut self, made: Vec<(&'static str, String)>) {
        for (verb, name) in made {
            let _ = self.events.send(Event::Made { task: self.task, verb, name });
        }
        let f = self.lo + (self.hi - self.lo) * ease(self.chars as f32 / self.expect as f32);
        if f - self.sent >= 0.004 {
            self.sent = f;
            step(&self.events, self.task, f);
        }
    }
}

/// JSON objects whose names are worth showing, and nested ones that set the verb.
const SECTIONS: &[&str] = &["biomes", "properties", "rules", "species", "new_species", "varieties", "new_types", "landmarks", "settlement", "buildings", "things", "characters", "creatures", "attitudes", "facts"];

/// Picks names out of a streamed reply, line by line: what each JSON section
/// names, and each code block's `name:`.
#[derive(Default)]
pub struct Scanner {
    line: String,
    in_code: bool,
    js: bool,
    js_named: bool,
    section: Option<&'static str>,
    seen: HashSet<(&'static str, String)>,
}

impl Scanner {
    pub fn feed(&mut self, t: &str) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        self.line.push_str(t);
        while let Some(i) = self.line.find('\n') {
            let l: String = self.line.drain(..=i).collect();
            self.scan_line(&l, &mut out);
        }
        out
    }

    pub fn flush(&mut self) -> Vec<(&'static str, String)> {
        let mut out = Vec::new();
        let l = std::mem::take(&mut self.line);
        self.scan_line(&l, &mut out);
        out
    }

    fn emit(&mut self, verb: &'static str, name: &str, out: &mut Vec<(&'static str, String)>) {
        let name = name.trim();
        if name.is_empty() || name.contains('…') || name.chars().count() > 48 {
            return;
        }
        if self.seen.insert((verb, name.to_lowercase())) {
            out.push((verb, name.to_string()));
        }
    }

    fn scan_line(&mut self, l: &str, out: &mut Vec<(&'static str, String)>) {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("```") {
            if self.in_code {
                self.in_code = false;
                self.js = false;
            } else {
                self.in_code = true;
                self.js = matches!(rest.split_whitespace().next(), Some("js" | "javascript"));
                self.js_named = false;
                if !self.js {
                    self.section = None;
                }
            }
            return;
        }
        if self.js {
            if !self.js_named {
                if let Some(n) = js_name(t) {
                    self.js_named = true;
                    self.emit("shaping", &n, out);
                }
            }
            return;
        }
        // JSON keys in order along the line (objects may be on one line).
        let mut rest = t;
        while let Some(q) = rest.find('"') {
            let after = &rest[q + 1..];
            let Some(e) = after.find('"') else { break };
            let key = &after[..e];
            let tail = after[e + 1..].trim_start();
            let Some(tail) = tail.strip_prefix(':') else {
                rest = &after[e + 1..];
                continue;
            };
            let tail = tail.trim_start();
            if let Some(v) = tail.strip_prefix('"') {
                let Some(end) = v.find('"') else { break };
                let value = &v[..end];
                let verb = match (key, self.section) {
                    ("name", None) => Some("naming"),
                    ("name", Some("biomes")) => Some("spreading"),
                    ("name", Some("properties")) => Some("defining"),
                    ("name", Some("rules")) => Some("binding"),
                    ("name", Some("species" | "new_species" | "varieties")) => Some("imagining"),
                    ("name", Some("new_types")) => Some("designing"),
                    ("name", Some("settlement")) => Some("founding"),
                    ("name", Some("characters")) => Some("waking"),
                    ("species", Some("creatures")) => Some("herding"),
                    ("type", Some("landmarks")) => Some("placing"),
                    ("type", Some("buildings")) => Some("building"),
                    ("type", Some("things")) => Some("leaving out"),
                    _ => None,
                };
                if let Some(verb) = verb {
                    self.emit(verb, value, out);
                }
                rest = &v[end + 1..];
            } else {
                if tail.starts_with(['[', '{']) {
                    if let Some(s) = SECTIONS.iter().find(|s| **s == key) {
                        self.section = Some(s);
                    }
                }
                rest = tail;
            }
        }
    }
}

/// `name: "lighthouse"` in a type's meta.
fn js_name(t: &str) -> Option<String> {
    let i = t.find("name")?;
    let rest = t[i + 4..].trim_start().strip_prefix(':')?.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let body = &rest[1..];
    Some(body[..body.find(quote)?].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(text: &str, chunk: usize) -> Vec<(&'static str, String)> {
        let mut s = Scanner::default();
        let mut out = Vec::new();
        let bytes = text.as_bytes();
        for c in bytes.chunks(chunk) {
            out.extend(s.feed(&String::from_utf8_lossy(c)));
        }
        out.extend(s.flush());
        out
    }

    #[test]
    fn names_from_a_genesis_reply_in_any_chunks() {
        let reply = r#"```json
{
  "name": "the Salt Reach",
  "palette": { "sky_day": [1,2,3], "fog": 1.2 },
  "biomes": [
    { "name": "tidal flats", "base": 2, "scatter": { "reed": 1.0 } },
    { "name": "cliffs", "base": 20 }
  ],
  "species": [ { "name": "selkie", "plural": "selkies", "temper": { "bold": 0.4 } } ],
  "attitudes": [ { "a": "selkie", "b": "human", "affection": 0.2 } ]
}
```
```js
export const meta = {
  name: "sea pine",
  bounds: [2, 6, 2],
};
const name = "not this";
```
"#;
        for chunk in [1, 7, 64, 10_000] {
            let got = all(reply, chunk);
            let want = vec![("naming", "the Salt Reach"), ("spreading", "tidal flats"), ("spreading", "cliffs"), ("imagining", "selkie"), ("shaping", "sea pine")];
            assert_eq!(got.iter().map(|(v, n)| (*v, n.as_str())).collect::<Vec<_>>(), want, "chunk {chunk}");
        }
    }

    #[test]
    fn names_from_a_region_plan() {
        let reply = r#"```json
{"name": "Gull Point", "mood": "windy", "facts": ["the light went out"],
 "new_types": [{"name": "net shed", "tags": ["building"]}],
 "landmarks": [{"type": "lighthouse", "x": 10}],
 "settlement": {"name": "Ebb", "x": 1, "buildings": [{"type": "net shed", "dx": 2}]},
 "characters": [{"name": "Mara", "look": {"height": 1.7}, "species": "human"}],
 "creatures": [{"species": "goat", "count": 3}, {"species": "goat", "count": 2}]}
```"#;
        let got = all(reply, 5);
        let want = vec![
            ("naming", "Gull Point"),
            ("designing", "net shed"),
            ("placing", "lighthouse"),
            ("founding", "Ebb"),
            ("building", "net shed"),
            ("waking", "Mara"),
            ("herding", "goat"),
        ];
        assert_eq!(got.iter().map(|(v, n)| (*v, n.as_str())).collect::<Vec<_>>(), want);
    }

    #[test]
    fn progress_eases_without_overshooting() {
        assert_eq!(ease(0.5), 0.5);
        assert!((ease(0.85) - 0.85).abs() < 1e-6);
        assert!(ease(1.0) > 0.85 && ease(1.0) < 1.0);
        assert!(ease(5.0) <= 1.0 && ease(2.0) > ease(1.0));
    }
}
