//! When the player has finished speaking: loudness against the room's own
//! noise, 20 ms at a time. Speech starts on a few loud frames in a row and
//! ends on a pause; the gaps between words are far shorter than that.

use std::collections::VecDeque;

/// Samples a second (what the recogniser takes).
pub const RATE: usize = 16_000;
/// One frame: 20 ms.
pub const FRAME: usize = RATE / 50;
const FRAME_SECS: f32 = FRAME as f32 / RATE as f32;

/// Silence after speech that ends it.
pub const PAUSE: f32 = 1.0;
/// Speech this long, at least, before it counts (a cough or a click doesn't).
const ONSET: f32 = 0.12;
/// Nothing said for this long: give up listening.
pub const NOTHING: f32 = 8.0;
/// The most one go takes (the recogniser hears 30 s at once).
pub const LONGEST: f32 = 28.0;
/// The room's noise is the quietest frame of the last this many seconds
/// (a fan or the rain is in every frame; words have gaps).
const FLOOR_SECS: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing said yet.
    Waiting,
    /// Speaking (or between words).
    Speech,
    /// Said, then paused: it's over.
    Done,
    /// Nothing was said.
    Nothing,
}

pub struct Pause {
    recent: VecDeque<f32>,
    /// Loud frames in a row, before speech has begun.
    run: f32,
    /// Speech began this many seconds in (less the onset), once it has.
    began: Option<f32>,
    quiet: f32,
    t: f32,
    pub step: Step,
}

impl Default for Pause {
    fn default() -> Self {
        Pause { recent: VecDeque::new(), run: 0.0, began: None, quiet: 0.0, t: 0.0, step: Step::Waiting }
    }
}

/// A frame's loudness (RMS).
pub fn level(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    (frame.iter().map(|x| x * x).sum::<f32>() / frame.len() as f32).sqrt()
}

impl Pause {
    /// Where speech began, in seconds from the start (once it has).
    pub fn began(&self) -> Option<f32> {
        self.began
    }

    /// The room's noise now.
    pub fn floor(&self) -> f32 {
        self.recent.iter().copied().fold(f32::MAX, f32::min).min(1.0)
    }

    /// One frame's loudness in; where things stand after it.
    pub fn frame(&mut self, rms: f32) -> Step {
        if matches!(self.step, Step::Done | Step::Nothing) {
            return self.step;
        }
        self.t += FRAME_SECS;
        let floor = if self.recent.is_empty() { rms } else { self.floor() };
        self.recent.push_back(rms);
        while self.recent.len() as f32 * FRAME_SECS > FLOOR_SECS {
            self.recent.pop_front();
        }
        // Harder to start than to keep going.
        let loud = match self.began {
            None => rms > floor * 2.5 + 0.004,
            Some(_) => rms > floor * 2.0 + 0.003,
        };
        match self.began {
            None => {
                self.run = if loud { self.run + FRAME_SECS } else { 0.0 };
                if self.run >= ONSET {
                    self.began = Some((self.t - self.run).max(0.0));
                    self.step = Step::Speech;
                } else if self.t >= NOTHING {
                    self.step = Step::Nothing;
                }
            }
            Some(b) => {
                self.quiet = if loud { 0.0 } else { self.quiet + FRAME_SECS };
                if self.quiet >= PAUSE || self.t - b >= LONGEST {
                    self.step = Step::Done;
                }
            }
        }
        self.step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed `secs` of frames at loudness `rms`; the last step.
    fn feed(p: &mut Pause, secs: f32, rms: f32) -> Step {
        let mut s = p.step;
        for _ in 0..(secs / FRAME_SECS).round() as usize {
            s = p.frame(rms);
        }
        s
    }

    /// Words: loud syllables with short quiet gaps between them.
    fn words(p: &mut Pause, n: usize, noise: f32) -> Step {
        let mut s = p.step;
        for _ in 0..n {
            feed(p, 0.24, 0.08 + noise * 2.0);
            s = feed(p, 0.16, noise);
        }
        s
    }

    #[test]
    fn a_pause_after_speech_ends_it() {
        let mut p = Pause::default();
        assert_eq!(feed(&mut p, 0.5, 0.002), Step::Waiting);
        assert_eq!(words(&mut p, 6, 0.002), Step::Speech, "the gaps between words don't end it");
        let began = p.began().unwrap();
        assert!((0.45..0.55).contains(&began), "{began}");
        assert_eq!(feed(&mut p, 0.6, 0.002), Step::Speech, "nor does a short breath");
        assert_eq!(feed(&mut p, 0.5, 0.002), Step::Done);
        assert_eq!(p.frame(0.1), Step::Done, "done stays done");
    }

    #[test]
    fn a_click_is_not_speech_and_silence_gives_up() {
        let mut p = Pause::default();
        feed(&mut p, 0.3, 0.002);
        assert_eq!(feed(&mut p, 0.04, 0.2), Step::Waiting, "a click");
        assert_eq!(feed(&mut p, 2.0, 0.002), Step::Waiting);
        assert_eq!(feed(&mut p, NOTHING, 0.002), Step::Nothing);
    }

    #[test]
    fn a_steady_noise_becomes_the_floor() {
        let mut p = Pause::default();
        // A fan starts: it reads as speech at first...
        feed(&mut p, 0.5, 0.001);
        feed(&mut p, 0.3, 0.03);
        // ...and then as the room, so it ends like a pause.
        assert_eq!(feed(&mut p, 3.0, 0.03), Step::Done);
        // Speaking over the fan still counts.
        let mut p = Pause::default();
        feed(&mut p, 2.0, 0.03);
        assert_eq!(p.step, Step::Waiting);
        assert_eq!(words(&mut p, 4, 0.03), Step::Speech);
        assert_eq!(feed(&mut p, 1.2, 0.03), Step::Done);
    }

    #[test]
    fn talking_on_and_on_is_cut_at_the_longest() {
        let mut p = Pause::default();
        let mut s = Step::Waiting;
        let mut secs = 0.0;
        while s != Step::Done && secs < 60.0 {
            s = words(&mut p, 1, 0.002);
            secs += 0.4;
        }
        assert_eq!(s, Step::Done);
        assert!((LONGEST..LONGEST + 1.0).contains(&secs), "{secs}");
    }
}
