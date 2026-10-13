// Frames blended over time, at a higher resolution than each is drawn: every
// frame's rays are shifted by a different fraction of a pixel, and each new
// frame is mixed into the history, which is first moved to where the camera
// now looks and held to what the new frame shows nearby (so nothing smears).

struct View {
  pos: vec4f,
  fwd: vec4f,
  right: vec4f,
  up: vec4f,
  // jitter x, y in pixels of the frame
  jitter: vec4f,
};

struct T {
  cur: View,
  prev: View,
  // frame size, history size (pixels)
  sizes: vec4f,
  // x: 1 = start over (no usable history)
  flags: vec4u,
};

@group(0) @binding(0) var<uniform> t: T;
@group(0) @binding(1) var cur_col: texture_2d<f32>;
@group(0) @binding(2) var cur_depth: texture_2d<f32>;
@group(0) @binding(3) var hist: texture_2d<f32>;
@group(0) @binding(4) var lin: sampler;

struct Full {
  @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> Full {
  let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
  var o: Full;
  o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
  return o;
}

fn ray(v: View, s: vec2f) -> vec3f {
  return normalize(v.fwd.xyz + v.right.xyz * (s.x * 2.0 - 1.0) + v.up.xyz * (1.0 - s.y * 2.0));
}

// Catmull-Rom from the history (sharper than bilinear, so detail survives
// being moved frame after frame).
fn hist_at(uv: vec2f) -> vec3f {
  let size = vec2f(textureDimensions(hist));
  let sp = uv * size;
  let tc = floor(sp - 0.5) + 0.5;
  let f = sp - tc;
  let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
  let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
  let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
  let w3 = f * f * (-0.5 + 0.5 * f);
  let w12 = w1 + w2;
  let t0 = (tc - 1.0) / size;
  let t3 = (tc + 2.0) / size;
  let t12 = (tc + w2 / w12) / size;
  var c = vec3f(0.0);
  c += textureSampleLevel(hist, lin, vec2(t12.x, t0.y), 0.0).rgb * w12.x * w0.y;
  c += textureSampleLevel(hist, lin, vec2(t0.x, t12.y), 0.0).rgb * w0.x * w12.y;
  c += textureSampleLevel(hist, lin, vec2(t12.x, t12.y), 0.0).rgb * w12.x * w12.y;
  c += textureSampleLevel(hist, lin, vec2(t3.x, t12.y), 0.0).rgb * w3.x * w12.y;
  c += textureSampleLevel(hist, lin, vec2(t12.x, t3.y), 0.0).rgb * w12.x * w3.y;
  // The corners are small; leaving them out keeps the weights' sum near 1.
  let wsum = w12.x * w0.y + w0.x * w12.y + w12.x * w12.y + w3.x * w12.y + w12.x * w3.y;
  return max(c / wsum, vec3f(0.0));
}

@fragment
fn fs_taa(f: Full) -> @location(0) vec4f {
  let src = t.sizes.xy;
  let s = f.pos.xy / t.sizes.zw;
  // The frame's texel centres sit at the jittered positions.
  let uvc = s - t.cur.jitter.xy / src;
  let smooth_c = textureSampleLevel(cur_col, lin, uvc, 0.0).rgb;
  let fp = uvc * src;
  let tc = vec2i(floor(fp));
  let hi_i = vec2i(src) - vec2i(1);
  // What the new frame shows round here (to hold the history to), and the
  // nearest surface among it (so edges of near things move with them).
  var lo = vec3f(1e9);
  var hi = vec3f(-1e9);
  var dmin = 1e10;
  var nearest = smooth_c;
  var best = 1e9;
  for (var dy = -1; dy <= 1; dy = dy + 1) {
    for (var dx = -1; dx <= 1; dx = dx + 1) {
      let q = clamp(tc + vec2i(dx, dy), vec2i(0), hi_i);
      let c = textureLoad(cur_col, q, 0).rgb;
      lo = min(lo, c);
      hi = max(hi, c);
      dmin = min(dmin, textureLoad(cur_depth, q, 0).r);
      let off = vec2f(q) + 0.5 - fp;
      let d2 = dot(off, off);
      if (d2 < best) { best = d2; nearest = c; }
    }
  }
  if (t.flags.x == 1u) {
    return vec4f(smooth_c, 1.0);
  }
  // Where this point was in the last blended frame.
  let dir = ray(t.cur, s);
  var d = dir;
  if (dmin < 1e8) {
    d = t.cur.pos.xyz + dir * dmin - t.prev.pos.xyz;
  }
  let lam = dot(d, t.prev.fwd.xyz);
  let r = t.prev.right.xyz;
  let u = t.prev.up.xyz;
  var ps = vec2f(-1.0);
  if (lam > 1e-4) {
    let pu = dot(d, r) / (lam * dot(r, r));
    let pv = dot(d, u) / (lam * dot(u, u));
    ps = vec2f((pu + 1.0) * 0.5, (1.0 - pv) * 0.5);
  }
  if (any(ps < vec2f(0.0)) || any(ps > vec2f(1.0))) {
    return vec4f(smooth_c, 1.0);
  }
  // A little room in the box keeps still detail from flickering.
  let pad = (hi - lo) * 0.1 + vec3f(0.01);
  let h = clamp(hist_at(ps), lo - pad, hi + pad);
  // The new sample counts most where it lands on this output pixel, and more
  // while things move (the history is less to be trusted).
  let w = exp(-best * 6.0);
  let moved = length((ps - s) * t.sizes.zw);
  let alpha = clamp(0.04 + 0.2 * w + moved * 0.01, 0.04, 0.5);
  // Moving fast, the new frame's own pixels would show as blocks: take it smooth.
  let c = mix(nearest, smooth_c, clamp(moved * 0.08, 0.0, 1.0));
  return vec4f(mix(h, c, alpha), 1.0);
}
