//! The screen as a grid of text cells: what every screen of the game draws
//! into. The window sets it over the world (`window`); without one it can be
//! written out as terminal text, diffed against what was sent last.

use std::io::Write;
use std::sync::atomic::AtomicBool;

pub fn truecolor() -> bool {
    let ct = std::env::var("COLORTERM").unwrap_or_default().to_lowercase();
    ct.contains("truecolor") || ct.contains("24bit")
}

/// Set by SIGTERM / SIGHUP / SIGINT; the main loop exits cleanly when it flips.
pub fn signal_flag() -> std::sync::Arc<AtomicBool> {
    let f = std::sync::Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGHUP, signal_hook::consts::SIGINT, signal_hook::consts::SIGQUIT] {
        let _ = signal_hook::flag::register(sig, f.clone());
    }
    f
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub ch: char,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub bold: bool,
}

impl Cell {
    pub const BLANK: Cell = Cell { ch: ' ', fg: [200, 200, 200], bg: [0, 0, 0], bold: false };
}

/// Whether a hint's first word names a key ("Enter", "F2", "W/S", "e"), not a thing.
pub fn is_key(k: &str) -> bool {
    const NAMED: &[&str] = &["Enter", "Esc", "Tab", "Space", "Shift", "PgUp", "PgDn", "Home", "End", "Backspace"];
    let n = k.chars().count();
    n > 0 && (n <= 2 || NAMED.contains(&k) || k.contains('/') || k.contains('+') || (k.starts_with('F') && k[1..].chars().all(|c| c.is_ascii_digit())))
}

pub const KEYCAP: [u8; 3] = [52, 56, 76];
pub const KEYCAP_EDGE: [u8; 3] = [92, 98, 130];
pub const KEYCAP_TEXT: [u8; 3] = [236, 232, 220];

/// Colours within this distance of what is on screen are not re-sent.
const TOLERANCE: i32 = 3;

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= TOLERANCE)
}

/// A soft shape drawn under the text (positions in cells, fractions allowed):
/// panels, pills, keycaps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deco {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub fill: [u8; 3],
    pub alpha: f32,
    /// Corner radius, in rows.
    pub radius: f32,
    pub shadow: bool,
    pub border: Option<[u8; 3]>,
}

impl Deco {
    pub fn panel(x: f32, y: f32, w: f32, h: f32, fill: [u8; 3]) -> Deco {
        Deco { x, y, w, h, fill, alpha: 1.0, radius: 0.6, shadow: true, border: None }
    }

    pub fn pill(x: f32, y: f32, w: f32, h: f32, fill: [u8; 3]) -> Deco {
        Deco { x, y, w, h, fill, alpha: 1.0, radius: h * 0.5, shadow: false, border: None }
    }

    pub fn border(self, c: [u8; 3]) -> Deco {
        Deco { border: Some(c), ..self }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// The display face, for titles.
    Title,
    Body,
    Bold,
}

/// Text set freely, not in cells: titles, keycaps. `x`, `y` in cells (top
/// left, or the centre of the line when `center`); `size` in rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub size: f32,
    pub color: [u8; 3],
    pub face: Face,
    pub center: bool,
}

#[derive(Clone)]
pub struct Screen {
    pub w: u16,
    pub h: u16,
    pub cells: Vec<Cell>,
    shown: Vec<Option<Cell>>,
    pub truecolor: bool,
    pub bytes_last: usize,
    /// Cells that show the world beneath them (window mode only): the world's
    /// own pixels, multiplied by the cell's background (white leaves it as is).
    pub see: Vec<bool>,
    /// A text cursor (in a window): before the letter in this cell, in this
    /// colour, wherever the words set it.
    pub caret: Option<(u16, u16, [u8; 3])>,
    /// Shapes and free text over the cells (each screen draws them anew).
    pub decor: Vec<Deco>,
    pub labels: Vec<Label>,
}

