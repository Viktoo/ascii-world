//! The microphone: the default input device, mono, brought to 16 kHz.

use super::pause::RATE;
use anyhow::{Result, anyhow, bail};
use crossbeam_channel::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Down to the recogniser's rate: each output sample is the mean of the
/// input that falls in its span (which also keeps out what is too high to
/// be heard at 16 kHz).
pub struct Resample {
    step: f64,
    /// Where the next output sample's span ends, in input samples.
    next: f64,
    /// Input samples seen.
    seen: f64,
    sum: f32,
    n: u32,
}

impl Resample {
    pub fn new(from: u32) -> Resample {
        let step = from as f64 / RATE as f64;
        Resample { step, next: step, seen: 0.0, sum: 0.0, n: 0 }
    }

    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        for &x in input {
            self.sum += x;
            self.n += 1;
            self.seen += 1.0;
            if self.seen >= self.next {
                out.push(self.sum / self.n as f32);
                self.sum = 0.0;
                self.n = 0;
                self.next += self.step;
            }
        }
    }
}

/// The open mic: hearing until dropped (on the thread that opened it).
pub enum Mic {
    Device(#[allow(dead_code)] cpal::Stream),
    /// `POCKET_MIC_WAV`: a file played in as if said, in real time.
    File(Arc<AtomicBool>),
}

impl Drop for Mic {
    fn drop(&mut self) {
        if let Mic::File(stop) = self {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Open the default input device (or `POCKET_MIC_WAV`'s file); mono blocks
/// at its own rate go to `tx`.
pub fn open(tx: Sender<Vec<f32>>) -> Result<(Mic, u32)> {
    if let Some(f) = std::env::var_os("POCKET_MIC_WAV") {
        let pcm = super::bench::read_wav(std::path::Path::new(&f))?;
        let stop = Arc::new(AtomicBool::new(false));
        let st = stop.clone();
        std::thread::spawn(move || {
            // Then the room, quiet.
            let quiet = vec![0.0005f32; RATE * 10];
            for b in pcm.chunks(RATE / 100).chain(quiet.chunks(RATE / 100)) {
                if st.load(Ordering::Relaxed) || tx.send(b.to_vec()).is_err() {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        return Ok((Mic::File(stop), RATE as u32));
    }
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or_else(|| anyhow!("no microphone"))?;
    let supported = device.default_input_config()?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let rate = config.sample_rate;
    let ch = (config.channels as usize).max(1);
    let err = |e| crate::log::info(&format!("mic stream: {e}"));
    macro_rules! build {
        ($t:ty) => {
            device.build_input_stream(
                config.clone(),
                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                    let mono: Vec<f32> = data.chunks(ch).map(|f| f.iter().map(|s| <f32 as cpal::FromSample<$t>>::from_sample_(*s)).sum::<f32>() / ch as f32).collect();
                    let _ = tx.send(mono);
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
        f => bail!("unsupported mic sample format {f}"),
    };
    stream.play()?;
    Ok((Mic::Device(stream), rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brings_48k_and_44k1_to_16k() {
        for from in [48_000u32, 44_100, 16_000] {
            let mut r = Resample::new(from);
            let mut out = Vec::new();
            // A second in uneven blocks, a 200 Hz tone.
            let tone: Vec<f32> = (0..from).map(|i| (i as f32 / from as f32 * 200.0 * std::f32::consts::TAU).sin()).collect();
            for block in tone.chunks(441) {
                r.push(block, &mut out);
            }
            assert!((out.len() as i64 - RATE as i64).abs() <= 1, "{from}: {}", out.len());
            // The tone comes through (a low one barely loses anything).
            let rms = (out.iter().map(|x| x * x).sum::<f32>() / out.len() as f32).sqrt();
            assert!((0.68..0.72).contains(&rms), "{from}: {rms}");
        }
    }
}
