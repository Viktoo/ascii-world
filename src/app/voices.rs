//! Characters heard speaking: lines near the traveler go to the voice
//! thread, and what it makes plays from the speaker, one sentence after
//! another (nobody talks over themself).

use super::App;
use crate::audio::{actor_key, mix::{At, Cmd}};
use crate::sim::ActorId;
use crate::voice::{Job, Speaker, State, kokoro::SAMPLE_RATE, recipe::Recipe};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Lines said farther off than this aren't voiced (the mixer would barely
/// carry them anyway).
const VOICE_RANGE: f32 = 30.0;
/// A breath between one sentence and the next.
const GAP: f32 = 0.18;

impl App {
    /// Voices on when they and sound are.
    pub(super) fn sync_voice(&mut self) {
        let want = self.settings.voices && self.settings.sound && self.sound.is_some() && !cfg!(test);
        match (&self.voice, want) {
            (None, true) => {
                self.voice = Some(Speaker::start(crate::voice::threads()));
                self.voice_told = None;
            }
            (Some(_), false) => {
                self.voice = None;
                self.voice_busy.clear();
                self.voice_said.clear();
            }
            _ => {}
        }
    }

    pub(super) fn voice_status(&self) -> String {
        if !self.settings.voices {
            return "off".into();
        }
        if !self.settings.sound {
            return "on (needs sound)".into();
        }
        match self.voice.as_ref().map(|v| v.state()) {
            None => "on".into(),
            Some(State::Starting) => "on (starting)".into(),
            Some(State::Fetching(p)) => format!("on (fetching the voices, {:.0}%)", p * 100.0),
            Some(State::Ready) => "on".into(),
            Some(State::Failed(e)) => format!("on (failed: {e})"),
        }
    }

    fn recipe(&mut self, cid: i64) -> Option<Arc<Recipe>> {
        if let Some(r) = self.voice_recipes.get(&cid) {
            return Some(r.clone());
        }
        let n = self.sim.cast.get(cid)?;
        let r = Arc::new(Recipe::of(&n.def.persona, cid));
        self.voice_recipes.insert(cid, r.clone());
        Some(r)
    }

    /// A character's line, to be heard (if they are near enough).
    pub(super) fn voice_line(&mut self, cid: i64, text: &str, urgent: bool) {
        if self.voice.is_none() || text.trim().is_empty() {
            return;
        }
        let Some(pos) = self.sim.actor(ActorId::Npc(cid)).map(|a| a.pos) else { return };
        if !urgent && self.sim.dist_to_player(pos) > VOICE_RANGE {
            return;
        }
        let Some(recipe) = self.recipe(cid) else { return };
        if let Some(v) = &self.voice {
            v.say(Job { who: cid, text: text.trim().to_string(), recipe, urgent, asked: Instant::now() });
        }
    }

    /// A reply streaming in: each whole sentence is spoken as it arrives.
    pub(super) fn voice_token(&mut self, cid: i64, text: &str) {
        if self.voice.is_none() {
            return;
        }
        let buf = self.voice_said.entry(cid).or_default();
        buf.push_str(text);
        if let Some(said) = take_spoken(buf) {
            self.voice_line(cid, &said, true);
        }
    }

    /// The reply ended: say what is left of it.
    pub(super) fn voice_reply_done(&mut self, cid: i64, ok: bool) {
        if let Some(rest) = self.voice_said.remove(&cid) {
            if ok {
                self.voice_line(cid, &rest, true);
            }
        }
    }

    /// Made lines to the mixer, each after the speaker's last one ends.
    pub(super) fn voice_frame(&mut self) {
        let Some(v) = &self.voice else { return };
        // Fetching and being ready are news, once each.
        let state = v.state();
        let phase = match state {
            State::Starting => 0,
            State::Fetching(_) => 1,
            State::Ready => 2,
            State::Failed(_) => 3,
        };
        if self.voice_told != Some(phase) {
            match &state {
                State::Fetching(_) => self.say(None, "Fetching the characters' voices (about 350 MB, once)…", super::DIM),
                State::Ready if self.voice_told == Some(1) => self.say(None, "Voices ready.", super::DIM),
                State::Failed(e) => self.say(None, &format!("Voices are off: {e}"), super::DIM),
                _ => {}
            }
            self.voice_told = Some(phase);
        }
        let Some(v) = &self.voice else { return };
        let clips = v.poll();
        let Some(out) = self.sound.as_ref() else { return };
        let now = Instant::now();
        for c in clips {
            let who = ActorId::Npc(c.who);
            let Some(a) = self.sim.actor(who) else { continue };
            let at = a.pos + glam::Vec3::Y * a.dims.height * 0.75;
            let busy = self.voice_busy.get(&c.who).copied().filter(|b| *b > now);
            let delay = busy.map(|b| (b - now).as_secs_f32() + GAP).unwrap_or(0.0);
            let secs = c.samples.len() as f32 / SAMPLE_RATE as f32;
            self.voice_busy.insert(c.who, now + Duration::from_secs_f32(delay + secs));
            crate::log::info(&format!("voice: {} ({:.0} m off, in {delay:.1}s for {secs:.1}s): {}", self.sim.actor_name(who), self.sim.dist_to_player(a.pos), c.text));
            out.send(Cmd::Speech { at: At::Follow(actor_key(who), at), samples: c.samples, rate: SAMPLE_RATE as f32, gain: self.settings.voice_volume, delay });
        }
    }
}

/// From a reply still streaming in, the whole sentences so far (left in
/// `buf`: what may still be growing).
fn take_spoken(buf: &mut String) -> Option<String> {
    let chars: Vec<char> = buf.chars().collect();
    let cut = (0..chars.len().saturating_sub(1)).rev().find(|&i| ".!?…".contains(chars[i]) && chars[i + 1].is_whitespace())?;
    let said: String = chars[..=cut].iter().collect();
    if said.trim().chars().count() < 12 {
        return None;
    }
    *buf = chars[cut + 1..].iter().collect();
    Some(said)
}

#[cfg(test)]
mod tests {
    use super::take_spoken;

    #[test]
    fn a_streaming_reply_is_spoken_a_sentence_at_a_time() {
        let mut buf = String::new();
        let mut said = Vec::new();
        for tok in ["Aye", ", the ice", " is thin", " today. Mind", " your step", " out there", "! And", " that"] {
            buf.push_str(tok);
            if let Some(s) = take_spoken(&mut buf) {
                said.push(s);
            }
        }
        assert_eq!(said, vec!["Aye, the ice is thin today.".to_string(), " Mind your step out there!".to_string()]);
        assert_eq!(buf, " And that");
        // Too short to be worth a breath: it waits for more.
        let mut b = "No. ".to_string();
        assert!(take_spoken(&mut b).is_none());
    }
}
