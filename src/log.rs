//! Debug log file (~/.pocket/pocket.log). The terminal is never written to
//! directly while the UI is running.

use parking_lot::Mutex;
use std::io::Write;

static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);
static ECHO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Also write the log to stderr (command-line tools with no UI).
pub fn echo(on: bool) {
    ECHO.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn dir() -> std::path::PathBuf {
    std::env::var_os("POCKET_HOME").map(std::path::PathBuf::from).unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| ".".into()).join(".pocket"))
}

pub fn init() {
    let d = dir();
    let _ = std::fs::create_dir_all(&d);
    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("pocket.log")) {
        *FILE.lock() = Some(f);
    }
}

fn write(level: &str, msg: &str) {
    if ECHO.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("  {level} {msg}");
    }
    if let Some(f) = FILE.lock().as_mut() {
        let t = crate::db::now();
        let _ = writeln!(f, "{t:.3} {level} {msg}");
    }
}

pub fn info(msg: impl AsRef<str>) {
    write("INFO", msg.as_ref());
}

pub fn error(msg: impl AsRef<str>) {
    write("ERROR", msg.as_ref());
}
