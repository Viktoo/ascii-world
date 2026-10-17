//! A character's body, given to a plain voice: the WORLD vocoder splits the
//! speech into pitch, the throat's resonances and breath, and each is
//! reshaped (pitch and its range, throat size, breath, rasp, an old voice's
//! tremor) before it is put back together; it can be whispered (breath
//! alone, no voice), sound hollow (as down a tube), and carry a second voice
//! an octave under it. The words stay the voice model's.

use super::recipe::Recipe;

const FP: f64 = 5.0; // ms per frame

/// Small seeded noise (no allocation-heavy RNG on the voice thread).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn gauss(&mut self) -> f64 {
        let (a, b) = (self.next().max(1e-12), self.next());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

/// Slow random wander: a point every `step` frames, joined straight.
fn wander(n: usize, step: usize, rng: &mut Rng) -> Vec<f64> {
    let pts: Vec<f64> = (0..n / step + 2).map(|_| rng.gauss()).collect();
    (0..n)
        .map(|i| {
            let x = i as f64 / step as f64;
            let k = x.floor() as usize;
            let f = x - k as f64;
            pts[k] * (1.0 - f) + pts[k + 1] * f
        })
        .collect()
}

/// Resample one frame along frequency: formants move by `alpha`.
fn warp(row: &[f64], alpha: f64) -> Vec<f64> {
    let n = row.len();
    (0..n)
        .map(|k| {
            let src = (k as f64 / alpha).min((n - 1) as f64);
            let i = src.floor() as usize;
            let f = src - i as f64;
            let j = (i + 1).min(n - 1);
            row[i] * (1.0 - f) + row[j] * f
        })
        .collect()
}

/// Reshape mono speech at `fs` into the character's own voice.
pub fn reshape(x: &[f32], fs: u32, r: &Recipe) -> Vec<f32> {
    if x.len() < fs as usize / 10 {
        return x.to_vec();
    }
    let fs_i = fs as i32;
    let x: Vec<f64> = x.iter().map(|v| *v as f64).collect();
    let (t, f0) = if std::env::var_os("POCKET_VOICE_HARVEST").is_some() {
        let mut opt = rsworld_sys::HarvestOption::new();
        opt.f0_floor = 60.0;
        opt.f0_ceil = 500.0;
        opt.frame_period = FP;
        rsworld::harvest(&x, fs_i, &opt)
    } else {
        let mut opt = rsworld_sys::DioOption::new();
        opt.f0_floor = 60.0;
        opt.f0_ceil = 500.0;
        opt.frame_period = FP;
        let (t, f0) = rsworld::dio(&x, fs_i, &opt);
        let f0 = rsworld::stonemask(&x, fs_i, &t, &f0);
        (t, f0)
    };
    let mut ct = rsworld_sys::CheapTrickOption::new(fs_i);
    let mut sp = rsworld::cheaptrick(&x, fs_i, &t, &f0, &mut ct);
    let mut ap = rsworld::d4c(&x, fs_i, &t, &f0, &rsworld_sys::D4COption::new());
    let n = f0.len();
    if n == 0 || sp.is_empty() {
        return x.iter().map(|v| *v as f32).collect();
    }
    let bins = sp[0].len();
    let freq: Vec<f64> = (0..bins).map(|k| k as f64 * fs as f64 / 2.0 / (bins - 1) as f64).collect();
    let voiced: Vec<bool> = f0.iter().map(|v| *v > 0.0).collect();
    let mut rng = Rng(r.seed | 1);

    // Pitch: the character's own mean and range, rough and wandering, an
    // old voice's tremor, and a creak where a phrase dies away.
    let logs: Vec<f64> = f0.iter().filter(|v| **v > 0.0).map(|v| v.ln()).collect();
    let mean = if logs.is_empty() { (r.f0 as f64).ln() } else { logs.iter().sum::<f64>() / logs.len() as f64 };
    let slow = wander(n, 30, &mut rng);
    let phase = rng.next() * 6.0;
    let jit = r.jitter as f64;
    let mut nl: Vec<f64> = (0..n)
        .map(|i| {
            let tt = i as f64 * FP / 1000.0;
            let l = if f0[i] > 0.0 { f0[i].ln() } else { mean };
            (r.f0 as f64).ln() + r.range as f64 * (l - mean) + 0.012 * jit * rng.gauss() + 0.03 * jit * slow[i] + 0.045 * r.tremor as f64 * (std::f64::consts::TAU * r.trem_hz as f64 * tt + phase).sin()
        })
        .collect();
    if r.rasp > 0.0 {
        for e in 0..n.saturating_sub(1) {
            if voiced[e] && !voiced[e + 1] {
                let s = e.saturating_sub(14);
                let len = (e - s + 1) as f64;
                for (j, k) in (s..=e).enumerate() {
                    nl[k] -= r.rasp as f64 * 0.35 * j as f64 / len.max(1.0);
                }
            }
        }
    }
    // A whisper has no voice at all, only breath through the same mouth.
    let whisper = r.whisper as f64;
    let nf0: Vec<f64> = (0..n).map(|i| if voiced[i] && whisper < 0.75 { nl[i].exp() } else { 0.0 }).collect();

    // Throat: formants move, the pitch doesn't.
    let alpha = (r.throat / r.base_throat()) as f64;
    if (alpha - 1.0).abs() > 1e-3 {
        for row in sp.iter_mut() {
            *row = warp(row, alpha);
        }
        for row in ap.iter_mut() {
            *row = warp(row, alpha);
        }
    }
    // Brightness (capped above 5 kHz: no harsh hiss), breath, rasp.
    let tilt: Vec<f64> = freq.iter().map(|f| (1.0 + f.min(5000.0) / 1500.0).powf(0.9 * r.tilt as f64).powi(2)).collect();
    let wgt: Vec<f64> = freq.iter().map(|f| (f / 3000.0).clamp(0.15, 1.0)).collect();
    let band: Vec<f64> = freq.iter().map(|f| (-((f - 1200.0) / 900.0).powi(2)).exp()).collect();
    // Hollow: resonances every 550 Hz, as down a pipe or in a barrel chest.
    let hollow = r.hollow as f64;
    let comb: Vec<f64> = freq.iter().map(|f| 1.0 + hollow * 0.85 * (std::f64::consts::TAU * f / 550.0).cos()).collect();
    for i in 0..n {
        let shimmer = if r.rasp > 0.0 { (0.25 * r.rasp as f64 * rng.gauss()).exp() } else { 1.0 };
        for k in 0..bins {
            sp[i][k] *= tilt[k] * shimmer * comb[k];
            let mut a = ap[i][k];
            a += (1.0 - a) * r.breath as f64 * 0.8 * wgt[k];
            a += (1.0 - a) * whisper;
            if r.rasp > 0.0 {
                a += (1.0 - a) * r.rasp as f64 * 0.5 * band[k];
            }
            ap[i][k] = a.clamp(0.0, 0.999);
        }
    }
    let mut y = rsworld::synthesis(&nf0, &sp, &ap, FP, fs_i);
    // A voice under the voice: an octave down, from a bigger throat.
    if r.under > 0.0 && nf0.iter().any(|v| *v > 0.0) {
        let low: Vec<f64> = nf0.iter().map(|v| v * 0.5).collect();
        let big: Vec<Vec<f64>> = sp.iter().map(|row| warp(row, 0.85)).collect();
        let ab: Vec<Vec<f64>> = ap.iter().map(|row| warp(row, 0.85)).collect();
        let y2 = rsworld::synthesis(&low, &big, &ab, FP, fs_i);
        let g = r.under as f64 * 0.9;
        for (a, b) in y.iter_mut().zip(y2.iter()) {
            *a += g * b;
        }
    }
    // And the hollow's echo: a short ring inside it.
    if hollow > 0.0 {
        let d = (fs as f64 * 0.009) as usize;
        for i in (d..y.len()).rev() {
            y[i] += hollow * 0.45 * y[i - d];
        }
    }
    level(&y)
}

