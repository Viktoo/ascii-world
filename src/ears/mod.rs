//! The player's voice, heard: speak to talk instead of typing. All on this
//! machine, nothing sent anywhere:
//!
//! - `mic`: the microphone, brought to 16 kHz mono.
//! - `pause`: when speech starts and when it has ended (a pause).
//! - `words`: the recogniser's text tidied and put in the input line.
//! - here: the recogniser (whisper.cpp, through `whisper-rs`) and the thread
//!   that listens, sending words as they are made out and the whole line at
//!   the pause.
//!
//! The model (`ggml-base.en.bin`, 148 MB) is fetched once into the voice
//! folder (`voice::pack::dir()`). The mic is only open while listening.

pub mod bench;
pub mod mic;
pub mod pause;
pub mod words;

use crate::voice::pack::{self, File};
use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender, unbounded};
use parking_lot::Mutex;
use pause::{FRAME, Pause, RATE, Step};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub const MODEL: File = File { name: "ggml-base.en.bin", url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin", size: 147_964_211 };

/// Words as they are made out, this often while speaking.
const LIVE_EVERY: Duration = Duration::from_millis(400);
/// Kept from before speech was first loud enough (its first sound is quiet).
const BEFORE: f32 = 0.3;

/// Threads for the recogniser: the same share as the voices take.
pub fn threads() -> usize {
    crate::voice::threads()
}

/// Whether the recogniser runs on the GPU (`POCKET_EARS_GPU=1`). The CPU is
/// fast enough for a few seconds of speech and leaves the GPU to the world.
fn gpu() -> bool {
    std::env::var("POCKET_EARS_GPU").is_ok_and(|v| v != "0")
}

/// whisper.cpp with a state of its own.
pub struct Recogniser {
    _ctx: whisper_rs::WhisperContext,
    state: whisper_rs::WhisperState,
    threads: usize,
}

impl Recogniser {
    pub fn load(path: &std::path::Path, gpu: bool, threads: usize) -> Result<Recogniser> {
        whisper_rs::install_logging_hooks();
        let mut p = whisper_rs::WhisperContextParameters::default();
        p.use_gpu(gpu);
        let ctx = whisper_rs::WhisperContext::new_with_params(path.to_str().ok_or_else(|| anyhow!("model path"))?, p).map_err(|e| anyhow!("loading {}: {e}", path.display()))?;
        let state = ctx.create_state().map_err(|e| anyhow!("{e}"))?;
        Ok(Recogniser { _ctx: ctx, state, threads })
    }

    /// The words in `pcm` (16 kHz mono). `quick` looks at only as much
    /// audio as there is (the model otherwise always takes 30 s), for live
    /// words; the final pass takes it all. `prompt`: names likely to be said.
    pub fn hear(&mut self, pcm: &[f32], prompt: &str, quick: bool) -> Result<String> {
        use whisper_rs::{FullParams, SamplingStrategy};
        // It wants a second at least.
        let mut padded;
        let pcm = if pcm.len() < RATE + RATE / 10 {
            padded = pcm.to_vec();
            padded.resize(RATE + RATE / 10, 0.0);
            &padded[..]
        } else {
            pcm
        };
        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        p.set_n_threads(self.threads as i32);
        p.set_language(Some("en"));
        p.set_translate(false);
        p.set_no_context(true);
        p.set_no_timestamps(true);
        p.set_single_segment(true);
        p.set_suppress_blank(true);
        p.set_suppress_nst(true);
        p.set_print_special(false);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_timestamps(false);
        if quick {
            // 50 frames a second of audio, and some room.
            let secs = pcm.len() as f32 / RATE as f32;
            p.set_audio_ctx((((secs + 1.0) * 50.0) as i32).clamp(128, 1500));
            p.set_temperature_inc(0.0);
            // About as many tokens as could be said in that time.
            p.set_max_tokens((secs * 8.0) as i32 + 8);
        }
        if !prompt.is_empty() {
            p.set_initial_prompt(prompt);
        }
        self.state.full(p, pcm).map_err(|e| anyhow!("{e}"))?;
        let mut text = String::new();
        for seg in self.state.as_iter() {
            text.push_str(&seg.to_str_lossy().map_err(|e| anyhow!("{e}"))?);
        }
        let text = words::clean(&text);
        Ok(if quick { words::unrepeat_live(&text) } else { words::unrepeat(&text) })
    }
}

pub enum Cmd {
    /// Start listening; `prompt` names who and what may be spoken of.
    Listen { id: u64, prompt: String },
    /// Stop now and send what was said.
    Finish,
    /// Stop and forget it.
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Heard {
    /// The words so far (they may still change).
    Words { id: u64, text: String },
    /// The whole line, after the pause (empty: nothing was said).
    Done { id: u64, text: String },
    /// Couldn't listen.
    Trouble { id: u64, why: String },
    /// What asking for the mic came to: it hears, or why not.
    MicCheck(Result<(), String>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Starting,
    /// Fetching the model: share done.
    Fetching(f32),
    Ready,
    Failed(String),
}

/// The game's handle on the listening thread.
pub struct Ears {
    tx: Option<Sender<Cmd>>,
    rx: Receiver<Heard>,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    /// The mic's loudness now (0..1, as f32 bits), while listening.
    level: Arc<AtomicU32>,
    /// The mic is open (it may wait on the system asking the player first).
    open: Arc<AtomicBool>,
    heard: Sender<Heard>,
}

impl Ears {
    /// Start the thread (fetching the model first, if it must).
    pub fn start() -> Ears {
        let (tx, cmds) = unbounded::<Cmd>();
        let (heard, rx) = unbounded::<Heard>();
        let state = Arc::new(Mutex::new(State::Starting));
        let stop = Arc::new(AtomicBool::new(false));
        let level = Arc::new(AtomicU32::new(0));
        let open = Arc::new(AtomicBool::new(false));
        let (st, sp, lv, op) = (state.clone(), stop.clone(), level.clone(), open.clone());
        let h = heard.clone();
        let _ = std::thread::Builder::new().name("ears".into()).spawn(move || run(cmds, h, st, sp, lv, op));
        Ears { tx: Some(tx), rx, state, stop, level, open, heard }
    }

    /// Open the mic once, so the system asks the player for it now rather
    /// than on the first word; `Heard::MicCheck` says how it went.
    pub fn ask_mic(&self) {
        let heard = self.heard.clone();
        let _ = std::thread::Builder::new().name("mic check".into()).spawn(move || {
            let _ = heard.send(Heard::MicCheck(mic::check()));
        });
    }

    pub fn state(&self) -> State {
        self.state.lock().clone()
    }

    pub fn send(&self, c: Cmd) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }

    pub fn poll(&self) -> Vec<Heard> {
        self.rx.try_iter().collect()
    }

    pub fn mic_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

impl Drop for Ears {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.tx = None;
    }
}

fn run(cmds: Receiver<Cmd>, heard: Sender<Heard>, state: Arc<Mutex<State>>, stop: Arc<AtomicBool>, level: Arc<AtomicU32>, open: Arc<AtomicBool>) {
    if !pack::ready_of(&[MODEL]) {
        *state.lock() = State::Fetching(0.0);
        let st = state.clone();
        if let Err(e) = pack::fetch_of(&[MODEL], &|p| *st.lock() = State::Fetching(p)) {
            crate::log::info(&format!("speech model: {e:#}"));
            *state.lock() = State::Failed(format!("couldn't fetch the speech model: {e}"));
            return;
        }
    }
    if stop.load(Ordering::Relaxed) {
        return;
    }
    let t = Instant::now();
    let mut rec = match Recogniser::load(&pack::path(MODEL.name), gpu(), threads()) {
        Ok(r) => r,
        Err(e) => {
            crate::log::info(&format!("ears: {e:#}"));
            *state.lock() = State::Failed(format!("{e}"));
            return;
        }
    };
    crate::log::info(&format!("ears: model loaded in {:.2}s (gpu {})", t.elapsed().as_secs_f32(), gpu()));
    *state.lock() = State::Ready;
    let mut next: Option<Cmd> = None;
    loop {
        let c = match next.take() {
            Some(c) => c,
            None => match cmds.recv_timeout(Duration::from_millis(250)) {
                Ok(c) => c,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    continue;
                }
                Err(_) => return,
            },
        };
        if let Cmd::Listen { id, prompt } = c {
            next = listen(&mut rec, id, &prompt, &cmds, &heard, &stop, &level, &open);
            level.store(0, Ordering::Relaxed);
            open.store(false, Ordering::Relaxed);
        }
    }
}

/// One go: the mic open until the pause (or Finish / Cancel). Returns a
/// new Listen that arrived meanwhile.
#[allow(clippy::too_many_arguments)]
fn listen(rec: &mut Recogniser, id: u64, prompt: &str, cmds: &Receiver<Cmd>, heard: &Sender<Heard>, stop: &AtomicBool, level: &AtomicU32, open: &AtomicBool) -> Option<Cmd> {
    let (tx, rx) = unbounded::<Vec<f32>>();
    let (stream, rate) = match mic::open(tx) {
        Ok(s) => s,
        Err(e) => {
            let _ = heard.send(Heard::Trouble { id, why: format!("{e}") });
            return None;
        }
    };
    open.store(true, Ordering::Relaxed);
    // What arrived while the mic was opening (it can wait on the player
    // being asked) is old news, except the last word.
    let mut cancelled = false;
    let mut again = None;
    for c in cmds.try_iter() {
        match c {
            Cmd::Cancel => cancelled = true,
            Cmd::Finish => {}
            l @ Cmd::Listen { .. } => again = Some(l),
        }
    }
    if again.is_some() || cancelled {
        return again;
    }
    let mut rs = mic::Resample::new(rate);
    let mut fresh: Vec<f32> = Vec::new();
    let mut audio: Vec<f32> = Vec::new();
    let mut pause = Pause::default();
    let mut last_pass = Instant::now();
    let mut passed_len = 0;
    let mut said = String::new();
    let mut finish = false;
    let mut silent = true;
    let started = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        for c in cmds.try_iter() {
            match c {
                Cmd::Cancel => return None,
                Cmd::Finish => finish = true,
                l @ Cmd::Listen { .. } => return Some(l),
            }
        }
        if let Ok(b) = rx.recv_timeout(Duration::from_millis(30)) {
            rs.push(&b, &mut fresh);
        }
        for b in rx.try_iter() {
            rs.push(&b, &mut fresh);
        }
        let mut step = pause.step;
        let whole = fresh.len() / FRAME * FRAME;
        for f in fresh[..whole].chunks(FRAME) {
            let l = pause::level(f);
            silent &= l == 0.0;
            level.store((l * 4.0).min(1.0).to_bits(), Ordering::Relaxed);
            step = pause.frame(l);
        }
        audio.extend(fresh.drain(..whole));
        // A mic the system keeps closed reads as pure zeros.
        if silent && started.elapsed() > Duration::from_millis(1500) {
            drop(stream);
            let _ = heard.send(Heard::Trouble { id, why: "the microphone hears nothing at all (on a Mac: System Settings → Privacy & Security → Microphone, allow the app you started the game from)".into() });
            return None;
        }
        let from = pause.began().map(|b| (((b - BEFORE).max(0.0)) * RATE as f32) as usize).unwrap_or(audio.len()).min(audio.len());
        let speech = &audio[from..];
        if finish || matches!(step, Step::Done | Step::Nothing) {
            drop(stream);
            let text = if speech.is_empty() || step == Step::Nothing {
                String::new()
            } else {
                rec.hear(speech, prompt, false).unwrap_or_else(|e| {
                    crate::log::info(&format!("ears: {e:#}"));
                    said.clone()
                })
            };
            let _ = heard.send(Heard::Done { id, text });
            return None;
        }
        if step == Step::Speech && last_pass.elapsed() >= LIVE_EVERY && audio.len() > passed_len {
            last_pass = Instant::now();
            passed_len = audio.len();
            match rec.hear(speech, prompt, true) {
                Ok(t) if t != said => {
                    said = t.clone();
                    let _ = heard.send(Heard::Words { id, text: t });
                }
                Ok(_) => {}
                Err(e) => crate::log::info(&format!("ears: {e:#}")),
            }
        }
    }
}
