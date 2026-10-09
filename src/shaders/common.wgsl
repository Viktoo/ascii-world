// Shared declarations, noise and terrain. Twins: src/noise.rs, src/terrain.rs.

struct Globals {
  cam_pos: vec4f,     // xyz, time (s)
  cam_fwd: vec4f,     // xyz, max view distance
  cam_right: vec4f,   // xyz scaled by tan(fov/2) * aspect
  cam_up: vec4f,      // xyz scaled by tan(fov/2)
  sun_dir: vec4f,     // xyz (sun, or moon at night), daylight 0..1
  sun_col: vec4f,     // rgb, night 0..1
  sky_zen: vec4f,     // rgb, fog density
  sky_hor: vec4f,     // rgb, ambient strength
  water: vec4f,       // rgb, level
  rock: vec4f,        // rgb, unused
  sand: vec4f,
  snow: vec4f,
  toff0: vec4f,       // terrain offsets 0..3
  toff1: vec4f,       // terrain offsets 4..5, -, -
  dims: vec4u,        // width, height, instance count, flags (1 = grid, 2 = shadows)
  seed: vec4u,        // seed, biome count, frame, -
  grid0: vec4f,       // origin x, origin z, cell size, -
  grid1: vec4u,       // cells x, cells z, point lights, -
  probe: vec4u,       // count, mode (0 = type, 1 = terrain), type id, -
  hmap: vec4f,        // heightmap origin x, origin z, cell size, cells per side (0 = none)
  biomes: array<vec4f, 18>, // per biome: (cx, cy, base, amp), (rough, g1), (g2, cliffs)
  lights: array<vec4f, 8>,     // point lights: position, intensity
  light_cols: array<vec4f, 8>, // colour, reach (m)
  extra: vec4u,       // hollows, roads, -, -
  hollows: array<vec4f, 16>,   // per hollow: (cx, cz, cos, sin), (hx, hz, floor, round)
  roads: array<vec4f, 32>,     // per road: (ax, az, bx, bz), (half width, r, g, b)
  // weather: 0 (cover, drift x, drift z, cloud height), 1 (cloud rgb, flash),
  // 2 (falls rgb, amount), 3 (wind dir x, z, wind, falls look), 4 (soak, dusk,
  // sheltered, gloom), 5 (the sun itself xyz, its height), 6 (sunset glow rgb,
  // daylight), 7 (dusk sky rgb, -)
  wx: array<vec4f, 8>,
}

struct Inst {
  pos_scale: vec4f,   // world position, scale
  rot: vec4f,         // cos, sin, bounding radius, fade 0..1
  tilt: vec4f,        // xyz of a unit quaternion (w >= 0 implied) applied after the yaw; 0 = upright
  k0: vec4f,          // seed, scale, a, b
  k1: vec4f,          // c, d, e, f
  s0: vec4f,          // live state s0..s3
  s1: vec4f,          // live state s4..s7
  fx: vec4f,          // charred, wet, glow, highlight
  info: vec4u,        // type id, sphere centre y, box half-extents x/z
  cuts: array<vec4f, 4>, // local centre, size (>0 sphere radius, <0 cube half-size, 0 none)
}

struct Params { seed: f32, scale: f32, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32,
  s0: f32, s1: f32, s2: f32, s3: f32, s4: f32, s5: f32, s6: f32, s7: f32, }

@group(0) @binding(0) var<uniform> G: Globals;
@group(0) @binding(1) var<storage, read> insts: array<Inst>;
@group(0) @binding(2) var<storage, read> cells: array<vec2u>;
@group(0) @binding(3) var<storage, read> items: array<u32>;
@group(0) @binding(4) var<storage, read_write> outp: array<u32>;
@group(0) @binding(5) var<storage, read_write> probe_io: array<vec4f>;
@group(0) @binding(6) var<storage, read_write> hmap: array<f32>;

