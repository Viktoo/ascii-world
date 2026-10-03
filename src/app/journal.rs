//! The log and the journal. The log under the view keeps what matters now:
//! talk, answers to you, and what changes the world stay; everyday life
//! nearby shows a short while and gives way. The journal (key 1) keeps it
//! all, with filters, and news from beyond earshot. A line said again soon
//! after is counted on the old line instead of added.

use super::{ACCENT, App, DIM, PANEL_BG, TEXT, speaker_color};
use crate::sim::Note;
use crate::term::Cell;
use glam::Vec3;
use std::time::{Duration, Instant};

/// Everyday life stays in the small log this long.
const LIFE_FOR: Duration = Duration::from_secs(20);
/// A line said again within this long, a few lines back, is counted, not added.
const SQUASH_WITHIN: Duration = Duration::from_secs(90);
const SQUASH_LOOKBACK: usize = 8;
const KEEP: usize = 1000;
const LIFE: [u8; 3] = [128, 133, 150];
const NOTABLE: [u8; 3] = [238, 226, 196];
const MARK: [u8; 3] = [255, 205, 120];
const FAR: [u8; 3] = [150, 162, 190];
const FAINT: [u8; 3] = [90, 94, 110];

/// What a line is, which decides where it shows and for how long.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    /// Someone speaking, you included.
    Talk,
    /// For you: answers, help, the game's own news.
    System,
    /// Everyday life nearby.
    Life,
    /// Something that changed the world, nearby.
    Notable,
    /// Something that changed the world, beyond earshot.
    Far,
}

#[derive(Clone)]
pub(super) struct LogLine {
    pub speaker: Option<String>,
    pub text: String,
    pub color: [u8; 3],
    /// A streaming reply from this character.
    pub streaming: Option<i64>,
    /// The character who said it, for the box over their head.
    pub who: Option<i64>,
    pub kind: Kind,
    /// When it was last said.
    pub at: Instant,
    /// How many times it was said.
    pub count: u32,
}

impl LogLine {
    pub fn new(kind: Kind, speaker: Option<String>, text: String, color: [u8; 3]) -> LogLine {
        LogLine { speaker, text, color, streaming: None, who: None, kind, at: Instant::now(), count: 1 }
    }
}

/// Which lines the journal shows.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum Filter {
    #[default]
    All,
    Talk,
    Notable,
    Life,
    Elsewhere,
}

impl Filter {
    const ALL: [Filter; 5] = [Filter::All, Filter::Talk, Filter::Notable, Filter::Life, Filter::Elsewhere];

    fn name(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Talk => "talk",
            Filter::Notable => "✦ notable",
            Filter::Life => "life",
            Filter::Elsewhere => "elsewhere",
        }
    }

    pub fn next(self) -> Filter {
        let i = Filter::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Filter::ALL[(i + 1) % Filter::ALL.len()]
    }

    fn keeps(self, k: Kind) -> bool {
        match self {
            Filter::All => true,
            Filter::Talk => k == Kind::Talk,
            Filter::Notable => matches!(k, Kind::Notable | Kind::Far),
            Filter::Life => k == Kind::Life,
            Filter::Elsewhere => k == Kind::Far,
        }
    }
}

/// What came in since the journal was last open, for the count on the log.
#[derive(Default)]
pub(super) struct Unseen {
    pub life: u32,
    pub far: u32,
}

type Styled = Vec<(char, [u8; 3], bool)>;

/// Word-wrap styled text: the first row `first` wide, the rest `rest` wide.
fn wrap_styled(s: &Styled, first: usize, rest: usize) -> Vec<Styled> {
    let mut rows: Vec<Styled> = vec![Vec::new()];
    let mut words: Vec<Styled> = Vec::new();
    let mut cur: Styled = Vec::new();
    let mut breaks = Vec::new();
    for &c in s {
        if c.0 == ' ' || c.0 == '\n' {
            words.push(std::mem::take(&mut cur));
            breaks.push(c.0 == '\n');
        } else {
            cur.push(c);
        }
    }
    words.push(cur);
    breaks.push(false);
    for (word, hard) in words.into_iter().zip(breaks) {
        let width = if rows.len() == 1 { first } else { rest }.max(4);
        let row = rows.last_mut().unwrap();
        if !row.is_empty() && row.len() + 1 + word.len() > width {
            rows.push(Vec::new());
        } else if !row.is_empty() {
            let sp = row.last().map(|c| (' ', c.1, false)).unwrap();
            row.push(sp);
        }
        for c in word {
            let width = if rows.len() == 1 { first } else { rest }.max(4);
            if rows.last().unwrap().len() >= width {
                rows.push(Vec::new());
            }
            rows.last_mut().unwrap().push(c);
        }
        if hard {
            rows.push(Vec::new());
        }
    }
    rows
}

