//! Sound: made live from small recipes, placed where it happens and heard
//! from where the traveler stands and faces. No audio files anywhere.
//!
//! - `call`: what a sound is, as data (written by the LLM for species,
//!   guessed from words otherwise), and materials.
//! - `synth`: the four kinds of part and the endless textures.
//! - `mix`: placing voices around the head, the room, beds and chorus.
//! - `foley`: what the world sounds like each frame, read off the sim
//!   (footsteps, brushing, bumps, calls, wind, water, the night chorus).
//! - here: the sim's cues, the output device, and WAV writing.
//!
//! Sound is output only: the sim pushes cues (plain data) and never hears
//! back, so headless runs and their determinism are untouched.

pub mod call;
pub mod dsp;
pub mod foley;
pub mod listen;
pub mod mix;
pub mod synth;

use crate::sim::ActorId;
use call::{Call, Material};
use crossbeam_channel::{Receiver, Sender, bounded};
use glam::Vec3;
use mix::{Cmd, Mixer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Something the sim made heard. Kept small; turned into voices by `foley`.
#[derive(Clone, Debug)]
pub struct Cue {
    pub at: Vec3,
    /// Who made it, to follow as they move.
    pub from: Option<ActorId>,
    pub what: Heard,
}

#[derive(Clone, Debug)]
pub enum Heard {
    /// A being's or a thing's call; `mass` is the maker's (its register),
    /// `pitch` this one individual's offset.
    Call { call: Call, mass: f32, pitch: f32, gain: f32 },
    /// Something struck: what it is made of, how heavy, how fast it was met,
    /// and what met it (a body's material and mass), when not like itself.
    Hit { mat: Material, mass: f32, speed: f32, by: Option<(Material, f32)> },
}

/// Keys for moving emitters.
pub fn actor_key(a: ActorId) -> u64 {
    (1 << 48) | a.code() as u64
}

/// An individual's own pitch: a stable offset from its id, and its size
/// against its kind (bigger: lower).
pub fn individual_pitch(id: i64, size_ratio: f32) -> f32 {
    let h = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
    let jitter = 0.93 + 0.14 * (h % 1000) as f32 / 1000.0;
    jitter * size_ratio.clamp(0.2, 5.0).powf(-0.5)
}

/// The game's handle on the sound device. Dropping it stops the sound.
pub struct Output {
    tx: Sender<Cmd>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Output {
    pub fn send(&self, c: Cmd) {
        let _ = self.tx.try_send(c);
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Open the default output device on a thread of its own (it owns the
/// stream). `None` when there is no device, or sound is turned off by
/// `POCKET_NO_SOUND`.
pub fn start(volume: f32) -> Option<Output> {
    if std::env::var_os("POCKET_NO_SOUND").is_some() || cfg!(test) {
        return None;
    }
    let (tx, rx) = bounded::<Cmd>(4096);
    let stop = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = bounded::<bool>(1);
    let st = stop.clone();
    let thread = std::thread::Builder::new()
        .name("sound".into())
        .spawn(move || {
            let stream = match open(rx, volume) {
                Ok(s) => {
                    let _ = ready_tx.send(true);
                    s
                }
                Err(e) => {
                    crate::log::info(&format!("no sound: {e:#}"));
                    let _ = ready_tx.send(false);
                    return;
                }
            };
            while !st.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            drop(stream);
        })
        .ok()?;
    match ready_rx.recv_timeout(std::time::Duration::from_secs(3)) {
        Ok(true) => Some(Output { tx, stop, thread: Some(thread) }),
        _ => {
            stop.store(true, Ordering::Relaxed);
            None
        }
    }
}

fn open(rx: Receiver<Cmd>, volume: f32) -> anyhow::Result<cpal::Stream> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| anyhow::anyhow!("no output device"))?;
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let sr = config.sample_rate as f32;
    let channels = config.channels as usize;
    let mut mixer = Mixer::new(sr);
    mixer.apply(Cmd::Volume(volume));
    let mut scratch: Vec<f32> = Vec::new();
    let err = |e| crate::log::info(&format!("sound stream: {e}"));
    // One arm runs; each moves the mixer into its own callback.
    macro_rules! build {
        ($t:ty) => {
            device.build_output_stream(
                config.clone(),
                move |data: &mut [$t], _: &cpal::OutputCallbackInfo| {
                    while let Ok(c) = rx.try_recv() {
                        mixer.apply(c);
                    }
                    let ch = channels.max(1);
                    let frames = data.len() / ch;
                    if scratch.len() < frames * 2 {
                        scratch.resize(frames * 2, 0.0);
                    }
                    let s = &mut scratch[..frames * 2];
                    mixer.process(s);
                    for (f, out) in data.chunks_mut(ch).enumerate() {
                        for (c, x) in out.iter_mut().enumerate() {
                            let v = if ch == 1 { (s[2 * f] + s[2 * f + 1]) * 0.5 } else if c < 2 { s[2 * f + c] } else { 0.0 };
                            *x = <$t as cpal::FromSample<f32>>::from_sample_(v);
                        }
                    }
                },
                err,
                None,
            )?
        };
    }
    let stream = match format {
        cpal::SampleFormat::F32 => build!(f32),
        cpal::SampleFormat::I16 => build!(i16),
        cpal::SampleFormat::U16 => build!(u16),
        cpal::SampleFormat::I32 => build!(i32),
        cpal::SampleFormat::F64 => build!(f64),
        f => anyhow::bail!("unsupported sample format {f}"),
    };
    stream.play()?;
    Ok(stream)
}

/// Write interleaved stereo as a 16-bit WAV.
pub fn write_wav(path: &std::path::Path, sr: u32, st: &[f32]) -> std::io::Result<()> {
    let mut b = Vec::with_capacity(44 + st.len() * 2);
    let data = (st.len() * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&sr.to_le_bytes());
    b.extend_from_slice(&(sr * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for x in st {
        b.extend_from_slice(&((x.clamp(-1.0, 1.0) * 32000.0) as i16).to_le_bytes());
    }
    std::fs::write(path, b)
}
