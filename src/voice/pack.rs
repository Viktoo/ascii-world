//! The voice pack: the model and its voices, kept in ~/.pocket/voice and
//! fetched once, the first time voices are on (about 350 MB). The speech
//! recogniser's model (`crate::ears`) is fetched into the same folder.

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use std::io::Write;
use std::path::PathBuf;

pub struct File {
    pub name: &'static str,
    pub url: &'static str,
    pub size: u64,
}

pub const FILES: &[File] = &[
    File { name: "kokoro-v1.0.onnx", url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/kokoro-v1.0.onnx", size: 325_532_387 },
    File { name: "voices-v1.0.bin", url: "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0/voices-v1.0.bin", size: 28_214_398 },
];

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