impl Screen {
    pub fn new(w: u16, h: u16, truecolor: bool) -> Screen {
        let n = w as usize * h as usize;
        Screen { w, h, cells: vec![Cell::BLANK; n], shown: vec![None; n], truecolor, bytes_last: 0, see: vec![false; n], caret: None, decor: Vec::new(), labels: Vec::new() }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        *self = Screen::new(w, h, self.truecolor);
    }

    pub fn set(&mut self, x: u16, y: u16, c: Cell) {
        if x < self.w && y < self.h {
            let i = y as usize * self.w as usize + x as usize;
            self.cells[i] = c;
            self.see[i] = false;
        }
    }

    /// Change a cell in place, keeping whatever shows through it: tints and
    /// marks drawn over the world stay over the world in a window.
    pub fn over(&mut self, x: u16, y: u16, f: impl FnOnce(Cell) -> Cell) {
        if x < self.w && y < self.h {
            let i = y as usize * self.w as usize + x as usize;
            self.cells[i] = f(self.cells[i]);
        }
    }

    /// The top `rows` rows show the world itself (window mode).
    pub fn fill_world(&mut self, rows: u16) {
        let n = rows.min(self.h) as usize * self.w as usize;
        self.cells[..n].fill(Cell { ch: ' ', fg: [255, 255, 255], bg: [255, 255, 255], bold: false });
        self.see[..n].fill(true);
    }

    pub fn get(&self, x: u16, y: u16) -> Option<Cell> {
        (x < self.w && y < self.h).then(|| self.cells[y as usize * self.w as usize + x as usize])
    }

    /// Write text; returns the column after it. Wide characters are replaced.
    pub fn text(&mut self, x: u16, y: u16, s: &str, fg: [u8; 3], bg: [u8; 3], bold: bool) -> u16 {
        let mut cx = x;
        for ch in s.chars() {
            if cx >= self.w {
                break;
            }
            self.set(cx, y, Cell { ch: narrow(ch), fg, bg, bold });
            cx += 1;
        }
        cx
    }

    pub fn fill_row(&mut self, y: u16, c: Cell) {
        for x in 0..self.w {
            self.set(x, y, c);
        }
    }

    /// Forget the shapes and free text (at the start of each screen drawn).
    pub fn clear_decor(&mut self) {
        self.decor.clear();
        self.labels.clear();
        self.caret = None;
    }

    /// Text centred on column `cx`, set freely (not in cells).
    pub fn center_text(&mut self, cx: f32, y: u16, text: &str, color: [u8; 3]) {
        self.labels.push(Label { x: cx, y: y as f32 + 0.5, text: text.to_string(), size: 1.0, color, face: Face::Body, center: true });
    }

    pub fn deco(&mut self, d: Deco) {
        self.decor.push(d);
    }

    /// A title in the display face, `rows` tall, centred on column `cx`.
    pub fn title(&mut self, cx: f32, y: f32, text: &str, rows: f32, color: [u8; 3]) {
        self.labels.push(Label { x: cx, y, text: text.to_string(), size: rows, color, face: Face::Title, center: true });
    }

    /// A key drawn as a keycap; returns the column after it. The cells under
    /// it are left blank (the cap and its letters are drawn over them).
    pub fn keycap(&mut self, x: u16, y: u16, key: &str, bg: [u8; 3]) -> u16 {
        let n = key.chars().count().max(1) as u16 + 1;
        for i in 0..n {
            self.set(x + i, y, Cell { ch: ' ', fg: KEYCAP_TEXT, bg, bold: false });
        }
        self.decor.push(Deco { x: x as f32 + 0.05, y: y as f32 + 0.1, w: n as f32 - 0.1, h: 0.8, fill: KEYCAP, alpha: 1.0, radius: 0.22, shadow: false, border: Some(KEYCAP_EDGE) });
        // Arrows are small in the face: larger, so they read.
        let size = if key.chars().any(|c| c.is_alphanumeric()) { 0.74 } else { 1.0 };
        self.labels.push(Label { x: x as f32 + n as f32 * 0.5, y: y as f32 + 0.5, text: key.to_string(), size, color: KEYCAP_TEXT, face: Face::Bold, center: true });
        x + n
    }

