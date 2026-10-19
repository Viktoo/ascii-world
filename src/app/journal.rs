//! The log and the journal. The log under the view keeps what matters now:
//! talk, answers to you, and what changes the world stay; everyday life
//! nearby shows a short while and gives way. The journal (key 1) keeps it
//! all, with filters, and news from beyond earshot. A line said again soon
//! after is counted on the old line instead of added.

use super::{ACCENT, App, DIM, TEXT, hud, speaker_color};
use crate::sim::Note;
use crate::term::{Cell, Deco, Face};
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
/// Lines in the feed at most.
const FEED_ROWS: usize = 6;
/// Rows one line may take in the feed.
const FEED_LINE_ROWS: usize = 2;
/// Marks a wrapped row (hung under the first) in the feed.
const HANG: char = '\u{1}';

/// How long a line stays in the feed: long enough to read it.
fn feed_for(l: &LogLine) -> Duration {
    let n = l.text.chars().count() as u64;
    Duration::from_millis((12_000 + n * 60).min(30_000))
}

/// Styled text as runs of one colour and weight.
fn runs(row: &Styled) -> Vec<(String, [u8; 3], bool)> {
    let mut out: Vec<(String, [u8; 3], bool)> = Vec::new();
    for &(ch, c, b) in row.iter().filter(|c| c.0 != HANG) {
        match out.last_mut() {
            Some(r) if (r.1 == c && r.2 == b) || ch == ' ' => r.0.push(ch),
            _ => out.push((ch.to_string(), c, b)),
        }
    }
    out
}

/// Word-wrap styled text by its set width in cells (the first row `first`
/// wide, the rest `rest`); rows after the first start with `HANG`.
fn wrap_measured(s: &Styled, first: f32, rest: f32, space: f32) -> Vec<Styled> {
    let cw = |c: &(char, [u8; 3], bool)| hud::width(&c.0.to_string(), if c.2 { Face::Bold } else { Face::Body }, hud::SIZE);
    let mut rows: Vec<Styled> = vec![Vec::new()];
    let mut used = 0.0;
    for (wi, word) in s.split(|c| c.0 == ' ' || c.0 == '\n').enumerate() {
        let ww: f32 = word.iter().map(cw).sum();
        let room = if rows.len() == 1 { first } else { rest };
        let row = rows.last_mut().unwrap();
        let hard = wi > 0 && hard_break_before(s, wi);
        if hard || (used > 0.0 && used + space + ww > room) {
            rows.push(vec![(HANG, TEXT, false)]);
            used = 0.0;
        } else if wi > 0 && used > 0.0 {
            let sp = row.last().map(|c| (' ', c.1, false)).unwrap_or((' ', TEXT, false));
            row.push(sp);
            used += space;
        }
        rows.last_mut().unwrap().extend_from_slice(word);
        used += ww;
    }
    rows
}

/// Whether the `wi`th word of `s` follows a line break.
fn hard_break_before(s: &Styled, wi: usize) -> bool {
    s.iter().filter(|c| c.0 == ' ' || c.0 == '\n').nth(wi - 1).is_some_and(|c| c.0 == '\n')
}

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
    /// Something made or remade nearby (far ones are `Far` with `made`).
    Made,
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
    /// A making (listed under "made", near or far).
    pub made: bool,
    /// The incident this line tells (updated in place).
    pub incident: Option<u32>,
}

impl LogLine {
    pub fn new(kind: Kind, speaker: Option<String>, text: String, color: [u8; 3]) -> LogLine {
        LogLine { speaker, text, color, streaming: None, who: None, kind, at: Instant::now(), count: 1, made: kind == Kind::Made, incident: None }
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
    Made,
    Elsewhere,
}

impl Filter {
    const ALL: [Filter; 6] = [Filter::All, Filter::Talk, Filter::Notable, Filter::Life, Filter::Made, Filter::Elsewhere];

    fn name(self) -> &'static str {
        match self {
            Filter::All => "all",
            Filter::Talk => "talk",
            Filter::Notable => "✦ notable",
            Filter::Life => "life",
            Filter::Made => "made",
            Filter::Elsewhere => "elsewhere",
        }
    }

    pub fn next(self) -> Filter {
        let i = Filter::ALL.iter().position(|f| *f == self).unwrap_or(0);
        Filter::ALL[(i + 1) % Filter::ALL.len()]
    }