impl App {
    /// A line for the log: the game's own (no speaker) or someone's words.
    pub(super) fn say(&mut self, speaker: Option<&str>, text: &str, color: [u8; 3]) {
        let kind = if speaker.is_some() { Kind::Talk } else { Kind::System };
        self.log_push(LogLine::new(kind, speaker.map(str::to_string), text.to_string(), color));
    }

    /// Add a line, or count it on the same line said a moment ago.
    pub(super) fn log_push(&mut self, line: LogLine) {
        match line.kind {
            Kind::Life => self.unseen.life += 1,
            Kind::Far => self.unseen.far += 1,
            _ => {}
        }
        let now = Instant::now();
        let same = self.log.iter().rev().take(SQUASH_LOOKBACK).position(|l| {
            l.streaming.is_none() && l.kind == line.kind && l.speaker == line.speaker && l.text == line.text && now.duration_since(l.at) < SQUASH_WITHIN
        });
        if let Some(back) = same {
            let i = self.log.len() - 1 - back;
            let mut old = self.log.remove(i).unwrap();
            old.count += 1;
            old.at = now;
            self.log.push_back(old);
        } else {
            self.log.push_back(line);
        }
        while self.log.len() > KEEP {
            self.log.pop_front();
        }
        self.dirty = true;
    }

    /// Something from the sim for the log.
    pub(super) fn tell(&mut self, n: Note) {
        let line = match n {
            Note::Line { id, who, text } => {
                let c = match id {
                    Some(crate::sim::ActorId::Npc(c)) => Some(c),
                    _ => None,
                };
                if let Some(c) = c {
                    self.conv.entry(c).or_default().push((false, text.clone()));
                }
                LogLine { who: c, ..LogLine::new(Kind::Talk, Some(who), text, TEXT) }
            }
            Note::Info(t) => LogLine::new(Kind::System, None, t, [200, 205, 220]),
            Note::Effect(t) => LogLine::new(Kind::System, None, format!("↳ {t}"), [150, 175, 200]),
            Note::Ambient(t) => LogLine::new(Kind::Life, None, t, LIFE),
            Note::Notable(t) => LogLine::new(Kind::Notable, None, t, NOTABLE),
            Note::Far(t, at) => LogLine::new(Kind::Far, None, format!("{t} ({})", self.far_place(Vec3::from(at))), FAR),
        };
        self.log_push(line);
    }

    /// Where far news happened: the place's name if it has one, how far, which way.
    fn far_place(&self, at: Vec3) -> String {
        let me = self.pos();
        let d = Vec3::new(at.x - me.x, 0.0, at.z - me.z);
        let dist = d.length();
        let far = if dist >= 1000.0 { format!("{:.1} km", dist / 1000.0) } else { format!("{} m", (dist / 10.0).round() as i32 * 10) };
        let way = format!("{far} {}", crate::sim::compass(d));
        match self.snap.region_name(crate::world::region_of(at.x, at.z)) {
            Some(n) if !n.is_empty() => format!("{n}, {way}"),
            _ => way,
        }
    }

    /// Is this line in the open view: the small log keeps what matters now,
    /// the journal what its filter asks for.
    fn in_view(&self, l: &LogLine, now: Instant) -> bool {
        if self.log_view > 0 {
            return self.journal_filter.keeps(l.kind);
        }
        match l.kind {
            Kind::Far => false,
            Kind::Life => now.duration_since(l.at) < LIFE_FOR,
            _ => true,
        }
    }

    /// The line as styled text: its mark, who speaks, the words, how often.
    fn styled(l: &LogLine) -> Styled {
        let mut s: Styled = Vec::new();
        let put = |s: &mut Styled, t: &str, c: [u8; 3], b: bool| s.extend(t.chars().map(|ch| (ch, c, b)));
        match l.kind {
            Kind::Notable => put(&mut s, "✦ ", MARK, true),
            Kind::Far => put(&mut s, "✧ ", FAR, false),
            _ => {}
        }
        if let Some(sp) = &l.speaker {
            let sc = if sp == "you" { ACCENT } else { speaker_color(sp) };
            put(&mut s, &format!("{sp}:"), sc, true);
            put(&mut s, " ", l.color, false);
        }
        put(&mut s, l.text.trim_end(), l.color, false);
        if l.count > 1 {
            put(&mut s, &format!(" ×{}", l.count), DIM, false);
        }
        s
    }

