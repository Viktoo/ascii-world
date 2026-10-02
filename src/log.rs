//! Debug log file (~/.pocket/pocket.log). The terminal is never written to
//! directly while the UI is running.

use parking_lot::Mutex;
use std::io::Write;

static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

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