    fn keeps(self, l: &LogLine) -> bool {
        match self {
            Filter::All => true,
            Filter::Talk => l.kind == Kind::Talk,
            Filter::Notable => matches!(l.kind, Kind::Notable | Kind::Far) && !l.made,
            Filter::Life => l.kind == Kind::Life,
            Filter::Made => l.made,
            Filter::Elsewhere => l.kind == Kind::Far,
        }
    }
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
                    self.voice_line(c, &text, false);
                }
                LogLine { who: c, ..LogLine::new(Kind::Talk, Some(who), text, TEXT) }
            }
            Note::Info(t) => LogLine::new(Kind::System, None, t, [200, 205, 220]),
            Note::Effect(t) => LogLine::new(Kind::System, None, format!("↳ {t}"), [150, 175, 200]),
            Note::Ambient(t) => LogLine::new(Kind::Life, None, t, LIFE),
            Note::Notable(t) => LogLine::new(Kind::Notable, None, t, NOTABLE),
            Note::Made(t) => LogLine::new(Kind::Made, None, t, NOTABLE),
            Note::Far { text, at, made } => LogLine { made, ..LogLine::new(Kind::Far, None, format!("{text} ({})", self.far_place(Vec3::from(at))), FAR) },
            Note::Incident { id, text, at, near } => return self.tell_incident(id, text, Vec3::from(at), near),
        };
        self.log_push(line);
    }

    /// An incident has one line: the first telling adds it, the rest
    /// rewrite it where it is. Once it came near, it stays marked as near.
    /// An empty text: it joined another, and its line goes.
    fn tell_incident(&mut self, id: u32, text: String, at: Vec3, near: bool) {
        if text.is_empty() {
            self.log.retain(|l| l.incident != Some(id));
            self.dirty = true;
            return;
        }
        let where_ = self.far_place(at);
        let shown = |near: bool| if near { text.clone() } else { format!("{text} ({where_})") };
        if let Some(l) = self.log.iter_mut().rev().find(|l| l.incident == Some(id)) {
            let near = near || l.kind == Kind::Notable;
            l.kind = if near { Kind::Notable } else { Kind::Far };
            l.color = if near { NOTABLE } else { FAR };
            l.text = shown(near);
            self.dirty = true;
            return;
        }
        let kind = if near { Kind::Notable } else { Kind::Far };
        let line = LogLine { incident: Some(id), ..LogLine::new(kind, None, shown(near), if near { NOTABLE } else { FAR }) };
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

    /// Is this line in the open view: the feed keeps what was said and
    /// done a moment ago, the journal what its filter asks for.
    fn in_view(&self, l: &LogLine, now: Instant) -> bool {
        if self.log_view > 0 {
            return self.journal_filter.keeps(l);
        }
        let age = now.duration_since(l.at);
        match l.kind {
            Kind::Far => false,
            Kind::Life => age < LIFE_FOR,
            _ => l.streaming.is_some() || age < feed_for(l),
        }
    }

    /// The line as styled text: its mark, who speaks, the words, how often.
    fn styled(l: &LogLine) -> Styled {
        let mut s: Styled = Vec::new();
        let put = |s: &mut Styled, t: &str, c: [u8; 3], b: bool| s.extend(t.chars().map(|ch| (ch, c, b)));
        match l.kind {
            Kind::Notable | Kind::Made => put(&mut s, "✦ ", MARK, true),
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

    /// The rows of the open view, oldest first, scrolled back `log_scroll`,
    /// wrapped by `wrap`.
    fn log_view_rows(&mut self, rows: usize, wrap: impl Fn(&Styled) -> Vec<Styled>) -> Vec<Styled> {
        let now = Instant::now();
        let mut out: Vec<Styled> = Vec::new();
        for l in self.log.iter().rev() {
            if !self.in_view(l, now) {
                continue;
            }
            out.extend(wrap(&Self::styled(l)).into_iter().rev());
            if out.len() >= rows + self.log_scroll {
                break;
            }
        }
        self.log_scroll = self.log_scroll.min(out.len().saturating_sub(rows));
        let mut shown: Vec<Styled> = out.into_iter().skip(self.log_scroll).take(rows).collect();
        shown.reverse();
        shown
    }

    /// Bottom left, over the world: the last few things said and done,
    /// each going after a while (the journal keeps them all).
    pub(super) fn draw_feed(&mut self, w: f32, h: f32) {
        if self.log_view > 0 {
            return;
        }
        // Beside the input line when there is room, else over it.
        let side = (w - hud::input_w(w)) * 0.5 - 3.2;
        let (fw, bottom) = if side >= 34.0 { (side.min(60.0), h - 0.8) } else { ((w - 3.2).min(60.0), h - 7.0) };
        let x = 1.6;
        let inner = fw - 2.0;
        let space = hud::width(" ", Face::Body, hud::SIZE);
        let rows = self.log_view_rows(FEED_ROWS, |s| {
            // A long line keeps to a few rows here (the journal has it all).
            let mut r = wrap_measured(s, inner, inner - 1.5, space);
            if r.len() > FEED_LINE_ROWS {
                r.truncate(FEED_LINE_ROWS);
                if let Some(last) = r.last_mut() {
                    let c = last.last().map(|c| c.1).unwrap_or(TEXT);
                    while last.len() > 1 && last.iter().map(|c| hud::width(&c.0.to_string(), Face::Body, hud::SIZE)).sum::<f32>() > inner - 3.0 {
                        last.pop();
                    }
                    last.push(('…', c, false));
                }
            }
            r
        });
        // Everyday lines leave on their own: redraw when the next one goes.
        let now = Instant::now();
        self.log_expiry = self.log.iter().filter(|l| l.kind != Kind::Far && l.streaming.is_none()).map(|l| l.at + if l.kind == Kind::Life { LIFE_FOR } else { feed_for(l) }).filter(|t| *t > now).min();
        if rows.is_empty() {
            return;
        }
        const LINE: f32 = 1.3;
        let ph = rows.len() as f32 * LINE + 0.9;
        let y0 = bottom - ph;
        let s = &mut self.screen;
        hud::shade(s, x, y0, fw, ph, 0.7);
        let mut y = y0 + 0.45;
        for (i, row) in rows.iter().enumerate() {
            let mid = y + LINE * 0.5;
            let indent = if i > 0 && row.first().is_some_and(|c| c.0 == HANG) { 1.5 } else { 0.0 };
            let mut px = x + 1.0 + indent;
            for (text, color, bold) in runs(row) {
                let face = if bold { Face::Bold } else { Face::Body };
                px = hud::words(s, px, mid, &text, hud::SIZE, color, face);
            }
            y += LINE;
        }
    }

    /// Rows in the journal panel.
    pub(super) fn journal_rows(&self) -> usize {
        (self.screen.h as usize).saturating_sub(13).max(3)
    }

    /// The journal (key 1): a panel over the view with everything kept,
    /// filtered (Tab), scrolled back (PgUp, the wheel).
    pub(super) fn draw_journal(&mut self, w: u16, h: u16) {
        let (wf, hf) = (w as f32, h as f32);
        let pw = (wf - 4.0).min(110.0);
        let (x, y, ph) = ((wf - pw) * 0.5, 1.5, hf - 3.0);
        let s = &mut self.screen;
        s.deco(Deco { x: 0.0, y: 0.0, w: wf, h: hf, fill: [6, 7, 12], alpha: 0.35, radius: 0.0, shadow: false, border: None });
        s.deco(Deco::panel(x, y, pw, ph, crate::ui::PANEL).border(crate::ui::PANEL_EDGE));
        s.labels.push(crate::ui::left(x + 2.0, y + 2.0, "Journal", 1.8, ACCENT, Face::Title));
        // The filters, as tabs.
        let mut tx = x + 2.0;
        let ty = y + 4.4;
        for f in Filter::ALL {
            let on = f == self.journal_filter;
            let name = f.name();
            let tw = hud::width(name, Face::Bold, 0.9) + 2.4;
            let d = Deco::pill(tx, ty - 0.85, tw, 1.7, if on { ACCENT } else { crate::ui::BUTTON });
            s.deco(if on { d } else { d.border(crate::ui::BUTTON_EDGE) });
            s.labels.push(crate::term::Label { x: tx + tw * 0.5, y: ty, text: name.to_string(), size: 0.9, color: if on { crate::ui::ON_ACCENT } else { TEXT }, face: Face::Bold, center: true });
            tx += tw + 0.8;
        }
        if self.log_scroll > 0 {
            hud::words(s, tx + 1.0, ty, &format!("↑ {} lines back", self.log_scroll), 0.85, DIM, Face::Body);
        }
        // The lines, in cells (the panel under them).
        let rows = self.journal_rows();
        let width = ((pw - 4.0) as usize).max(10);
        let lines = self.log_view_rows(rows, |s| {
            let mut wrapped = wrap_styled(s, width, width.saturating_sub(2));
            for r in wrapped.iter_mut().skip(1) {
                r.splice(0..0, [(' ', TEXT, false), (' ', TEXT, false)]);
            }
            wrapped
        });
        let top = (y + 6.4).ceil() as u16;
        let lx = (x + 2.0).ceil() as u16;
        for (i, row) in lines.iter().enumerate() {
            let yy = top + i as u16;
            for (j, (ch, fg, bold)) in row.iter().enumerate() {
                self.screen.over(lx + j as u16, yy, |c| Cell { ch: crate::term::narrow(*ch), fg: *fg, bold: *bold, ..c });
            }
        }
        if lines.is_empty() {
            self.screen.labels.push(crate::ui::left(x + 2.0, top as f32 + 0.5, "Nothing here yet.", 0.95, DIM, Face::Body));
        }
        // The keys, along the bottom.
        let s = &mut self.screen;
        let mid = y + ph - 1.5;
        let mut kx = x + 2.0;
        for (key, what) in [("Tab", "filter"), ("PgUp/PgDn", "scroll back"), ("1", "close")] {
            kx = hud::keycap(s, kx, mid, key);
            kx = hud::words(s, kx + 0.6, mid, what, 0.9, DIM, Face::Body) + 1.8;
        }
        self.log_expiry = None;
    }

    /// Open (1) or close (0) the journal.
    pub(super) fn set_log_view(&mut self, v: u8) {
        self.log_view = v;
        self.log_scroll = 0;
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