    /// The rows of the open view, oldest first, scrolled back `log_scroll`.
    fn log_view_rows(&mut self, width: usize, rows: usize) -> Vec<Styled> {
        let now = Instant::now();
        let mut out: Vec<Styled> = Vec::new();
        for l in self.log.iter().rev() {
            if !self.in_view(l, now) {
                continue;
            }
            // Wrapped lines hang under the first, so they read as the same line.
            let mut wrapped = wrap_styled(&Self::styled(l), width, width.saturating_sub(2));
            for r in wrapped.iter_mut().skip(1) {
                r.splice(0..0, [(' ', TEXT, false), (' ', TEXT, false)]);
            }
            out.extend(wrapped.into_iter().rev());
            if out.len() >= rows + self.log_scroll {
                break;
            }
        }
        self.log_scroll = self.log_scroll.min(out.len().saturating_sub(rows));
        let mut shown: Vec<Styled> = out.into_iter().skip(self.log_scroll).take(rows).collect();
        shown.reverse();
        shown
    }

    /// The log or journal, from the separator at `top` down `rows` lines.
    /// `x` is where the separator's mode tag ends.
    pub(super) fn draw_log(&mut self, w: u16, top: u16, rows: u16, x: u16) {
        let journal = self.log_view > 0;
        let mut x = x;
        if journal {
            x = self.screen.text(x + 1, top, " journal ", ACCENT, PANEL_BG, true);
            for f in Filter::ALL {
                let on = f == self.journal_filter;
                let label = if on { format!("[{}]", f.name()) } else { format!(" {} ", f.name()) };
                x = self.screen.text(x, top, &label, if on { TEXT } else { DIM }, PANEL_BG, on);
            }
            let help = " Tab filter · 1 size · Esc close ";
            let rx = w.saturating_sub(help.chars().count() as u16 + 1);
            if rx > x + 1 {
                self.screen.text(rx, top, help, FAINT, PANEL_BG, false);
            }
        } else {
            // What the small log leaves out, so you know there is more.
            let mut parts = Vec::new();
            if self.unseen.life > 0 {
                parts.push(format!("{} stirring nearby", self.unseen.life));
            }
            if self.unseen.far > 0 {
                parts.push(format!("{} elsewhere", self.unseen.far));
            }
            if !parts.is_empty() {
                let s = format!(" {} · 1 journal ", parts.join(" · "));
                let rx = w.saturating_sub(s.chars().count() as u16 + 1);
                if rx > x + 1 {
                    self.screen.text(rx, top, &s, FAINT, PANEL_BG, false);
                }
            }
        }
        let width = (w as usize).saturating_sub(2);
        let lines = self.log_view_rows(width, rows as usize);
        if self.log_scroll > 0 {
            self.screen.text(x + 1, top, &format!(" ↑ {} lines back · PgDn ", self.log_scroll), DIM, PANEL_BG, false);
        }
        for i in 0..rows {
            let y = top + 1 + i;
            self.screen.fill_row(y, Cell { bg: PANEL_BG, ..Cell::BLANK });
            if let Some(row) = lines.get(i as usize) {
                for (j, (ch, fg, bold)) in row.iter().enumerate() {
                    self.screen.set(1 + j as u16, y, Cell { ch: crate::term::narrow(*ch), fg: *fg, bg: PANEL_BG, bold: *bold });
                }
            }
        }
        // Everyday lines leave the small log on their own: redraw when the next one goes.
        self.log_expiry = if journal { None } else { self.log.iter().filter(|l| l.kind == Kind::Life && l.at.elapsed() < LIFE_FOR).map(|l| l.at + LIFE_FOR).min() };
    }

    /// Open the journal at a size (0 closes it); opening it counts all as seen.
    pub(super) fn set_log_view(&mut self, v: u8) {
        self.log_view = v;
        self.log_scroll = 0;
        if v > 0 {
            self.unseen = Unseen::default();
        }
        self.last_frame = None;
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(rows: &[Styled]) -> Vec<String> {
        rows.iter().map(|r| r.iter().map(|c| c.0).collect()).collect()
    }

    #[test]
    fn wraps_words_and_breaks_lines() {
        let s: Styled = "one two three\nfour".chars().map(|c| (c, TEXT, false)).collect();
        assert_eq!(plain(&wrap_styled(&s, 8, 6)), ["one two", "three", "four"]);
    }
}
