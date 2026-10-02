//! The allowlisted API: names, signatures, and the Rust implementations.
//!
//! Each function also exists in WGSL (`API_WGSL` below). The pair must agree;
//! `tests::gpu_parity` in `render::gpu` checks this on real hardware.

use crate::noise::{hash3f, vnoise3};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    /// f32 scalar
    F,
    /// boolean (comparisons, logic)
    B,
    /// vec3: colours from rgb()/hsv()/mix(), points from rotX/rotY/rotZ
    V,
}

impl Ty {
    pub fn name(self) -> &'static str {
        match self {
            Ty::F => "number",
            Ty::B => "boolean",
            Ty::V => "vec3 (colour/point)",
        }
    }
    pub fn width(self) -> u16 {
        if self == Ty::V { 3 } else { 1 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Api {
    Sphere,
    Box3,
    RoundBox,
    Cylinder,
    CappedCone,
    Capsule,
    CapsuleV,
    Torus,
    Plane,
    PlaneY,
    Ellipsoid,
    Union,
    SmoothUnion,
    Subtract,
    SmoothSubtract,
    Intersect,
    RotX,
    RotY,
    RotZ,
    Noise3,
    Hash,
    Rgb,
    Hsv,
    Mix,
    MixV,
    Abs,
    Min,
    Max,
    Clamp,
    Floor,
    Ceil,
    Round,
    Fract,
    Sin,
    Cos,
    Tan,
    Atan2,
    Sqrt,
    Pow,
    Exp,
    Sign,
    Step,
    Smoothstep,
    Length2,
    Length3,
    Mod,
}

/// Names the model may call. Variadic/overloaded names resolve in `resolve`.
pub const NAMES: &[&str] = &[
    "sphere", "box", "roundBox", "cylinder", "cappedCone", "capsule", "torus", "plane", "ellipsoid",
    "union", "smoothUnion", "subtract", "smoothSubtract", "intersect", "rotX", "rotY", "rotZ",
    "noise3", "hash", "rgb", "hsv", "mix", "abs", "min", "max", "clamp", "floor", "ceil", "round",
    "fract", "sin", "cos", "tan", "atan2", "sqrt", "pow", "exp", "sign", "step", "smoothstep",
    "length2", "length3", "mod",
];

pub fn is_api_name(n: &str) -> bool {
    NAMES.contains(&n)
}

/// How a call is lowered after resolution.
pub enum Resolved {
    Call(Api, Ty),
    /// Fold a variadic call into nested binary calls of this API.
    Fold(Api),
    /// hash(a) / hash(a, b): pad missing arguments with zeros.
    PadHash,
}

pub fn resolve(name: &str, args: &[Ty]) -> Result<Resolved, String> {
    use Api::*;
    use Ty::*;
    let n = args.len();
    let all_f = args.iter().all(|t| *t == F);
    let need = |api: Api, k: usize| -> Result<Resolved, String> {
        if n != k {
            return Err(format!("{name}() takes {k} arguments, got {n}"));
        }
        if !all_f {
            return Err(format!("{name}() arguments must all be numbers"));
        }
        Ok(Resolved::Call(api, ret_ty(api)))
    };
    match name {
        "sphere" => need(Sphere, 4),
        "box" => need(Box3, 6),
        "roundBox" => need(RoundBox, 7),
        "cylinder" => need(Cylinder, 5),
        "cappedCone" => need(CappedCone, 6),
        "capsule" if n == 5 => need(CapsuleV, 5),
        "capsule" => need(Capsule, 10).map_err(|_| {
            "capsule() takes (x,y,z,h,r) for a vertical capsule or (x,y,z,ax,ay,az,bx,by,bz,r)".into()
        }),
        "torus" => need(Torus, 5),
        "plane" if n == 4 => need(PlaneY, 4),
        "plane" => need(Plane, 7).map_err(|_| "plane() takes (x,y,z,h) or (x,y,z,nx,ny,nz,d)".into()),
        "ellipsoid" => need(Ellipsoid, 6),
        "smoothUnion" => need(SmoothUnion, 3),
        "subtract" => need(Subtract, 2),
        "smoothSubtract" => need(SmoothSubtract, 3),
        "union" | "intersect" | "min" | "max" => {
            if !(2..=8).contains(&n) {
                return Err(format!("{name}() takes 2 to 8 numbers, got {n}"));
            }
            if !all_f {
                return Err(format!("{name}() arguments must all be numbers"));
            }
            Ok(Resolved::Fold(match name {
                "union" => Union,
                "intersect" => Intersect,
                "min" => Min,
                _ => Max,
            }))
        }
        "rotX" => need(RotX, 4),
        "rotY" => need(RotY, 4),
        "rotZ" => need(RotZ, 4),
        "noise3" => need(Noise3, 3),
        "hash" => {
            if !(1..=3).contains(&n) || !all_f {
                return Err("hash() takes 1 to 3 numbers".into());
            }
            Ok(Resolved::PadHash)
        }
        "rgb" => need(Rgb, 3),
        "hsv" => need(Hsv, 3),
        "mix" => {
            if n != 3 {
                return Err(format!("mix() takes 3 arguments, got {n}"));
            }
            match (args[0], args[1], args[2]) {
                (F, F, F) => Ok(Resolved::Call(Mix, F)),
                (V, V, F) => Ok(Resolved::Call(MixV, V)),
                _ => Err("mix(a, b, t) needs a and b both numbers or both colours, and t a number".into()),
            }
        }
        "abs" => need(Abs, 1),
        "clamp" => need(Clamp, 3),
        "floor" => need(Floor, 1),
        "ceil" => need(Ceil, 1),
        "round" => need(Round, 1),
        "fract" => need(Fract, 1),
        "sin" => need(Sin, 1),
        "cos" => need(Cos, 1),
        "tan" => need(Tan, 1),
        "atan2" => need(Atan2, 2),
        "sqrt" => need(Sqrt, 1),
        "pow" => need(Pow, 2),
        "exp" => need(Exp, 1),
        "sign" => need(Sign, 1),
        "step" => need(Step, 2),
        "smoothstep" => need(Smoothstep, 3),
        "length2" => need(Length2, 2),
        "length3" => need(Length3, 3),
        "mod" => need(Mod, 2),
        _ => Err(format!("'{name}' is not an allowed function")),
    }
}

pub fn ret_ty(api: Api) -> Ty {
    use Api::*;
    match api {
        RotX | RotY | RotZ | Rgb | Hsv | MixV => Ty::V,
        _ => Ty::F,
    }
}

/// Number of scalar inputs (vec3 arguments count as 3).
pub fn arg_width(api: Api) -> usize {
    use Api::*;
    match api {
        Sphere => 4,
        Box3 => 6,
        RoundBox => 7,
        Cylinder => 5,
        CappedCone => 6,
        Capsule => 10,
        CapsuleV => 5,
        Torus => 5,
        Plane => 7,
        PlaneY => 4,
        Ellipsoid => 6,
        Union | Subtract | Intersect | Min | Max | Atan2 | Pow | Step | Length2 | Mod => 2,
        SmoothUnion | SmoothSubtract | Clamp | Smoothstep | Length3 | Noise3 | Hash | Rgb | Hsv | Mix => 3,
        RotX | RotY | RotZ => 4,
        MixV => 7,
        Abs | Floor | Ceil | Round | Fract | Sin | Cos | Tan | Sqrt | Exp | Sign => 1,
    }
}

pub fn wgsl_name(api: Api) -> &'static str {
    use Api::*;
    match api {
        Sphere => "api_sphere",
        Box3 => "api_box",
        RoundBox => "api_round_box",
        Cylinder => "api_cylinder",
        CappedCone => "api_capped_cone",
        Capsule => "api_capsule",
        CapsuleV => "api_capsule_v",
        Torus => "api_torus",
        Plane => "api_plane",
        PlaneY => "api_plane_y",
        Ellipsoid => "api_ellipsoid",
        Union => "min",
        SmoothUnion => "api_smooth_union",
        Subtract => "api_subtract",
        SmoothSubtract => "api_smooth_subtract",
        Intersect => "max",
        RotX => "api_rot_x",
        RotY => "api_rot_y",
        RotZ => "api_rot_z",
        Noise3 => "api_noise3",
        Hash => "api_hash",
        Rgb => "api_rgb",
        Hsv => "api_hsv",
        Mix => "api_mix",
        MixV => "api_mix_v",
        Abs => "abs",
        Min => "min",
        Max => "max",
        Clamp => "api_clamp",
        Floor => "floor",
        Ceil => "ceil",
        Round => "round",
        Fract => "api_fract",
        Sin => "sin",
        Cos => "cos",
        Tan => "tan",
        Atan2 => "atan2",
        Sqrt => "sqrt",
        Pow => "api_pow",
        Exp => "exp",
        Sign => "sign",
        Step => "api_step",
        Smoothstep => "api_smoothstep",
        Length2 => "api_length2",
        Length3 => "api_length3",
        Mod => "api_mod",
    }
}

#[inline]
fn len2(a: f32, b: f32) -> f32 {
    (a * a + b * b).sqrt()
}
#[inline]
fn len3(a: f32, b: f32, c: f32) -> f32 {
    (a * a + b * b + c * c).sqrt()
}
#[inline]
fn clampf(x: f32, lo: f32, hi: f32) -> f32 {
    x.max(lo).min(hi)
}
#[inline]
fn mixf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
#[inline]
fn fmod(a: f32, b: f32) -> f32 {
    a - b * (a / b).floor()
}
#[inline]
fn fmax(a: f32, b: f32) -> f32 {
    // WGSL max/min on Metal propagate NaN the same way IEEE maxNum does.
    a.max(b)
}

/// Evaluate an API function. `a` holds the scalar inputs, `out` receives 1 or 3 values.
pub fn eval(api: Api, a: &[f32], out: &mut [f32; 3]) {
    use Api::*;
    let g = |i: usize| a.get(i).copied().unwrap_or(0.0);
    let r = match api {
        Sphere => len3(g(0), g(1), g(2)) - g(3),
        Box3 => sd_box(g(0), g(1), g(2), g(3), g(4), g(5)),
        RoundBox => {
            let r = g(6);
            sd_box(g(0), g(1), g(2), g(3) - r, g(4) - r, g(5) - r) - r
        }
        Cylinder => {
            let dx = len2(g(0), g(2)) - g(3);
            let dy = g(1).abs() - g(4);
            dx.max(dy).min(0.0) + len2(dx.max(0.0), dy.max(0.0))
        }
        CappedCone => sd_capped_cone(g(0), g(1), g(2), g(3), g(4), g(5)),
        Capsule => {
            let (px, py, pz) = (g(0) - g(3), g(1) - g(4), g(2) - g(5));
            let (bx, by, bz) = (g(6) - g(3), g(7) - g(4), g(8) - g(5));
            let bb = (bx * bx + by * by + bz * bz).max(1e-8);
            let h = clampf((px * bx + py * by + pz * bz) / bb, 0.0, 1.0);
            len3(px - bx * h, py - by * h, pz - bz * h) - g(9)
        }
        CapsuleV => {
            let y = g(1) - clampf(g(1), 0.0, g(3));
            len3(g(0), y, g(2)) - g(4)
        }
        Torus => len2(len2(g(0), g(2)) - g(3), g(1)) - g(4),
        Plane => {
            let l = len3(g(3), g(4), g(5)).max(1e-6);
            (g(0) * g(3) + g(1) * g(4) + g(2) * g(5)) / l + g(6)
        }
        PlaneY => g(1) - g(3),
        Ellipsoid => {
            let (rx, ry, rz) = (g(3), g(4), g(5));
            let k0 = len3(g(0) / rx, g(1) / ry, g(2) / rz);
            let k1 = len3(g(0) / (rx * rx), g(1) / (ry * ry), g(2) / (rz * rz)).max(1e-6);
            k0 * (k0 - 1.0) / k1
        }
        Union | Min => g(0).min(g(1)),
        Intersect | Max => fmax(g(0), g(1)),
        SmoothUnion => {
            let k = g(2).max(1e-4);
            let h = clampf(0.5 + 0.5 * (g(1) - g(0)) / k, 0.0, 1.0);
            mixf(g(1), g(0), h) - k * h * (1.0 - h)
        }
        Subtract => fmax(g(0), -g(1)),
        SmoothSubtract => {
            let k = g(2).max(1e-4);
            let h = clampf(0.5 - 0.5 * (g(0) + g(1)) / k, 0.0, 1.0);
            mixf(g(0), -g(1), h) + k * h * (1.0 - h)
        }
        RotX => {
            let (s, c) = g(3).sin_cos();
            *out = [g(0), c * g(1) - s * g(2), s * g(1) + c * g(2)];
            return;
        }
        RotY => {
            let (s, c) = g(3).sin_cos();
            *out = [c * g(0) + s * g(2), g(1), -s * g(0) + c * g(2)];
            return;
        }
        RotZ => {
            let (s, c) = g(3).sin_cos();
            *out = [c * g(0) - s * g(1), s * g(0) + c * g(1), g(2)];
            return;
        }
        Noise3 => vnoise3(g(0), g(1), g(2), 1234),
        Hash => hash3f(g(0), g(1), g(2)),
        Rgb => {
            *out = [
                clampf(g(0) / 255.0, 0.0, 1.0),
                clampf(g(1) / 255.0, 0.0, 1.0),
                clampf(g(2) / 255.0, 0.0, 1.0),
            ];
            return;
        }
        Hsv => {
            let h = g(0);
            let s = clampf(g(1), 0.0, 1.0);
            let v = clampf(g(2), 0.0, 1.0);
            let ch = |o: f32| clampf((fmod(h * 6.0 + o, 6.0) - 3.0).abs() - 1.0, 0.0, 1.0);
            *out = [v * mixf(1.0, ch(0.0), s), v * mixf(1.0, ch(4.0), s), v * mixf(1.0, ch(2.0), s)];
            return;
        }
        Mix => mixf(g(0), g(1), g(2)),
        MixV => {
            let t = g(6);
            *out = [mixf(g(0), g(3), t), mixf(g(1), g(4), t), mixf(g(2), g(5), t)];
            return;
        }
        Abs => g(0).abs(),
        Clamp => clampf(g(0), g(1), g(2)),
        Floor => g(0).floor(),
        Ceil => g(0).ceil(),
        Round => g(0).round_ties_even(),
        Fract => g(0) - g(0).floor(),
        Sin => g(0).sin(),
        Cos => g(0).cos(),
        Tan => g(0).tan(),
        Atan2 => g(0).atan2(g(1)),
        Sqrt => g(0).sqrt(),
        Pow => g(0).abs().max(1e-20).powf(g(1)),
        Exp => g(0).exp(),
        Sign => {
            if g(0) > 0.0 {
                1.0
            } else if g(0) < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
        Step => {
            if g(1) < g(0) {
                0.0
            } else {
                1.0
            }
        }
        Smoothstep => {
            let t = clampf((g(2) - g(0)) / (g(1) - g(0)), 0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }
        Length2 => len2(g(0), g(1)),
        Length3 => len3(g(0), g(1), g(2)),
        Mod => fmod(g(0), g(1)),
    };
    out[0] = r;
}

fn sd_box(x: f32, y: f32, z: f32, bx: f32, by: f32, bz: f32) -> f32 {
    let qx = x.abs() - bx;
    let qy = y.abs() - by;
    let qz = z.abs() - bz;
    len3(qx.max(0.0), qy.max(0.0), qz.max(0.0)) + qx.max(qy.max(qz)).min(0.0)
}

fn sd_capped_cone(x: f32, y: f32, z: f32, h: f32, r1: f32, r2: f32) -> f32 {
    let qx = len2(x, z);
    let qy = y;
    let (k1x, k1y) = (r2, h);
    let (k2x, k2y) = (r2 - r1, 2.0 * h);
    let rr = if qy < 0.0 { r1 } else { r2 };
    let cax = qx - qx.min(rr);
    let cay = qy.abs() - h;
    let kk = (k2x * k2x + k2y * k2y).max(1e-8);
    let t = clampf(((k1x - qx) * k2x + (k1y - qy) * k2y) / kk, 0.0, 1.0);
    let cbx = qx - k1x + k2x * t;
    let cby = qy - k1y + k2y * t;
    let s = if cbx < 0.0 && cay < 0.0 { -1.0 } else { 1.0 };
    s * (cax * cax + cay * cay).min(cbx * cbx + cby * cby).sqrt()
}

/// WGSL twins of everything in `eval`.
pub const API_WGSL: &str = r#"
fn api_len2(a: f32, b: f32) -> f32 { return sqrt(a * a + b * b); }
fn api_len3(a: f32, b: f32, c: f32) -> f32 { return sqrt(a * a + b * b + c * c); }
fn api_length2(a: f32, b: f32) -> f32 { return api_len2(a, b); }
fn api_length3(a: f32, b: f32, c: f32) -> f32 { return api_len3(a, b, c); }
fn api_clamp(x: f32, lo: f32, hi: f32) -> f32 { return min(max(x, lo), hi); }
fn api_mix(a: f32, b: f32, t: f32) -> f32 { return a + (b - a) * t; }
fn api_mix_v(a: vec3f, b: vec3f, t: f32) -> vec3f { return a + (b - a) * t; }
fn api_mod(a: f32, b: f32) -> f32 { return a - b * floor(a / b); }
fn api_fract(a: f32) -> f32 { return a - floor(a); }
fn api_pow(a: f32, b: f32) -> f32 { return pow(max(abs(a), 1e-20), b); }
fn api_step(e: f32, x: f32) -> f32 { if (x < e) { return 0.0; } return 1.0; }
fn api_smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
  let t = api_clamp((x - e0) / (e1 - e0), 0.0, 1.0);
  return t * t * (3.0 - 2.0 * t);
}
fn api_sphere(x: f32, y: f32, z: f32, r: f32) -> f32 { return api_len3(x, y, z) - r; }
fn api_box(x: f32, y: f32, z: f32, bx: f32, by: f32, bz: f32) -> f32 {
  let qx = abs(x) - bx; let qy = abs(y) - by; let qz = abs(z) - bz;
  return api_len3(max(qx, 0.0), max(qy, 0.0), max(qz, 0.0)) + min(max(qx, max(qy, qz)), 0.0);
}
fn api_round_box(x: f32, y: f32, z: f32, bx: f32, by: f32, bz: f32, r: f32) -> f32 {
  return api_box(x, y, z, bx - r, by - r, bz - r) - r;
}
fn api_cylinder(x: f32, y: f32, z: f32, r: f32, h: f32) -> f32 {
  let dx = api_len2(x, z) - r; let dy = abs(y) - h;
  return min(max(dx, dy), 0.0) + api_len2(max(dx, 0.0), max(dy, 0.0));
}
fn api_capped_cone(x: f32, y: f32, z: f32, h: f32, r1: f32, r2: f32) -> f32 {
  let qx = api_len2(x, z); let qy = y;
  let k1x = r2; let k1y = h; let k2x = r2 - r1; let k2y = 2.0 * h;
  var rr = r2; if (qy < 0.0) { rr = r1; }
  let cax = qx - min(qx, rr); let cay = abs(qy) - h;
  let kk = max(k2x * k2x + k2y * k2y, 1e-8);
  let t = api_clamp(((k1x - qx) * k2x + (k1y - qy) * k2y) / kk, 0.0, 1.0);
  let cbx = qx - k1x + k2x * t; let cby = qy - k1y + k2y * t;
  var s = 1.0; if (cbx < 0.0 && cay < 0.0) { s = -1.0; }
  return s * sqrt(min(cax * cax + cay * cay, cbx * cbx + cby * cby));
}
fn api_capsule(x: f32, y: f32, z: f32, ax: f32, ay: f32, az: f32, bx0: f32, by0: f32, bz0: f32, r: f32) -> f32 {
  let px = x - ax; let py = y - ay; let pz = z - az;
  let bx = bx0 - ax; let by = by0 - ay; let bz = bz0 - az;
  let bb = max(bx * bx + by * by + bz * bz, 1e-8);
  let h = api_clamp((px * bx + py * by + pz * bz) / bb, 0.0, 1.0);
  return api_len3(px - bx * h, py - by * h, pz - bz * h) - r;
}
fn api_capsule_v(x: f32, y: f32, z: f32, h: f32, r: f32) -> f32 {
  let yy = y - api_clamp(y, 0.0, h);
  return api_len3(x, yy, z) - r;
}
fn api_torus(x: f32, y: f32, z: f32, R: f32, r: f32) -> f32 { return api_len2(api_len2(x, z) - R, y) - r; }
fn api_plane(x: f32, y: f32, z: f32, nx: f32, ny: f32, nz: f32, d: f32) -> f32 {
  let l = max(api_len3(nx, ny, nz), 1e-6);
  return (x * nx + y * ny + z * nz) / l + d;
}
fn api_plane_y(x: f32, y: f32, z: f32, h: f32) -> f32 { return y - h; }
fn api_ellipsoid(x: f32, y: f32, z: f32, rx: f32, ry: f32, rz: f32) -> f32 {
  let k0 = api_len3(x / rx, y / ry, z / rz);
  let k1 = max(api_len3(x / (rx * rx), y / (ry * ry), z / (rz * rz)), 1e-6);
  return k0 * (k0 - 1.0) / k1;
}
fn api_smooth_union(a: f32, b: f32, k0: f32) -> f32 {
  let k = max(k0, 1e-4);
  let h = api_clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
  return api_mix(b, a, h) - k * h * (1.0 - h);
}
fn api_subtract(a: f32, b: f32) -> f32 { return max(a, -b); }
fn api_smooth_subtract(a: f32, b: f32, k0: f32) -> f32 {
  let k = max(k0, 1e-4);
  let h = api_clamp(0.5 - 0.5 * (a + b) / k, 0.0, 1.0);
  return api_mix(a, -b, h) + k * h * (1.0 - h);
}
fn api_rot_x(x: f32, y: f32, z: f32, a: f32) -> vec3f { let s = sin(a); let c = cos(a); return vec3f(x, c * y - s * z, s * y + c * z); }
fn api_rot_y(x: f32, y: f32, z: f32, a: f32) -> vec3f { let s = sin(a); let c = cos(a); return vec3f(c * x + s * z, y, -s * x + c * z); }
fn api_rot_z(x: f32, y: f32, z: f32, a: f32) -> vec3f { let s = sin(a); let c = cos(a); return vec3f(c * x - s * y, s * x + c * y, z); }
fn api_noise3(x: f32, y: f32, z: f32) -> f32 { return vnoise3(x, y, z, 1234u); }
fn api_hash(a: f32, b: f32, c: f32) -> f32 { return hash3f(a, b, c); }
fn api_rgb(r: f32, g: f32, b: f32) -> vec3f {
  return vec3f(api_clamp(r / 255.0, 0.0, 1.0), api_clamp(g / 255.0, 0.0, 1.0), api_clamp(b / 255.0, 0.0, 1.0));
}
fn api_hsv_ch(h: f32, o: f32) -> f32 { return api_clamp(abs(api_mod(h * 6.0 + o, 6.0) - 3.0) - 1.0, 0.0, 1.0); }
fn api_hsv(h: f32, s0: f32, v0: f32) -> vec3f {
  let s = api_clamp(s0, 0.0, 1.0); let v = api_clamp(v0, 0.0, 1.0);
  return vec3f(v * api_mix(1.0, api_hsv_ch(h, 0.0), s), v * api_mix(1.0, api_hsv_ch(h, 4.0), s), v * api_mix(1.0, api_hsv_ch(h, 2.0), s));
}
"#;