fn pcg(v: u32) -> u32 {
  let state = v * 747796405u + 2891336453u;
  let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
  return (word >> 22u) ^ word;
}
fn u2f(h: u32) -> f32 { return f32(h >> 8u) * (1.0 / 16777216.0); }
fn h2(ix: i32, iy: i32, s: u32) -> u32 { return pcg(pcg(bitcast<u32>(ix) + s) + bitcast<u32>(iy)); }
fn h3(ix: i32, iy: i32, iz: i32, s: u32) -> u32 { return pcg(pcg(pcg(bitcast<u32>(ix) + s) + bitcast<u32>(iy)) + bitcast<u32>(iz)); }
fn nlerp(a: f32, b: f32, t: f32) -> f32 { return a + (b - a) * t; }
fn nfade(t: f32) -> f32 { return t * t * (3.0 - 2.0 * t); }

fn vnoise2(x: f32, y: f32, s: u32) -> f32 {
  let fx = floor(x); let fy = floor(y);
  let ix = i32(fx); let iy = i32(fy);
  let ux = nfade(x - fx); let uy = nfade(y - fy);
  let a = u2f(h2(ix, iy, s));
  let b = u2f(h2(ix + 1, iy, s));
  let c = u2f(h2(ix, iy + 1, s));
  let d = u2f(h2(ix + 1, iy + 1, s));
  return nlerp(nlerp(a, b, ux), nlerp(c, d, ux), uy);
}

fn vnoise3(x: f32, y: f32, z: f32, s: u32) -> f32 {
  let fx = floor(x); let fy = floor(y); let fz = floor(z);
  let ix = i32(fx); let iy = i32(fy); let iz = i32(fz);
  let ux = nfade(x - fx); let uy = nfade(y - fy); let uz = nfade(z - fz);
  let a = u2f(h3(ix, iy, iz, s));
  let b = u2f(h3(ix + 1, iy, iz, s));
  let c = u2f(h3(ix, iy + 1, iz, s));
  let d = u2f(h3(ix + 1, iy + 1, iz, s));
  let e = u2f(h3(ix, iy, iz + 1, s));
  let f = u2f(h3(ix + 1, iy, iz + 1, s));
  let g = u2f(h3(ix, iy + 1, iz + 1, s));
  let h = u2f(h3(ix + 1, iy + 1, iz + 1, s));
  let lo = nlerp(nlerp(a, b, ux), nlerp(c, d, ux), uy);
  let hi = nlerp(nlerp(e, f, ux), nlerp(g, h, ux), uy);
  return nlerp(lo, hi, uz);
}

fn fbm2(x: f32, y: f32, s: u32, octaves: u32) -> f32 {
  var sum = 0.0; var amp = 0.5; var norm = 0.0;
  for (var i = 0u; i < min(octaves, 5u); i = i + 1u) {
    let m = FBM_M[i];
    sum = sum + amp * vnoise2(m.x * x + m.y * y, m.z * x + m.w * y, s + i * 1013u);
    norm = norm + amp;
    amp = amp * 0.5;
  }
  return (sum / norm) * 2.0 - 1.0;
}

fn ridged2(x: f32, y: f32, s: u32, octaves: u32) -> f32 {
  var sum = 0.0; var amp = 0.5; var norm = 0.0;
  for (var i = 0u; i < min(octaves, 5u); i = i + 1u) {
    let m = RIDGE_M[i];
    let n = 1.0 - abs(vnoise2(m.x * x + m.y * y, m.z * x + m.w * y, s + i * 7919u) * 2.0 - 1.0);
    sum = sum + amp * n * n;
    norm = norm + amp;
    amp = amp * 0.5;
  }
  return sum / norm;
}

fn fbits(x: f32) -> u32 { if (x == 0.0) { return 0u; } return bitcast<u32>(x); }
fn hash3f(a: f32, b: f32, c: f32) -> f32 { return u2f(pcg(fbits(a) + pcg(fbits(b) + pcg(fbits(c))))); }