/// Even loudness, soft-limited.
pub fn level(y: &[f64]) -> Vec<f32> {
    let rms = (y.iter().map(|v| v * v).sum::<f64>() / y.len().max(1) as f64).sqrt();
    let g = 0.12 / (rms + 1e-9);
    y.iter().map(|v| ((v * g * 1.2).tanh() / 1.2) as f32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain buzz at 120 Hz, a second long, like a held vowel.
    fn buzz(fs: u32) -> Vec<f32> {
        (0..fs as usize).map(|i| (1..12).map(|h| (std::f32::consts::TAU * 120.0 * h as f32 * i as f32 / fs as f32).sin() / h as f32).sum::<f32>() * 0.2).collect()
    }

    /// Energy below `hz` against all of it (a plain DFT over a few bins).
    fn low_share(y: &[f32], fs: u32, hz: f32) -> f32 {
        let n = y.len().min(8192);
        let mut lo = 0.0;
        let mut all = 0.0;
        for b in 1..(n / 8) {
            let f = b as f32 * fs as f32 / n as f32;
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (i, v) in y[..n].iter().enumerate() {
                let a = std::f32::consts::TAU * b as f32 * i as f32 / n as f32;
                re += v * a.cos();
                im += v * a.sin();
            }
            let e = re * re + im * im;
            all += e;
            if f < hz {
                lo += e;
            }
        }
        lo / all.max(1e-12)
    }

    /// A voice under the voice puts energy an octave below; a whisper and
    /// a hollow voice still come out whole and even.
    #[test]
    fn under_whisper_and_hollow_reshape_a_voice() {
        let fs = 24000;
        let x = buzz(fs);
        let p = crate::world::Persona { name: "Oda".into(), voice: "plain".into(), ..Default::default() };
        let plain = Recipe { f0: 120.0, range: 1.0, tremor: 0.0, jitter: 0.0, ..Recipe::of_plain(&p, 1) };
        let under = Recipe { under: 1.0, ..plain.clone() };
        let (a, b) = (reshape(&x, fs, &plain), reshape(&x, fs, &under));
        assert!(low_share(&b, fs, 90.0) > low_share(&a, fs, 90.0) * 2.0, "{} vs {}", low_share(&b, fs, 90.0), low_share(&a, fs, 90.0));
        for r in [Recipe { whisper: 1.0, ..plain.clone() }, Recipe { hollow: 1.0, ..plain.clone() }] {
            let y = reshape(&x, fs, &r);
            assert!(y.len() > x.len() / 2 && y.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        }
    }

    #[test]
    fn warp_moves_a_peak_up() {
        let mut row = vec![0.0; 100];
        row[20] = 1.0;
        let w = warp(&row, 1.5);
        let peak = w.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|p| p.0).unwrap();
        assert_eq!(peak, 30);
    }
}
