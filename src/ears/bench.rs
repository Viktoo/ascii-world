//! `pocket listen --hear`: the recogniser on a WAV file, as the game would
//! hear it live (words every 0.4 s of speech, the whole line at the pause),
//! with how long each pass took; or `--mic`, one go through the microphone.

use super::pause::{FRAME, Pause, RATE, Step};
use super::{Cmd, Ears, Heard, MODEL, Recogniser, State, mic, threads};
use crate::voice::pack;
use anyhow::{Context, Result, bail};
use std::time::Instant;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn fetch() -> Result<()> {
    pack::fetch_of(&[MODEL], &|p| eprint!("\rfetching the speech model: {:.0}%", p * 100.0))?;
    eprintln!();
    Ok(())
}

pub fn cli(args: &[String]) -> Result<()> {
    if args.iter().any(|a| a == "--mic") {
        return mic_once(&flag(args, "--prompt").unwrap_or_default());
    }
    let file = flag(args, "--hear").context("pocket listen --hear FILE.wav [--model PATH] [--gpu] [--prompt TEXT]")?;
    let model = flag(args, "--model").map(std::path::PathBuf::from).unwrap_or_else(|| pack::path(MODEL.name));
    if !model.exists() {
        fetch()?;
    }
    let pcm = read_wav(std::path::Path::new(&file))?;
    let gpu = args.iter().any(|a| a == "--gpu");
    let prompt = flag(args, "--prompt").unwrap_or_default();
    let t = Instant::now();
    let mut rec = Recogniser::load(&model, gpu, threads())?;
    println!("{} loaded in {:.2}s ({}, {} threads)", model.display(), t.elapsed().as_secs_f32(), if gpu { "gpu" } else { "cpu" }, threads());
    let mut pause = Pause::default();
    let mut next_pass = 0usize;
    let mut worst = 0.0f32;
    for (i, f) in pcm.chunks(FRAME).enumerate() {
        let step = pause.frame(super::pause::level(f));
        let end = (i + 1) * FRAME;
        let from = pause.began().map(|b| ((b - super::BEFORE).max(0.0) * RATE as f32) as usize).unwrap_or(end).min(end);
        if step == Step::Speech && end >= next_pass {
            next_pass = end + (super::LIVE_EVERY.as_secs_f32() * RATE as f32) as usize;
            let t = Instant::now();
            let words = rec.hear(&pcm[from..end], &prompt, true)?;
            let ms = t.elapsed().as_secs_f32() * 1000.0;
            worst = worst.max(ms);
            println!("{:5.2}s  live {ms:4.0} ms  {words}", end as f32 / RATE as f32);
        }
        if matches!(step, Step::Done | Step::Nothing) || end >= pcm.len() {
            let t = Instant::now();
            let words = if pause.began().is_some() { rec.hear(&pcm[from..end], &prompt, false)? } else { String::new() };
            println!("{:5.2}s  {step:?}, whole line {:4.0} ms: {words}", end as f32 / RATE as f32, t.elapsed().as_secs_f32() * 1000.0);
            break;
        }
    }
    println!("slowest live pass {worst:.0} ms");
    Ok(())
}

/// One go through the microphone, as in the game.
fn mic_once(prompt: &str) -> Result<()> {
    if !pack::SPEECH.ready() {
        fetch()?;
    }
    let ears = Ears::start();
    loop {
        match ears.state() {
            State::Ready => break,
            State::Failed(e) => bail!(e),
            State::Starting => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprintln!("listening… (speak, then pause)");
    let t = Instant::now();
    ears.send(Cmd::Listen { id: 1, prompt: prompt.to_string() });
    loop {
        for h in ears.poll() {
            match h {
                Heard::Words { text, .. } => eprintln!("{:5.2}s  {text}", t.elapsed().as_secs_f32()),
                Heard::Done { text, .. } => {
                    println!("{:5.2}s  done: {text}", t.elapsed().as_secs_f32());
                    return Ok(());
                }
                Heard::Trouble { why, .. } => bail!(why),
                Heard::MicCheck(_) => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// A WAV (16-bit or float, any rate and channels) as 16 kHz mono.
pub fn read_wav(path: &std::path::Path) -> Result<Vec<f32>> {
    let b = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    anyhow::ensure!(b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WAVE", "not a WAV file");
    let (mut fmt, mut ch, mut rate, mut bits) = (0u16, 1usize, 16000u32, 16u16);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into()?) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if id == b"fmt " {
            fmt = u16::from_le_bytes(body[0..2].try_into()?);
            ch = u16::from_le_bytes(body[2..4].try_into()?).max(1) as usize;
            rate = u32::from_le_bytes(body[4..8].try_into()?);
            bits = u16::from_le_bytes(body[14..16].try_into()?);
        } else if id == b"data" {
            let samples: Vec<f32> = match (fmt, bits) {
                (1, 16) => body.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).collect(),
                (3, 32) => body.chunks_exact(4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]])).collect(),
                (0xFFFE, 16) => body.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).collect(),
                _ => bail!("WAV format {fmt} at {bits} bits isn't read here (16-bit or float)"),
            };
            let mono: Vec<f32> = samples.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
            let mut out = Vec::new();
            mic::Resample::new(rate).push(&mono, &mut out);
            return Ok(out);
        }
        i += 8 + len + (len & 1);
    }
    bail!("no audio in {}", path.display())
}