// ---- terrain ----

const BIOME_SHARPNESS: f32 = 26.0;

struct Weights { w: array<f32, 6>, }

fn terrain_weights(x: f32, z: f32) -> Weights {
  let s = G.seed.x;
  var t = vnoise2(x * 0.0011 + G.toff0.x, z * 0.0011 + G.toff0.y, s);
  var m = vnoise2(x * 0.0011 + G.toff0.z, z * 0.0011 + G.toff0.w, s + 17u);
  t = clamp((t - 0.5) * 2.4 + 0.5, 0.0, 1.0);
  m = clamp((m - 0.5) * 2.4 + 0.5, 0.0, 1.0);
  var out: Weights;
  var sum = 1e-6;
  let nb = G.seed.y;
  for (var i = 0u; i < 6u; i = i + 1u) {
    if (i < nb) {
      let b = G.biomes[i * 3u];
      let dt = t - b.x; let dm = m - b.y;
      let w = exp(-(dt * dt + dm * dm) * BIOME_SHARPNESS);
      out.w[i] = w;
      sum = sum + w;
    } else {
      out.w[i] = 0.0;
    }
  }
  for (var i = 0u; i < 6u; i = i + 1u) { out.w[i] = out.w[i] / sum; }
  return out;
}

fn terrain_height_w(x: f32, z: f32, wt: Weights) -> f32 {
  var base = 0.0; var amp = 0.0; var rough = 0.0; var cl = 0.0;
  for (var i = 0u; i < 6u; i = i + 1u) {
    if (i < G.seed.y) {
      let b0 = G.biomes[i * 3u];
      let b1 = G.biomes[i * 3u + 1u];
      base = base + wt.w[i] * b0.z;
      amp = amp + wt.w[i] * b0.w;
      rough = rough + wt.w[i] * b1.x;
      cl = cl + wt.w[i] * G.biomes[i * 3u + 2u].w;
    }
  }
  let s = G.seed.x;
  let n = fbm2(x * 0.0045 + G.toff1.x, z * 0.0045 + G.toff1.y, s + 101u, 5u);
  let r = ridged2(x * 0.0032 + G.toff1.y, z * 0.0032 + G.toff1.x, s + 202u, 3u);
  return cliff_h(x, z, base + amp * nlerp(n, r * 2.0 - 0.6, rough), cl, amp);
}

const CLIFF_FACE: f32 = 0.12;
const CLIFF_TILT: f32 = 0.2;

// Ledges and rock faces where the land has cliffs. Twin of Terrain::cliff
// in terrain.rs (see there for how it works); keep the operation order.
fn cliff_h(x: f32, z: f32, h: f32, cl: f32, amp: f32) -> f32 {
  if (cl < 0.01) { return h; }
  let s = G.seed.x;
  let k = sstep(1.0 - cl, 1.3 - cl, vnoise2(x * 0.004, z * 0.004, s + 404u)) * sstep(3.0, 12.0, amp);
  if (k <= 0.0) { return h; }
  let st = 5.0 + 7.0 * vnoise2(x * 0.0023, z * 0.0023, s + 405u);
  let j = (0.3 * vnoise2(x * 0.02, z * 0.02, s + 406u) + 0.08 * vnoise2(x * 0.12, z * 0.12, s + 408u)) * sstep(4.0, 20.0, amp);
  let w = nlerp(CLIFF_FACE, 1.0, sstep(0.6, 0.75, vnoise2(x * 0.03, z * 0.03, s + 407u)));
  let wf = min(w * (0.55 + 0.9 * vnoise2(x * 0.2, z * 0.2, s + 409u)), 1.0);
  let q = h / st + j;
  let t = q - floor(q);
  let g = CLIFF_TILT * t + (1.0 - CLIFF_TILT) * sstep(1.0 - wf, 1.0, t);
  return h + k * (g - t) * st;
}

