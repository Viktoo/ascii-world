//! A character's body, given to a plain voice: the WORLD vocoder splits the
//! speech into pitch, the throat's resonances and breath, and each is
//! reshaped (pitch and its range, throat size, breath, rasp, an old voice's
//! tremor) before it is put back together. The words stay the voice model's.

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
    let nf0: Vec<f64> = (0..n).map(|i| if voiced[i] { nl[i].exp() } else { 0.0 }).collect();

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
    for i in 0..n {
        let shimmer = if r.rasp > 0.0 { (0.25 * r.rasp as f64 * rng.gauss()).exp() } else { 1.0 };
        for k in 0..bins {
            sp[i][k] *= tilt[k] * shimmer;
            let mut a = ap[i][k];
            a += (1.0 - a) * r.breath as f64 * 0.8 * wgt[k];
            if r.rasp > 0.0 {
                a += (1.0 - a) * r.rasp as f64 * 0.5 * band[k];
            }
            ap[i][k] = a.clamp(0.0, 0.999);
        }
    }
    let y = rsworld::synthesis(&nf0, &sp, &ap, FP, fs_i);
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

    #[test]
    fn warp_moves_a_peak_up() {
        let mut row = vec![0.0; 100];
        row[20] = 1.0;
        let w = warp(&row, 1.5);
        let peak = w.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|p| p.0).unwrap();
        assert_eq!(peak, 30);
    }
}
