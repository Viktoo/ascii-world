//! Deterministic hash and value-noise functions.
//!
//! Every function here has a twin in `shaders/common.wgsl`. The two must agree
//! (integer hashing is bit-exact; the float maths agrees to ~1e-6). Keep the
//! operation order identical when editing either side.

#[inline]
pub fn pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747796405).wrapping_add(2891336453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
    (word >> 22) ^ word
}

#[inline]
pub fn u2f(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16777216.0)
}

#[inline]
pub fn h2(ix: i32, iy: i32, s: u32) -> u32 {
    pcg(pcg((ix as u32).wrapping_add(s)).wrapping_add(iy as u32))
}

#[inline]
pub fn h3(ix: i32, iy: i32, iz: i32, s: u32) -> u32 {
    pcg(pcg(pcg((ix as u32).wrapping_add(s)).wrapping_add(iy as u32)).wrapping_add(iz as u32))
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Floats → i32 the way WGSL `i32(f32)` does for in-range values.
#[inline]
pub fn fi(x: f32) -> i32 {
    x as i32
}

/// 2D value noise in [0, 1).
pub fn vnoise2(x: f32, y: f32, s: u32) -> f32 {
    let fx = x.floor();
    let fy = y.floor();
    let ix = fi(fx);
    let iy = fi(fy);
    let ux = fade(x - fx);
    let uy = fade(y - fy);
    let a = u2f(h2(ix, iy, s));
    let b = u2f(h2(ix.wrapping_add(1), iy, s));
    let c = u2f(h2(ix, iy.wrapping_add(1), s));
    let d = u2f(h2(ix.wrapping_add(1), iy.wrapping_add(1), s));
    lerp(lerp(a, b, ux), lerp(c, d, ux), uy)
}

/// 3D value noise in [0, 1).
pub fn vnoise3(x: f32, y: f32, z: f32, s: u32) -> f32 {
    let fx = x.floor();
    let fy = y.floor();
    let fz = z.floor();
    let ix = fi(fx);
    let iy = fi(fy);
    let iz = fi(fz);
    let ux = fade(x - fx);
    let uy = fade(y - fy);
    let uz = fade(z - fz);
    let a = u2f(h3(ix, iy, iz, s));
    let b = u2f(h3(ix.wrapping_add(1), iy, iz, s));
    let c = u2f(h3(ix, iy.wrapping_add(1), iz, s));
    let d = u2f(h3(ix.wrapping_add(1), iy.wrapping_add(1), iz, s));
    let e = u2f(h3(ix, iy, iz.wrapping_add(1), s));
    let f = u2f(h3(ix.wrapping_add(1), iy, iz.wrapping_add(1), s));
    let g = u2f(h3(ix, iy.wrapping_add(1), iz.wrapping_add(1), s));
    let h = u2f(h3(ix.wrapping_add(1), iy.wrapping_add(1), iz.wrapping_add(1), s));
    let lo = lerp(lerp(a, b, ux), lerp(c, d, ux), uy);
    let hi = lerp(lerp(e, f, ux), lerp(g, h, ux), uy);
    lerp(lo, hi, uz)
}

/// Per-octave transforms `(a, b, c, d)`: octave coords are (a*x + b*y, c*x + d*y).
/// Precomputed so neither target accumulates rounding through the octaves;
/// the shader receives the same f32 constants (see `octave_wgsl`).
fn octave_mats(growth: f64) -> [[f32; 4]; 5] {
    let mut m = [[1.0f64, 0.0, 0.0, 1.0]; 5];
    for i in 1..5 {
        let p = m[i - 1];
        // R = [[1.6, 1.2], [-1.2, 1.6]] * growth;  M_i = R * M_{i-1}
        m[i] = [
            (1.6 * p[0] + 1.2 * p[2]) * growth,
            (1.6 * p[1] + 1.2 * p[3]) * growth,
            (-1.2 * p[0] + 1.6 * p[2]) * growth,
            (-1.2 * p[1] + 1.6 * p[3]) * growth,
        ];
    }
    m.map(|r| r.map(|v| v as f32))
}

pub static FBM_M: std::sync::LazyLock<[[f32; 4]; 5]> = std::sync::LazyLock::new(|| octave_mats(1.01));
pub static RIDGE_M: std::sync::LazyLock<[[f32; 4]; 5]> = std::sync::LazyLock::new(|| octave_mats(1.02));

/// WGSL declarations of the octave constants.
pub fn octave_wgsl() -> String {
    let fmt = |name: &str, m: &[[f32; 4]; 5]| {
        let rows: Vec<String> = m.iter().map(|r| format!("vec4f({:?}, {:?}, {:?}, {:?})", r[0], r[1], r[2], r[3])).collect();
        format!("const {name} = array<vec4f, 5>({});\n", rows.join(", "))
    };
    fmt("FBM_M", &FBM_M) + &fmt("RIDGE_M", &RIDGE_M)
}

/// Fractal value noise, normalised to [-1, 1].
pub fn fbm2(x: f32, y: f32, s: u32, octaves: u32) -> f32 {
    let mut sum = 0.0f32;
    let mut amp = 0.5f32;
    let mut norm = 0.0f32;
    for i in 0..octaves.min(5) {
        let m = FBM_M[i as usize];
        let px = m[0] * x + m[1] * y;
        let py = m[2] * x + m[3] * y;
        sum += amp * vnoise2(px, py, s.wrapping_add(i * 1013));
        norm += amp;
        amp *= 0.5;
    }
    (sum / norm) * 2.0 - 1.0
}

/// Ridged fractal noise in [0, 1].
pub fn ridged2(x: f32, y: f32, s: u32, octaves: u32) -> f32 {
    let mut sum = 0.0f32;
    let mut amp = 0.5f32;
    let mut norm = 0.0f32;
    for i in 0..octaves.min(5) {
        let m = RIDGE_M[i as usize];
        let px = m[0] * x + m[1] * y;
        let py = m[2] * x + m[3] * y;
        let n = 1.0 - (vnoise2(px, py, s.wrapping_add(i * 7919)) * 2.0 - 1.0).abs();
        sum += amp * n * n;
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

/// Bits of a float with -0 folded onto +0, so both targets hash it the same.
#[inline]
pub fn fbits(x: f32) -> u32 {
    if x == 0.0 { 0 } else { x.to_bits() }
}

/// The `hash(a, b, c)` API function: a float in [0, 1).
pub fn hash3f(a: f32, b: f32, c: f32) -> f32 {
    u2f(pcg(fbits(a).wrapping_add(pcg(fbits(b).wrapping_add(pcg(fbits(c)))))))
}

/// Seeded integer stream used for CPU-only placement (scatter, spawn).
pub fn hseq(seed: u32, a: i32, b: i32, c: u32) -> u32 {
    pcg(h2(a, b, seed).wrapping_add(c.wrapping_mul(0x9E37_79B9)))
}

pub fn hseqf(seed: u32, a: i32, b: i32, c: u32) -> f32 {
    u2f(hseq(seed, a, b, c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_ranges() {
        for i in 0..2000 {
            let x = i as f32 * 0.731 - 500.0;
            let y = i as f32 * -0.377 + 90.0;
            let n = vnoise2(x, y, 7);
            assert!((0.0..1.0).contains(&n));
            let f = fbm2(x * 0.01, y * 0.01, 3, 5);
            assert!((-1.0..=1.0).contains(&f));
            let r = ridged2(x * 0.01, y * 0.01, 3, 3);
            assert!((0.0..=1.0).contains(&r));
            let h = hash3f(x, y, 0.0);
            assert!((0.0..1.0).contains(&h));
        }
        assert_eq!(hash3f(0.0, 1.0, 2.0), hash3f(-0.0, 1.0, 2.0));
    }

    #[test]
    fn noise_is_continuous() {
        let a = vnoise2(10.999_99, 3.5, 1);
        let b = vnoise2(11.000_01, 3.5, 1);
        assert!((a - b).abs() < 1e-3);
    }
}