const HOLLOW_EDGE: f32 = 0.35;

// Ground taken away (cellars, houses set into hills, dug pits): the height
// with the hollows taken out (x) and how far into one the point is (y).
// Twin of Carve::apply in terrain.rs.
fn carve(x: f32, z: f32, h: f32) -> vec2f {
  var out = h;
  var wmax = 0.0;
  let n = min(G.extra.x, 8u);
  for (var i = 0u; i < n; i = i + 1u) {
    let a = G.hollows[i * 2u];
    let b = G.hollows[i * 2u + 1u];
    let dx = x - a.x; let dz = z - a.y;
    var e: f32;
    if (b.w > 0.5) {
      e = sqrt(dx * dx + dz * dz) - b.x;
    } else {
      let lx = a.z * dx - a.w * dz;
      let lz = a.w * dx + a.z * dz;
      e = max(abs(lx) - b.x, abs(lz) - b.y);
    }
    if (e >= HOLLOW_EDGE) { continue; }
    let w = sstep(HOLLOW_EDGE, 0.0, e);
    if (h > b.z) { out = min(out, h - w * (h - b.z)); }
    wmax = max(wmax, w);
  }
  return vec2f(out, wmax);
}

fn terrain_height(x: f32, z: f32) -> f32 {
  let h = terrain_height_w(x, z, terrain_weights(x, z));
  if (G.extra.x == 0u) { return h; }
  return carve(x, z, h).x;
}

// Rendering-only level of detail: beyond ~60 m the two finest octaves fade
// out (blended, so nothing pops). Collision, placement and parity use the
// full terrain_height.
// Bilinear lookup in the camera-centred heightmap (rendering only).
fn hmap_height(x: f32, z: f32, ok: ptr<function, bool>, steep: ptr<function, bool>) -> f32 {
  let n = G.hmap.w;
  let gx = (x - G.hmap.x) / G.hmap.z;
  let gz = (z - G.hmap.y) / G.hmap.z;
  if (n < 2.0 || gx < 0.0 || gz < 0.0 || gx >= n - 1.0 || gz >= n - 1.0) { *ok = false; return 0.0; }
  let ix = u32(gx); let iz = u32(gz);
  let fx = gx - f32(ix); let fz = gz - f32(iz);
  let ni = u32(n);
  let i0 = iz * ni + ix;
  let a = hmap[i0]; let b = hmap[i0 + 1u];
  let c = hmap[i0 + ni]; let d = hmap[i0 + ni + 1u];
  *ok = true;
  // A cell this steep (a rock face) is too coarse to draw from up close.
  *steep = max(max(a, b), max(c, d)) - min(min(a, b), min(c, d)) > HMAP_STEEP;
  return mix(mix(a, b, fx), mix(c, d, fx), fz);
}

// Rise across one heightmap cell beyond which, within HMAP_EXACT metres,
// the exact height is used instead (sheer faces stay sharp).
const HMAP_STEEP: f32 = 1.2;
const HMAP_EXACT: f32 = 80.0;

fn terrain_height_lod(x: f32, z: f32, t: f32) -> f32 {
  let h = terrain_height_lod_raw(x, z, t);
  if (G.extra.x == 0u) { return h; }
  return carve(x, z, h).x;
}