    /// A line of hints, "KEY what   KEY what": each key as a keycap, the rest as text.
    /// Returns the column after it.
    pub fn hints(&mut self, x: u16, y: u16, s: &str, fg: [u8; 3], bg: [u8; 3]) -> u16 {
        let mut cx = x;
        for (i, part) in s.split("  ").map(str::trim).filter(|p| !p.is_empty()).enumerate() {
            if cx >= self.w {
                break;
            }
            if i > 0 {
                cx += 3;
            }
            // "the axe: e pick up": the name as words, then the key.
            let part = match part.split_once(": ") {
                Some((name, rest)) if !is_key(name) && rest.split_once(' ').is_some_and(|(k, _)| is_key(k)) => {
                    cx = self.text(cx, y, &format!("{name}: "), fg, bg, false);
                    rest
                }
                _ => part,
            };
            match part.split_once(' ') {
                Some((k, rest)) if is_key(k.trim_end_matches(':')) => {
                    cx = self.keycap(cx, y, k.trim_end_matches(':'), bg);
                    cx = self.text(cx + 1, y, rest, fg, bg, false);
                }
                _ => cx = self.text(cx, y, part, fg, bg, false),
            }
        }
        cx
    }

    /// What the screen says, cells and free text together (for tests and agents).
    pub fn plain_text(&self) -> String {
        let mut rows: Vec<Vec<char>> = (0..self.h).map(|y| (0..self.w).map(|x| self.get(x, y).map_or(' ', |c| c.ch)).collect()).collect();
        for l in &self.labels {
            let n = l.text.chars().count() as f32;
            let x0 = if l.center { l.x - n * 0.5 } else { l.x }.max(0.0) as usize;
            let y = l.y.max(0.0) as usize;
            if let Some(row) = rows.get_mut(y) {
                for (i, ch) in l.text.chars().enumerate() {
                    if let Some(c) = row.get_mut(x0 + i) {
                        *c = ch;
                    }
                }
            }
        }
        rows.into_iter().map(|r| r.into_iter().collect::<String>()).collect::<Vec<_>>().join("\n")
    }

    /// Emit only what changed, as a single buffer.
    pub fn flush(&mut self, out: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
        buf.extend_from_slice(b"\x1b[?2026h");
        let mut pen_fg: Option<[u8; 3]> = None;
        let mut pen_bg: Option<[u8; 3]> = None;
        let mut pen_bold = false;
        buf.extend_from_slice(b"\x1b[0m");
        let mut cur: Option<(u16, u16)> = None;
        let w = self.w as usize;
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y as usize * w + x as usize;
                let c = self.cells[i];
                if let Some(s) = self.shown[i] {
                    if s.ch == c.ch && s.bold == c.bold && close(s.fg, c.fg) && close(s.bg, c.bg) {
                        continue;
                    }
                }
                if cur != Some((x, y)) {
                    let _ = write!(buf, "\x1b[{};{}H", y + 1, x + 1);
                }
                if c.bold != pen_bold {
                    buf.extend_from_slice(if c.bold { b"\x1b[1m" } else { b"\x1b[22m" });
                    pen_bold = c.bold;
                }
                let fg_change = pen_fg != Some(c.fg) && c.ch != ' ';
                let bg_change = pen_bg != Some(c.bg);
                if fg_change || bg_change {
                    buf.extend_from_slice(b"\x1b[");
                    if fg_change {
                        self.color(&mut buf, c.fg, 38);
                        pen_fg = Some(c.fg);
                    }
                    if bg_change {
                        if fg_change {
                            buf.push(b';');
                        }
                        self.color(&mut buf, c.bg, 48);
                        pen_bg = Some(c.bg);
                    }
                    buf.push(b'm');
                }
                let mut tmp = [0u8; 4];
                buf.extend_from_slice(c.ch.encode_utf8(&mut tmp).as_bytes());
                self.shown[i] = Some(c);
                cur = Some((x + 1, y));
            }
        }
        buf.extend_from_slice(b"\x1b[0m\x1b[?2026l");
        self.bytes_last = buf.len();
        out.write_all(&buf)?;
        out.flush()
    }

    fn color(&self, buf: &mut Vec<u8>, c: [u8; 3], base: u8) {
        if self.truecolor {
            let _ = write!(buf, "{};2;{};{};{}", base, c[0], c[1], c[2]);
        } else {
            let _ = write!(buf, "{};5;{}", base, ansi256(c));
        }
    }
}

