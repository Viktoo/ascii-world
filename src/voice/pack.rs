//! The voice pack: the model and its voices, kept in ~/.pocket/voice and
//! fetched once, when the player asks for it in settings (about 350 MB). The
//! speech recogniser's model (`crate::ears`) is fetched into the same folder.
//! Nothing is fetched unasked.

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use std::io::Write;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct File {
    pub name: &'static str,
    pub url: &'static str,
    pub size: u64,
}

pub const FILES: &[File] = &[
    File { name: "kokoro-v1.0.onnx", url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx", size: 325_532_387 },
    File { name: "voices-v1.0.bin", url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin", size: 28_214_398 },
];

/// A download the player can start from settings: what it is, where it
/// comes from, and how it is going.
pub struct Pack {
    pub files: &'static [File],
    /// The public page it comes from, shown under the button.
    pub page: &'static str,
    state: Mutex<Get>,
    /// Just finished, not yet acted on.
    done: AtomicBool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Get {
    Idle,
    /// Share done.
    Fetching(f32),
    Failed(String),
}

/// The characters' voices (Kokoro).
pub static VOICES: Pack = Pack::new(FILES, "github.com/thewh1teagle/kokoro-onnx");
/// The speech recogniser's model, for speak to talk (whisper.cpp).
pub static SPEECH: Pack = Pack::new(&[crate::ears::MODEL], "huggingface.co/ggerganov/whisper.cpp");

impl Pack {
    const fn new(files: &'static [File], page: &'static str) -> Pack {
        Pack { files, page, state: parking_lot::const_mutex(Get::Idle), done: AtomicBool::new(false) }
    }

    /// All its files are here (never in tests: they see the download rows).
    pub fn ready(&self) -> bool {
        !cfg!(test) && ready_of(self.files)
    }

    pub fn megabytes(&self) -> u64 {
        (self.files.iter().map(|f| f.size).sum::<u64>() + 500_000) / 1_000_000
    }

    pub fn state(&self) -> Get {
        self.state.lock().clone()
    }

    /// Start fetching in the background (once at a time; never in tests).
    pub fn start(&'static self) {
        if cfg!(test) {
            return;
        }
        {
            let mut st = self.state.lock();
            if matches!(*st, Get::Fetching(_)) {
                return;
            }
            *st = Get::Fetching(0.0);
        }
        let spawned = std::thread::Builder::new().name("fetch".into()).spawn(move || {
            let r = fetch_of(self.files, &|p| *self.state.lock() = Get::Fetching(p));
            *self.state.lock() = match r {
                Ok(()) => {
                    self.done.store(true, Ordering::Relaxed);
                    Get::Idle
                }
                Err(e) => {
                    crate::log::info(&format!("fetch {}: {e:#}", self.page));
                    Get::Failed(format!("{e}"))
                }
            };
        });
        if let Err(e) = spawned {
            *self.state.lock() = Get::Failed(format!("{e}"));
        }
    }

    /// True once after a fetch finishes.
    pub fn take_done(&self) -> bool {
        self.done.swap(false, Ordering::Relaxed)
    }
}

pub fn dir() -> PathBuf {
    std::env::var_os("POCKET_VOICE_DIR").map(PathBuf::from).unwrap_or_else(|| crate::log::dir().join("voice"))
}

pub fn path(name: &str) -> PathBuf {
    dir().join(name)
}

pub fn ready() -> bool {
    ready_of(FILES)
}

pub fn ready_of(files: &[File]) -> bool {
    files.iter().all(|f| std::fs::metadata(path(f.name)).is_ok_and(|m| m.len() == f.size))
}

/// Fetch what is missing; `progress` hears the share done (0..1).
pub fn fetch(progress: &dyn Fn(f32)) -> Result<()> {
    fetch_of(FILES, progress)
}

/// Fetch what is missing of `files` (into the same folder).
pub fn fetch_of(files: &[File], progress: &dyn Fn(f32)) -> Result<()> {
    std::fs::create_dir_all(dir())?;
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    let total: u64 = files.iter().map(|f| f.size).sum();
    let mut done: u64 = 0;
    for f in files {
        let dest = path(f.name);
        if std::fs::metadata(&dest).is_ok_and(|m| m.len() == f.size) {
            done += f.size;
            continue;
        }
        let part = dest.with_extension("part");
        rt.block_on(async {
            let client = reqwest::Client::builder().build()?;
            let res = client.get(f.url).send().await?.error_for_status().with_context(|| format!("fetching {}", f.url))?;
            let mut out = std::fs::File::create(&part)?;
            let mut got: u64 = 0;
            let mut stream = res.bytes_stream();
            let mut last = 0.0f32;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                out.write_all(&chunk)?;
                got += chunk.len() as u64;
                let share = (done + got) as f32 / total as f32;
                if share - last > 0.01 {
                    last = share;
                    progress(share);
                }
            }
            out.flush()?;
            if got != f.size {
                bail!("{}: got {got} bytes, expected {}", f.name, f.size);
            }
            Ok::<(), anyhow::Error>(())
        })?;
        std::fs::rename(&part, &dest)?;
        done += f.size;
    }
    progress(1.0);
    Ok(())
}