fn terrain_height_lod_raw(x: f32, z: f32, t: f32) -> f32 {
  if (t > 12.0) {
    var ok = false;
    var steep = false;
    let h = hmap_height(x, z, &ok, &steep);
    if (ok && (!steep || t > HMAP_EXACT)) { return h; }
  }
  let fine = 1.0 - clamp((t - 35.0) / 40.0, 0.0, 1.0);
  if (fine >= 1.0) { return terrain_height(x, z); }
  let wt = terrain_weights(x, z);
  var base = 0.0; var amp = 0.0; var rough = 0.0; var cl = 0.0;
  for (var i = 0u; i < 6u; i = i + 1u) {
    if (i < G.seed.y) {
      let b0 = G.biomes[i * 3u];
      let b1 = G.biomes[i * 3u + 1u];
      base = base + wt.w[i] * b0.z;
      amp = amp + wt.w[i] * b0.w;
      rough = rough + wt.w[i] * b1.x;
      cl = cl + wt.w[i] * G.biomes[i * 3u + 2u].w;
    }
  }
  let s = G.seed.x;
  let fx = x * 0.0045; let fz = z * 0.0045;
  var sum = 0.0; var a = 0.5;
  for (var i = 0u; i < 5u; i = i + 1u) {
    var w = 1.0;
    if (i >= 3u) { w = fine; }
    if (w > 0.0) {
      let m = FBM_M[i];
      sum = sum + a * w * vnoise2(m.x * fx + m.y * fz, m.z * fx + m.w * fz, s + 101u + i * 1013u);
    }
    if (i >= 3u) { sum = sum + a * (1.0 - w) * 0.5; }
    a = a * 0.5;
  }
  let n = (sum / 0.96875) * 2.0 - 1.0;
  var r = 0.5;
  if (rough > 0.02) {
    r = ridged2(x * 0.0032, z * 0.0032, s + 202u, select(3u, 2u, fine <= 0.0));
  }
  return cliff_h(x, z, base + amp * nlerp(n, r * 2.0 - 0.6, rough), cl, amp);
}

fn sstep(e0: f32, e1: f32, x: f32) -> f32 {
  let t = clamp((x - e0) / (e1 - e0), 0.0, 1.0);
  return t * t * (3.0 - 2.0 * t);
}

fn ground_color(x: f32, z: f32, h: f32, ny: f32) -> vec3f {
  let wt = terrain_weights(x, z);
  let s = G.seed.x;
  let v = vnoise2(x * 0.11, z * 0.11, s + 303u);
  let v2 = vnoise2(x * 0.023, z * 0.023, s + 304u);
  var c = vec3f(0.0);
  for (var i = 0u; i < 6u; i = i + 1u) {
    if (i < G.seed.y) {
      let b1 = G.biomes[i * 3u + 1u];
      let b2 = G.biomes[i * 3u + 2u];
      c = c + wt.w[i] * mix(b1.yzw, b2.xyz, clamp(v * 0.5 + v2 * 0.8, 0.0, 1.0));
    }
  }
  let steep = sstep(0.82, 0.68, ny);
  let strata = sstep(0.3, 0.7, vnoise2(h * 0.9, (x + z) * 0.015, s + 305u));
  let bed = G.rock.xyz * (0.55 + 0.25 * v + 0.45 * strata);
  c = mix(c, bed * vec3f(1.0 + 0.1 * strata, 1.0, 1.0 - 0.1 * strata), steep);
  let beach = sstep(G.water.w + 1.4, G.water.w + 0.3, h);
  c = mix(c, G.sand.xyz, beach);
  let snowy = sstep(46.0, 56.0, h + v * 6.0) * sstep(0.6, 0.8, ny);
  c = mix(c, G.snow.xyz, snowy);
  let nr = min(G.extra.y, 16u);
  for (var i = 0u; i < nr; i = i + 1u) {
    let a = G.roads[i * 2u];
    let b = G.roads[i * 2u + 1u];
    let ab = a.zw - a.xy;
    let ap = vec2f(x, z) - a.xy;
    let tt = clamp(dot(ap, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
    let d = length(ap - ab * tt);
    let k = sstep(b.x + 0.4, b.x - 0.2, d);
    if (k > 0.0) { c = mix(c, b.yzw * (0.9 + 0.2 * v), k); }
  }
  if (G.extra.x > 0u) {
    let dug = carve(x, z, h).y;
    c = mix(c, vec3f(0.29, 0.22, 0.16) * (0.8 + 0.3 * v), dug * 0.85);
  }
  return c;
}