/// Nearest xterm-256 colour (cube or grey ramp).
pub fn ansi256(c: [u8; 3]) -> u8 {
    let q = |v: u8| -> u8 {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            (v - 35) / 40
        }
    };
    let lv = [0u8, 95, 135, 175, 215, 255];
    let (r, g, b) = (q(c[0]), q(c[1]), q(c[2]));
    let cube = [lv[r as usize], lv[g as usize], lv[b as usize]];
    let avg = (c[0] as u32 + c[1] as u32 + c[2] as u32) / 3;
    let gi = if avg > 238 { 23 } else { (avg.saturating_sub(3)) / 10 };
    let gv = (8 + gi * 10) as u8;
    let d = |a: [u8; 3]| -> u32 { (0..3).map(|i| (a[i] as i32 - c[i] as i32).pow(2) as u32).sum() };
    if d([gv, gv, gv]) < d(cube) { 232 + gi as u8 } else { 16 + 36 * r + 6 * g + b }
}

/// Characters assumed single-width; anything else is replaced.
pub fn narrow(c: char) -> char {
    let u = c as u32;
    if c.is_control() {
        ' '
    } else if u < 0x1100 || (0x2000..0x2E80).contains(&u) || (0xFB00..0xFE00).contains(&u) {
        c
    } else {
        '?'
    }
}

/// Word-wrap to `width` columns; a line break starts a new line.
pub fn wrap(s: &str, width: usize) -> Vec<String> {
    if s.contains('\n') {
        return s.split('\n').flat_map(|p| wrap(p, width)).collect();
    }
    let width = width.max(8);
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        let wl = word.chars().count();
        let cl = cur.chars().count();
        if cl > 0 && cl + 1 + wl > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        if wl > width {
            for ch in word.chars() {
                if cur.chars().count() >= width {
                    lines.push(std::mem::take(&mut cur));
                }
                cur.push(ch);
            }
        } else {
            cur.push_str(word);
        }
    }
    lines.push(cur);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_only_emits_changes() {
        let mut s = Screen::new(10, 3, true);
        let mut out = Vec::new();
        s.flush(&mut out).unwrap();
        let first = out.len();
        out.clear();
        s.flush(&mut out).unwrap();
        assert!(out.len() < 40, "unchanged frame should be nearly empty, got {} bytes", out.len());
        s.set(3, 1, Cell { ch: 'x', fg: [255, 0, 0], bg: [0, 0, 0], bold: false });
        out.clear();
        s.flush(&mut out).unwrap();
        let t = String::from_utf8_lossy(&out);
        assert!(t.contains("\x1b[2;4H") && t.contains('x'));
        assert!(out.len() < first);
    }

    #[test]
    fn hints_draw_keys_as_caps() {
        let mut s = Screen::new(60, 2, true);
        s.hints(0, 0, "Enter begin   Rusty axe: e pick up  F2 inspect", [200, 200, 200], [0, 0, 0]);
        let t = s.plain_text();
        assert!(t.contains("Enter") && t.contains("begin") && t.contains("Rusty axe:") && t.contains("F2"), "{t}");
        assert_eq!(s.decor.len(), 3, "Enter, e and F2 are keys; 'Rusty' is not");
        assert!(is_key("W/S") && is_key("e") && is_key("Ctrl+U") && is_key("F10") && !is_key("Rusty"));
    }

    #[test]
    fn colours_and_wrap() {
        assert_eq!(ansi256([255, 0, 0]), 196);
        assert_eq!(ansi256([0, 0, 0]), 16);
        assert_eq!(wrap("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
        assert_eq!(narrow('▀'), '▀');
        assert_eq!(narrow('漢'), '?');
    }
}
