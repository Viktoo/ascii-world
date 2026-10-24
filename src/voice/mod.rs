//! Spoken voices: what characters say, heard in their own voice from where
//! they stand. All on this machine, nothing sent anywhere:
//!
//! - `recipe`: a character's voice read off their persona (sex, age, height,
//!   how they speak), with a blend of model voices of their own.
//! - `kokoro`: the voice model (text's phonemes to speech).
//! - `body`: the character's body given to that speech (pitch, throat size,
//!   breath, rasp, tremor), through the WORLD vocoder.
//! - `names`: reading out words no dictionary knows (the world's names).
//! - `pack`: the model files, fetched once.
//!
//! Like sound, it is output only: the sim never waits on it. Lines are made
//! on a thread of their own and played through the mixer as placed sounds.

pub mod body;
pub mod kokoro;
pub mod names;
pub mod pack;
pub mod recipe;
pub mod speakable;

use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender, unbounded};
use misaki_rs::{G2P, Language};
use parking_lot::Mutex;
use recipe::Recipe;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Lines waiting longer than this are dropped (the moment has passed).
const STALE: Duration = Duration::from_secs(6);

/// How long a line takes to say, at a pace (1 = ordinary).
pub fn secs(text: &str, pace: f32) -> f64 {
    let chars = text.chars().filter(|c| !c.is_whitespace()).count() as f64;
    let stops = text.chars().filter(|c| ".!?;".contains(*c)).count() as f64;
    (0.3 + chars / 13.5 + 0.25 * stops) / pace.clamp(0.5, 2.0) as f64
}

/// Text cut into sentences (short ones kept with the next).
pub fn sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        cur.push(*c);
        let end = ".!?…".contains(*c) && chars.get(i + 1).is_none_or(|n| n.is_whitespace());
        if end && cur.trim().chars().count() >= 12 {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        match out.last_mut() {
            Some(l) if cur.trim().chars().count() < 12 => {
                l.push(' ');
                l.push_str(cur.trim());
            }
            _ => out.push(cur.trim().to_string()),
        }
    }
    out
}

/// Threads for the voice model: a share of the machine, leaving the game room.
pub fn threads() -> usize {
    std::thread::available_parallelism().map(|n| (n.get() / 4).clamp(1, 4)).unwrap_or(2)
}

/// The model and the words-to-phonemes readers, ready to speak.
pub struct Engine {
    kokoro: kokoro::Kokoro,
    us: G2P,
    gb: G2P,
}

impl Engine {
    pub fn new(threads: usize) -> Result<Engine> {
        let kokoro = kokoro::Kokoro::load(&pack::path(pack::FILES[0].name), &pack::path(pack::FILES[1].name), threads)?;
        let us = G2P::with_fallback(Language::EnglishUS, Some(Box::new(names::SoundOut)));
        let gb = G2P::with_fallback(Language::EnglishGB, Some(Box::new(names::SoundOut)));
        Ok(Engine { kokoro, us, gb })
    }

    /// The line in the character's voice: mono at `kokoro::SAMPLE_RATE`.
    pub fn speak(&mut self, text: &str, r: &Recipe) -> Result<Vec<f32>> {
        let raw = self.plain(text, r)?;
        Ok(body::reshape(&raw, kokoro::SAMPLE_RATE, r))
    }

    /// The line in the blended model voice, before the body.
    pub fn plain(&mut self, text: &str, r: &Recipe) -> Result<Vec<f32>> {
        let g2p = if r.british { &self.gb } else { &self.us };
        let (ph, _) = g2p.g2p(&speakable::text(text)).map_err(|e| anyhow!("{e}"))?;
        let ph = speakable::phonemes(&ph.replace('❓', ""), r.british);
        self.kokoro.speak(&ph, &r.blend, r.speed)
    }
}

pub struct Job {
    /// Who speaks (a character's id).
    pub who: i64,
    pub text: String,
    pub recipe: Arc<Recipe>,
    /// Said to the player: never dropped, made first.
    pub urgent: bool,
    pub asked: Instant,
}

pub struct Clip {
    pub who: i64,
    /// The words (for the log).
    pub text: String,
    pub samples: Arc<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Starting,
    Ready,
    Failed(String),
}

/// The game's handle on the voice thread.
pub struct Speaker {
    tx: Option<Sender<Job>>,
    rx: Receiver<Clip>,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
}

impl Speaker {
    /// Start the voice thread (fetching the pack first, if it must).
    pub fn start(threads: usize) -> Speaker {
        let (tx, jobs) = unbounded::<Job>();
        let (done, rx) = unbounded::<Clip>();
        let state = Arc::new(Mutex::new(State::Starting));
        let stop = Arc::new(AtomicBool::new(false));
        let (st, sp) = (state.clone(), stop.clone());
        let _ = std::thread::Builder::new().name("voice".into()).spawn(move || run(jobs, done, st, sp, threads));
        Speaker { tx: Some(tx), rx, state, stop }
    }

    pub fn state(&self) -> State {
        self.state.lock().clone()
    }

    pub fn say(&self, job: Job) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(job);
        }
    }

    pub fn poll(&self) -> Vec<Clip> {
        self.rx.try_iter().collect()
    }
}

impl Drop for Speaker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.tx = None;
    }
}

fn run(jobs: Receiver<Job>, done: Sender<Clip>, state: Arc<Mutex<State>>, stop: Arc<AtomicBool>, threads: usize) {
    // Fetched only when the player asks, in settings.
    if !pack::ready() {
        *state.lock() = State::Failed("the voices aren't downloaded (Settings → Sound)".into());
        return;
    }
    if stop.load(Ordering::Relaxed) {
        return;
    }
    let mut engine = match Engine::new(threads) {
        Ok(e) => e,
        Err(e) => {
            crate::log::info(&format!("voice: {e:#}"));
            *state.lock() = State::Failed(format!("{e}"));
            return;
        }
    };
    *state.lock() = State::Ready;
    loop {
        let first = match jobs.recv_timeout(Duration::from_millis(250)) {
            Ok(j) => j,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                continue;
            }
            Err(_) => return,
        };
        let mut queue: Vec<Job> = std::iter::once(first).chain(jobs.try_iter()).collect();
        // What was said to the player comes first; stale chatter is let go.
        queue.retain(|j| j.urgent || j.asked.elapsed() < STALE);
        queue.sort_by_key(|j| !j.urgent);
        for j in queue {
            // A sentence at a time: the first is heard while the rest is made.
            for part in sentences(&j.text) {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                match engine.speak(&part, &j.recipe) {
                    Ok(s) if !s.is_empty() => {
                        let _ = done.send(Clip { who: j.who, text: part.clone(), samples: Arc::new(s) });
                    }
                    Ok(_) => {}
                    Err(e) => crate::log::info(&format!("voice line: {e:#}")),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_sentences_keeping_short_ones_together() {
        assert_eq!(sentences("Morning! Did you see the fox? It took two hens."), vec!["Morning! Did you see the fox?", "It took two hens."]);
        assert_eq!(sentences("Aye."), vec!["Aye."]);
        assert_eq!(sentences("It is 3.5 miles off, and that is that. Go."), vec!["It is 3.5 miles off, and that is that. Go."]);
    }

    #[test]
    fn longer_and_slower_lines_take_longer() {
        assert!(secs("Morning!", 1.0) < secs("Did you see the fox down by the old mill?", 1.0));
        assert!(secs("Quiet day.", 0.8) > secs("Quiet day.", 1.2));
        let s = secs("Did you see the fox by the old mill?", 1.0);
        assert!((2.0..3.5).contains(&s), "{s}");
    }
}

