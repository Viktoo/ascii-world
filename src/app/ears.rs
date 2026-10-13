//! Speak to talk: `/`, or Enter with someone, opens the mic. The words show
//! in the input line as they are made out and are sent at the pause, like
//! typed ones. Typing takes over (the mic closes, the words stay to edit),
//! Enter sends at once, Esc forgets what was heard.

use super::{ACCENT, App, DIM, Mode};
use crate::ears::{Cmd, Ears, Heard, State, words};
use crate::sim::ActorId;
use std::time::Instant;

/// One go of listening.
pub(super) struct Hearing {
    id: u64,
    /// The input line before (`/`, or nothing).
    base: String,
    since: Instant,
}

impl App {
    /// The recogniser runs while the setting is on (the mic only opens to listen).
    pub(super) fn sync_ears(&mut self) {
        let want = self.settings.speak_to_talk && !cfg!(test);
        match (&self.ears, want) {
            (None, true) => {
                self.ears = Some(Ears::start());
                self.ears_told = None;
            }
            (Some(_), false) => {
                self.ears = None;
                self.hearing = None;
            }
            _ => {}
        }
    }

    pub(super) fn ears_status(&self) -> String {
        if !self.settings.speak_to_talk {
            return "off".into();
        }
        match self.ears.as_ref().map(|e| e.state()) {
            Some(State::Fetching(p)) => format!("on (fetching the speech model, {:.0}%)", p * 100.0),
            Some(State::Failed(e)) => format!("on (failed: {e})"),
            Some(State::Starting) => "on (starting)".into(),
            _ if self.ears_trouble.is_some() => format!("on ({})", self.ears_trouble.as_deref().unwrap_or_default()),
            _ => "on: / or Enter, then speak".into(),
        }
    }

    /// Open the mic for the input line (when the recogniser is ready).
    pub(super) fn start_hearing(&mut self) {
        let Some(e) = &self.ears else { return };
        if e.state() != State::Ready {
            return;
        }
        self.hear_id += 1;
        let id = self.hear_id;
        e.send(Cmd::Listen { id, prompt: self.hear_prompt() });
        self.hearing = Some(Hearing { id, base: self.input.clone(), since: Instant::now() });
        self.dirty = true;
    }

    /// Close the mic, forgetting what it is still making out.
    pub(super) fn stop_hearing(&mut self) {
        if self.hearing.take().is_some() {
            if let Some(e) = &self.ears {
                e.send(Cmd::Cancel);
            }
            self.dirty = true;
        }
    }

    /// While listening: Esc forgets what was heard, Enter sends it now, and
    /// anything else is typing (the mic closes, the words stay). True when
    /// the key was used up.
    pub(super) fn hearing_key(&mut self, code: crossterm::event::KeyCode) -> bool {
        use crossterm::event::KeyCode;
        let Some(h) = &self.hearing else { return false };
        match code {
            KeyCode::Esc => {
                let base = h.base.clone();
                self.stop_hearing();
                self.input = base;
                if self.mode == Mode::Command && self.input.trim() == "/" {
                    self.set_mode(Mode::Walk);
                }
                true
            }
            KeyCode::Enter => {
                if let Some(e) = &self.ears {
                    e.send(Cmd::Finish);
                }
                true
            }
            _ => {
                self.stop_hearing();
                false
            }
        }
    }

    /// Names likely to be said (the recogniser spells them right then).
    fn hear_prompt(&self) -> String {
        let mut names: Vec<String> = Vec::new();
        let mut add = |n: String| {
            if !n.is_empty() && !names.contains(&n) {
                names.push(n);
            }
        };
        if let Some(cid) = self.sim.talking_to {
            add(self.sim.actor_name(ActorId::Npc(cid)));
        }
        if let Some((_, n)) = &self.talk_hint {
            add(n.clone());
        }
        if let Some(p) = &self.pointed {
            add(p.name.clone());
        }
        if let Some(h) = self.sim.player.held {
            add(self.sim.thing_name(h));
        }
        if names.is_empty() { String::new() } else { format!("{}.", names.join(", ")) }
    }

    /// News from the recogniser, and what it heard.
    pub(super) fn ears_frame(&mut self) {
        let Some(e) = &self.ears else { return };
        let state = e.state();
        let phase = match state {
            State::Starting => 0,
            State::Fetching(_) => 1,
            State::Ready => 2,
            State::Failed(_) => 3,
        };
        if self.ears_told != Some(phase) {
            match &state {
                State::Fetching(_) => self.say(None, "Fetching the speech model for speaking to talk (148 MB, once)…", DIM),
                State::Ready if self.ears_told == Some(1) => self.say(None, "Speak to talk is ready: press / or Enter with someone, and speak.", DIM),
                State::Failed(e) => self.say(None, &format!("Speak to talk is off: {e}"), DIM),
                _ => {}
            }
            self.ears_told = Some(phase);
        }
        let Some(e) = &self.ears else { return };
        for h in e.poll() {
            let Some(cur) = &self.hearing else { continue };
            match h {
                Heard::Words { id, text } if id == cur.id => {
                    self.input = words::merge(&cur.base, &text);
                    self.dirty = true;
                }
                Heard::Done { id, text } if id == cur.id => {
                    let base = cur.base.clone();
                    self.hearing = None;
                    self.dirty = true;
                    if text.is_empty() {
                        self.input = base;
                        continue;
                    }
                    self.input = words::merge(&base, &text);
                    crate::log::info(&format!("ears: heard \"{text}\""));
                    self.submit_input();
                }
                Heard::Trouble { id, why } if id == cur.id => {
                    self.hearing = None;
                    self.dirty = true;
                    if self.ears_trouble.as_deref() != Some(why.as_str()) {
                        self.say(None, &format!("Can't listen: {why}. Type instead."), DIM);
                        self.ears_trouble = Some(why);
                    }
                }
                _ => {}
            }
        }
    }

    /// The mark at the end of the input line: listening (with the mic's
    /// level), or why it isn't yet. `None` when there is nothing to show.
    pub(super) fn hearing_mark(&self) -> Option<(String, [u8; 3])> {
        let e = self.ears.as_ref()?;
        if let Some(h) = &self.hearing {
            let lit = (e.level() * 1.5).min(1.0);
            let c = [DIM, ACCENT];
            let fg = std::array::from_fn(|i| (c[0][i] as f32 + (c[1][i] as f32 - c[0][i] as f32) * (0.35 + 0.65 * lit)) as u8);
            if !e.mic_open() && h.since.elapsed().as_secs_f32() > 1.5 {
                return Some(("waiting for the microphone (allow it if the system asks) · Esc".into(), DIM));
            }
            return Some(("● listening · pause to send · Esc cancel".to_string(), fg));
        }
        match e.state() {
            State::Fetching(p) => Some((format!("speech model {:.0}%", p * 100.0), DIM)),
            State::Starting => Some(("speech model loading".into(), DIM)),
            _ => None,
        }
    }
}
