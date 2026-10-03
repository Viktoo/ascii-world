//! Terminal setup/restore and the compositor: a cell grid diffed against what
//! is already on screen, emitted as one write per frame. Never clears.

use crossterm::event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::{cursor, execute, terminal};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static ENHANCED: AtomicBool = AtomicBool::new(false);

pub struct TermInfo {
    pub enhanced: bool,
    pub truecolor: bool,
}

pub fn truecolor() -> bool {
    let ct = std::env::var("COLORTERM").unwrap_or_default().to_lowercase();
    ct.contains("truecolor") || ct.contains("24bit")
}

pub fn enter() -> std::io::Result<TermInfo> {
    let mut out = std::io::stdout();
    terminal::enable_raw_mode()?;
    ACTIVE.store(true, Ordering::SeqCst);
    install_panic_hook();
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide, terminal::DisableLineWrap, crossterm::event::EnableFocusChange)?;
    let enhanced = std::env::var("POCKET_NO_KEYBOARD_ENHANCEMENT").is_err() && terminal::supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        execute!(
            out,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;
        ENHANCED.store(true, Ordering::SeqCst);
    }
    Ok(TermInfo { enhanced, truecolor: truecolor() })
}

/// Restore the terminal. Safe to call more than once and from a panic hook.
pub fn restore() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut out = std::io::stdout();
    // End any synchronized update and reset attributes first.
    let _ = out.write_all(b"\x1b[?2026l\x1b[0m");
    if ENHANCED.swap(false, Ordering::SeqCst) {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(out, crossterm::event::DisableFocusChange, terminal::EnableLineWrap, cursor::Show, terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    let _ = out.flush();
}

fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            crate::log::error(format!("panic: {info}"));
            prev(info);
        }));
    });
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

/// Colours within this distance of what is on screen are not re-sent.
const TOLERANCE: i32 = 3;

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= TOLERANCE)
}

pub struct Screen {
    pub w: u16,
    pub h: u16,
    pub cells: Vec<Cell>,
    shown: Vec<Option<Cell>>,
    pub truecolor: bool,
    pub bytes_last: usize,
}

impl Screen {
    pub fn new(w: u16, h: u16, truecolor: bool) -> Screen {
        let n = w as usize * h as usize;
        Screen { w, h, cells: vec![Cell::BLANK; n], shown: vec![None; n], truecolor, bytes_last: 0 }
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        *self = Screen::new(w, h, self.truecolor);
    }

    pub fn set(&mut self, x: u16, y: u16, c: Cell) {
        if x < self.w && y < self.h {
            self.cells[y as usize * self.w as usize + x as usize] = c;
        }
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
    fn colours_and_wrap() {
        assert_eq!(ansi256([255, 0, 0]), 196);
        assert_eq!(ansi256([0, 0, 0]), 16);
        assert_eq!(wrap("aaa bbb ccc", 7), vec!["aaa bbb", "ccc"]);
        assert_eq!(narrow('▀'), '▀');
        assert_eq!(narrow('漢'), '?');
    }
}
