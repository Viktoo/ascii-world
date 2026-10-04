// Raymarcher: terrain heightfield + instanced SDF objects, sun/sky/fog/water.
// Appended after common.wgsl, the API functions, the generated types and the
// dispatch functions `type_sdf` / `type_color`.

const MAX_CAND: u32 = 24u;
const NEAR: f32 = 0.06;

var<private> cand: array<u32, 24>;
var<private> cand_t: array<vec2f, 24>;
// bounding sphere (centre, radius) of each candidate, cached to avoid storage reads per step
var<private> cand_s: array<vec4f, 24>;
var<private> ncand: u32;
var<private> dither: f32;

fn inst_params(i: Inst) -> Params {
  return Params(i.k0.x, i.k0.y, i.k0.z, i.k0.w, i.k1.x, i.k1.y, i.k1.z, i.k1.w,
                i.s0.x, i.s0.y, i.s0.z, i.s0.w, i.s1.x, i.s1.y, i.s1.z, i.s1.w);
}

fn to_local(i: Inst, p: vec3f) -> vec3f {
  let d = p - i.pos_scale.xyz;
  let c = i.rot.x; let s = i.rot.y;
  return vec3f(c * d.x - s * d.z, d.y, s * d.x + c * d.z) / i.pos_scale.w;
}

fn inst_center(i: Inst) -> vec3f {
  return i.pos_scale.xyz + vec3f(0.0, bitcast<f32>(i.info.y) * i.pos_scale.w, 0.0);
}

fn cut_sdf(c: vec4f, p: vec3f) -> f32 {
  let d = p - c.xyz;
  if (c.w > 0.0) { return length(d) - c.w; }
  let q = abs(d) - vec3f(-c.w);
  return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// A shape's local distance with the instance's cuts taken out.
fn shape_sdf(i: Inst, lp: vec3f) -> f32 {
  var d = type_sdf(i.info.x, lp, inst_params(i));
  for (var c = 0u; c < 4u; c = c + 1u) {
    let cut = i.cuts[c];
    if (cut.w != 0.0) { d = max(d, -cut_sdf(cut, lp)); }
  }
  return d;
}

fn inst_sdf(idx: u32, p: vec3f) -> f32 {
  let i = insts[idx];
  return shape_sdf(i, to_local(i, p)) * i.pos_scale.w;
}

fn consider(idx: u32, ro: vec3f, rd: vec3f, tmax: f32, dedupe: bool) {
  let i = insts[idx];
  if (i.rot.w < 1.0 && i.rot.w <= dither) { return; }
  let ctr = inst_center(i);
  let oc = ro - ctr;
  let r = i.rot.z;
  let b = dot(oc, rd);
  let c = dot(oc, oc) - r * r;
  let disc = b * b - c;
  if (disc < 0.0) { return; }
  let sq = sqrt(disc);
  let t0 = -b - sq;
  let t1 = -b + sq;
  if (t1 < 0.0 || t0 > tmax) { return; }
  if (dedupe) {
    for (var j = 0u; j < ncand; j = j + 1u) { if (cand[j] == idx) { return; } }
  }
  // Keep the list sorted by entry distance; when full, drop the farthest.
  if (ncand == MAX_CAND && t0 >= cand_t[MAX_CAND - 1u].x) { return; }
  var k = min(ncand, MAX_CAND - 1u);
  loop {
    if (k == 0u || cand_t[k - 1u].x <= t0) { break; }
    cand[k] = cand[k - 1u];
    cand_t[k] = cand_t[k - 1u];
    cand_s[k] = cand_s[k - 1u];
    k = k - 1u;
  }
  cand[k] = idx;
  cand_t[k] = vec2f(t0, t1);
  cand_s[k] = vec4f(ctr, r);
  if (ncand < MAX_CAND) { ncand = ncand + 1u; }
}

fn gather(ro: vec3f, rd: vec3f, tmax: f32) {
  ncand = 0u;
  if ((G.dims.w & 4u) != 0u) { return; }
  let n = G.dims.z;
  if ((G.dims.w & 1u) == 0u) {
    for (var i = 0u; i < n; i = i + 1u) { consider(i, ro, rd, tmax, false); }
    return;
  }
  // Coarse XZ grid: walk the cells the ray crosses, nearest first.
  let cs = G.grid0.z;
  let org = G.grid0.xy;
  let gw = i32(G.grid1.x); let gh = i32(G.grid1.y);
  var cx = i32(floor((ro.x - org.x) / cs));
  var cz = i32(floor((ro.z - org.y) / cs));
  let sx = select(-1, 1, rd.x >= 0.0);
  let sz = select(-1, 1, rd.z >= 0.0);
  let ix = select(1e30, abs(1.0 / rd.x), abs(rd.x) > 1e-6);
  let iz = select(1e30, abs(1.0 / rd.z), abs(rd.z) > 1e-6);
  let nbx = org.x + f32(cx + select(0, 1, sx > 0)) * cs;
  let nbz = org.y + f32(cz + select(0, 1, sz > 0)) * cs;
  var tx = abs(nbx - ro.x) * ix;
  var tz = abs(nbz - ro.z) * iz;
  let dx = cs * ix; let dz = cs * iz;
  var t = 0.0;
  for (var step = 0; step < 96; step = step + 1) {
    if (cx >= 0 && cz >= 0 && cx < gw && cz < gh) {
      let cell = cells[u32(cz * gw + cx)];
      for (var k = 0u; k < cell.y; k = k + 1u) {
        consider(items[cell.x + k], ro, rd, tmax, true);
      }
    }
    if (ncand >= MAX_CAND || t > tmax) { break; }
    if (tx < tz) { t = tx; tx = tx + dx; cx = cx + sx; } else { t = tz; tz = tz + dz; cz = cz + sz; }
  }
}

// x = safe step distance, y = candidate slot (-1 = terrain), z = distance to
// real surfaces only (bounding spheres excluded; used for penumbra and AO)
// `t` selects terrain detail; `ray_t` >= 0 lets candidates the ray has already
// left be skipped (their distance can only grow from there).
fn map_scene_lod(p: vec3f, t: f32, ray_t: f32) -> vec3f {
  var dt = (p.y - terrain_height_lod(p.x, p.z, t)) * 0.6;
  var d = dt;
  var id = -1.0;
  for (var j = 0u; j < ncand; j = j + 1u) {
    if (ray_t > cand_t[j].y + 0.5) { continue; }
    let sph = cand_s[j];
    let ds = length(p - sph.xyz) - sph.w;
    if (ds >= d) { continue; }
    if (ds < 0.25) {
      let i = insts[cand[j]];
      let lp = to_local(i, p);
      // Local bounding box (much tighter than the sphere for tall or flat shapes).
      let bx = bitcast<f32>(i.info.z);
      let bz = bitcast<f32>(i.info.w);
      if (bx > 0.0) {
        let rs = sph.w / i.pos_scale.w;
        let hy = sqrt(max(rs * rs - bx * bx - bz * bz, 0.0)) + 0.02;
        let q = abs(lp - vec3f(0.0, bitcast<f32>(i.info.y), 0.0)) - vec3f(bx, hy, bz);
        let db = (length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0)) * i.pos_scale.w;
        if (db > 0.2) {
          if (db < d) { d = db; }
          continue;
        }
      }
      let dj = shape_sdf(i, lp) * i.pos_scale.w;
      dt = min(dt, dj);
      if (dj < d) { d = dj; id = f32(j); }
    } else {
      d = ds;
    }
  }
  return vec3f(d, id, dt);
}

fn map_scene(p: vec3f) -> vec3f { return map_scene_lod(p, 0.0, -1.0); }

// Along a ray: only spheres the ray is inside right now are evaluated, and the
// step never jumps past the next sphere's entry. x = step, y = slot, z = true distance.
fn map_ray(p: vec3f, t: f32, lod_t: f32, relax: f32) -> vec3f {
  let dterr = (p.y - terrain_height_lod(p.x, p.z, lod_t)) * 0.6;
  var dtrue = dterr;
  var id = -1.0;
  var step = dterr * relax;
  for (var j = 0u; j < ncand; j = j + 1u) {
    let tr = cand_t[j];
    if (t < tr.x - 0.02) { step = min(step, max(tr.x - t, 0.0)); break; }
    if (t > tr.y + 0.02) { continue; }
    let i = insts[cand[j]];
    let lp = to_local(i, p);
    let bx = bitcast<f32>(i.info.z);
    let bz = bitcast<f32>(i.info.w);
    if (bx > 0.0) {
      let rs = i.rot.z / i.pos_scale.w;
      let hy = sqrt(max(rs * rs - bx * bx - bz * bz, 0.0)) + 0.02;
      let q = abs(lp - vec3f(0.0, bitcast<f32>(i.info.y), 0.0)) - vec3f(bx, hy, bz);
      let db = (length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0)) * i.pos_scale.w;
      if (db > 0.2) { step = min(step, db); continue; }
    }
    let dj = shape_sdf(i, lp) * i.pos_scale.w;
    if (dj < dtrue) { dtrue = dj; id = f32(j); }
    step = min(step, dj);
  }
  return vec3f(step, id, dtrue);
}

struct Hit { t: f32, id: f32, steps: f32, }

fn march(ro: vec3f, rd: vec3f, tmax: f32) -> Hit {
  var t = NEAR;
  var tp = NEAR;
  var h = Hit(1e9, -2.0, 0.0);
  for (var i = 0; i < 160; i = i + 1) {
    let p = ro + rd * t;
    // Far away the height bound is loosened (bisection repairs overshoots).
    let m = map_ray(p, t, t, select(1.0, 1.3, t > 40.0));
    let eps = 0.0012 * t + 0.002;
    if (m.z < eps) {
      h = Hit(t, m.y, f32(i));
      break;
    }
    tp = t;
    t = t + max(m.x, 0.01 + 0.002 * t);
    if (t > tmax) { break; }
  }
  if (h.id == -1.0) {
    // Refine terrain hits by bisection (the height bound can overshoot).
    var a = tp; var b = h.t;
    for (var k = 0; k < 6; k = k + 1) {
      let m = (a + b) * 0.5;
      let p = ro + rd * m;
      if (p.y - terrain_height_lod(p.x, p.z, m) > 0.0) { a = m; } else { b = m; }
    }
    h.t = b;
  }
  return h;
}

fn terrain_normal(p: vec3f, t: f32) -> vec3f {
  let e = 0.04 + 0.0025 * t;
  let hx = terrain_height_lod(p.x + e, p.z, t) - terrain_height_lod(p.x - e, p.z, t);
  let hz = terrain_height_lod(p.x, p.z + e, t) - terrain_height_lod(p.x, p.z - e, t);
  return normalize(vec3f(-hx, 2.0 * e, -hz));
}

fn inst_normal(idx: u32, p: vec3f, t: f32) -> vec3f {
  let e = 0.002 + 0.0006 * t;
  let k1 = vec3f(1.0, -1.0, -1.0); let k2 = vec3f(-1.0, -1.0, 1.0);
  let k3 = vec3f(-1.0, 1.0, -1.0); let k4 = vec3f(1.0, 1.0, 1.0);
  return normalize(k1 * inst_sdf(idx, p + k1 * e) + k2 * inst_sdf(idx, p + k2 * e)
                 + k3 * inst_sdf(idx, p + k3 * e) + k4 * inst_sdf(idx, p + k4 * e));
}

fn soft_shadow(ro: vec3f, rd: vec3f) -> f32 {
  let tmax = 24.0;
  gather(ro, rd, tmax);
  var res = 1.0;
  var t = 0.08;
  for (var i = 0; i < 28; i = i + 1) {
    let p = ro + rd * t;
    // Coarse terrain is plenty for a short soft shadow.
    let m = map_ray(p, t, 1000.0, 1.0);
    res = min(res, 10.0 * m.z / t);
    if (res < 0.02) { return 0.0; }
    t = t + clamp(m.x, 0.08 + 0.04 * t, 3.0);
    if (t > tmax) { break; }
  }
  return clamp(res, 0.0, 1.0);
}

fn sky(rd: vec3f) -> vec3f {
  let up = max(rd.y, 0.0);
  var col = mix(G.sky_hor.xyz, G.sky_zen.xyz, sqrt(up));
  if (rd.y < 0.0) { col = G.sky_hor.xyz * (1.0 + rd.y * 0.3); }
  let s = max(dot(rd, G.sun_dir.xyz), 0.0);
  let day = G.sun_dir.w;
  let night = G.sun_col.w;
  col = col + G.sun_col.xyz * (pow(s, 900.0) * 6.0 * day + pow(s, 10.0) * 0.22 * day);
  // moon disc (sun_dir points at the moon at night)
  col = col + vec3f(0.85, 0.9, 1.0) * smoothstep(0.9993, 0.9996, s) * night;
  if (night > 0.0 && rd.y > 0.02) {
    let q = rd * 260.0;
    let hsh = hash3f(floor(q.x), floor(q.y), floor(q.z));
    if (hsh > 0.9965) {
      let tw = 0.6 + 0.4 * sin(G.cam_pos.w * 1.7 + hsh * 900.0);
      col = col + vec3f(0.9, 0.92, 1.0) * night * tw * smoothstep(0.02, 0.2, rd.y);
    }
  }
  return col;
}

fn fog_color(rd: vec3f) -> vec3f {
  let s = max(dot(rd, G.sun_dir.xyz), 0.0);
  return G.sky_hor.xyz + G.sun_col.xyz * pow(s, 6.0) * 0.25 * G.sun_dir.w;
}

fn to_lin(c: vec3f) -> vec3f { return c * c; }

// Lanterns, fires and other glowing things: no shadows, soft falloff.
fn point_light(p: vec3f, n: vec3f) -> vec3f {
  var sum = vec3f(0.0);
  let count = min(G.grid1.z, 8u);
  for (var i = 0u; i < count; i = i + 1u) {
    let l = G.lights[i];
    let c = G.light_cols[i];
    let d = l.xyz - p;
    let dist = length(d);
    if (dist > c.w) { continue; }
    let fall = 1.0 - smoothstep(c.w * 0.4, c.w, dist);
    let ndl = max(dot(n, d / max(dist, 1e-3)), 0.0) * 0.8 + 0.2;
    sum = sum + to_lin(c.xyz) * l.w * ndl * fall / (1.0 + dist * dist * 0.3);
  }
  return sum;
}

// Generic looks every object can have: charred, wet, highlighted (fx.w > 0)
// or corrupted (fx.w < 0: the dark creeping over it).
fn apply_fx(albedo: vec3f, fx: vec4f) -> vec3f {
  var a = mix(albedo, vec3f(0.07, 0.06, 0.055), clamp(fx.x, 0.0, 1.0));
  a = a * (1.0 - 0.35 * clamp(fx.y, 0.0, 1.0));
  a = mix(a, vec3f(0.04, 0.03, 0.03), clamp(-fx.w, 0.0, 1.0) * 0.7);
  return mix(a, vec3f(1.0, 0.97, 0.8), clamp(fx.w, 0.0, 1.0) * 0.28);
}

// The faint dull red corrupted things give off, like embers under ash.
fn corrupt_glow(fx: vec4f) -> vec3f {
  return vec3f(0.3, 0.02, 0.01) * clamp(-fx.w, 0.0, 1.0) * 0.12;
}

fn shade(p: vec3f, n: vec3f, albedo: vec3f, rd: vec3f, ao: f32) -> vec3f {
  let sun = G.sun_dir.xyz;
  let ndl = dot(n, sun);
  var shadow = 1.0;
  if (ndl > 0.0 && (G.dims.w & 2u) != 0u && sun.y > 0.0) {
    shadow = soft_shadow(p + n * 0.05, sun);
  }
  let a = to_lin(albedo);
  let sky_amb = to_lin(mix(G.sky_hor.xyz, G.sky_zen.xyz, 0.5 + 0.5 * n.y)) * G.sky_hor.w;
  let bounce = to_lin(G.sky_hor.xyz) * 0.15 * clamp(-n.y * 0.5 + 0.5, 0.0, 1.0) * G.sky_hor.w;
  // Moonlight stays useful at night: the land must remain walkable.
  let strength = max(0.15 + 0.85 * G.sun_dir.w, 0.75 * G.sun_col.w);
  let direct = to_lin(G.sun_col.xyz) * max(ndl, 0.0) * shadow * strength * 1.6;
  let night_floor = vec3f(0.018, 0.022, 0.035) * G.sun_col.w;
  var lin = a * (direct + (sky_amb + bounce + night_floor) * ao + point_light(p, n));
  let h = normalize(sun - rd);
  lin = lin + to_lin(G.sun_col.xyz) * pow(max(dot(n, h), 0.0), 40.0) * 0.08 * shadow * G.sun_dir.w;
  return sqrt(max(lin, vec3f(0.0)));
}

fn apply_fog(col: vec3f, t: f32, rd: vec3f) -> vec3f {
  let maxd = G.cam_fwd.w;
  var f = 1.0 - exp(-t * 0.0045 * G.sky_zen.w);
  f = max(f, smoothstep(maxd * 0.72, maxd, t));
  return mix(col, fog_color(rd), clamp(f, 0.0, 1.0));
}

fn bayer4(x: u32, y: u32) -> f32 {
  let m = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
  return (m[(y & 3u) * 4u + (x & 3u)] + 0.5) / 16.0;
}

fn render(ro: vec3f, rd: vec3f) -> vec3f {
  let tmax = G.cam_fwd.w;
  gather(ro, rd, tmax);
  let h = march(ro, rd, tmax);
  var col: vec3f;
  var t_hit = h.t;
  if ((G.dims.w & 8u) != 0u) {
    // debug: step heatmap
    return vec3f(h.steps / 200.0, f32(ncand) / 24.0, 0.0);
  }
  if (h.id < -1.5) {
    col = sky(rd);
    t_hit = 1e9;
  } else {
    let p = ro + rd * h.t;
    var n: vec3f;
    var albedo: vec3f;
    var glow = vec3f(0.0);
    var ao = 1.0;
    if (h.id < 0.0) {
      n = terrain_normal(p, h.t);
      albedo = ground_color(p.x, p.z, p.y, n.y);
      // contact darkening from nearby objects
      let o = map_scene(p + n * 0.6).z;
      ao = clamp(0.45 + o * 0.9, 0.0, 1.0);
    } else {
      let idx = cand[u32(h.id)];
      n = inst_normal(idx, p, h.t);
      let i = insts[idx];
      let raw = api_split_glow(type_color(i.info.x, to_local(i, p), inst_params(i)));
      let base = clamp(raw.xyz, vec3f(0.0), vec3f(1.0));
      albedo = apply_fx(base, i.fx);
      // Only the parts marked with glow() shine, unless the type marks none.
      let lit = select(1.0, raw.w, type_marks_glow(i.info.x));
      // The dark's own (charred past 1): whatever glows on it glows red.
      let gc = select(base, vec3f(1.0, 0.07, 0.03), i.fx.x > 1.5);
      glow = gc * clamp(i.fx.z, 0.0, 2.0) * lit + corrupt_glow(i.fx);
      let o1 = map_scene(p + n * 0.15).z;
      let o2 = map_scene(p + n * 0.5).z;
      ao = clamp(0.35 + (o1 / 0.15) * 0.3 + (o2 / 0.5) * 0.35, 0.0, 1.0);
    }
    col = shade(p, n, albedo, rd, ao) + glow;
  }
  // water plane
  let wl = G.water.w;
  if (rd.y < 0.0 && ro.y > wl) {
    let tw = (wl - ro.y) / rd.y;
    if (tw < t_hit && tw < tmax) {
      let pw = ro + rd * tw;
      let depth = max(wl - terrain_height(pw.x, pw.z), 0.0);
      let tm = G.cam_pos.w;
      let rx = vnoise2(pw.x * 0.7 + tm * 0.35, pw.z * 0.7, 9u) - 0.5;
      let rz = vnoise2(pw.x * 0.7, pw.z * 0.7 - tm * 0.3, 10u) - 0.5;
      let n = normalize(vec3f(rx * 0.22, 1.0, rz * 0.22));
      let fres = 0.03 + 0.97 * pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
      let refl = sky(reflect(rd, n));
      let body = shade(pw, vec3f(0.0, 1.0, 0.0), G.water.xyz, rd, 1.0);
      let under = col * vec3f(0.55, 0.75, 0.8);
      var wc = mix(under, body, clamp(1.0 - exp(-depth * 0.7), 0.15, 1.0));
      wc = mix(wc, refl, fres * 0.85);
      let hv = normalize(G.sun_dir.xyz - rd);
      wc = wc + G.sun_col.xyz * pow(max(dot(n, hv), 0.0), 120.0) * 0.6 * G.sun_dir.w;
      col = wc;
      t_hit = tw;
    }
  }
  if (t_hit < 1e8) { col = apply_fog(col, t_hit, rd); }
  return clamp(col, vec3f(0.0), vec3f(1.0));
}

@compute @workgroup_size(8, 8)
fn render_main(@builtin(global_invocation_id) gid: vec3u) {
  let w = G.dims.x; let hgt = G.dims.y;
  if (gid.x >= w || gid.y >= hgt) { return; }
  dither = bayer4(gid.x, gid.y);
  let u = (2.0 * (f32(gid.x) + 0.5) / f32(w)) - 1.0;
  let v = 1.0 - (2.0 * (f32(gid.y) + 0.5) / f32(hgt));
  let rd = normalize(G.cam_fwd.xyz + G.cam_right.xyz * u + G.cam_up.xyz * v);
  let col = render(G.cam_pos.xyz, rd);
  outp[gid.y * w + gid.x] = pack4x8unorm(vec4f(col, 1.0));
}

@compute @workgroup_size(8, 8)
fn heightmap_main(@builtin(global_invocation_id) gid: vec3u) {
  let n = u32(G.hmap.w);
  if (gid.x >= n || gid.y >= n) { return; }
  let x = G.hmap.x + f32(gid.x) * G.hmap.z;
  let z = G.hmap.y + f32(gid.y) * G.hmap.z;
  hmap[gid.y * n + gid.x] = terrain_height(x, z);
}

// Parity probe: evaluate a type (mode 0) or the terrain (mode 1) at given points.
@compute @workgroup_size(64)
fn probe_main(@builtin(global_invocation_id) gid: vec3u) {
  let i = gid.x;
  if (i >= G.probe.x) { return; }
  let q = probe_io[i];
  if (G.probe.y == 0u) {
    var inst = insts[0];
    inst.info.x = G.probe.z;
    let k = inst_params(inst);
    let d = shape_sdf(inst, q.xyz);
    let c = type_color(G.probe.z, q.xyz, k);
    probe_io[i] = vec4f(d, c);
  } else if (G.probe.y == 2u) {
    let s = G.seed.x;
    let wt = terrain_weights(q.x, q.z);
    let n = fbm2(q.x * 0.0045 + G.toff1.x, q.z * 0.0045 + G.toff1.y, s + 101u, 5u);
    let r = ridged2(q.x * 0.0032 + G.toff1.y, q.z * 0.0032 + G.toff1.x, s + 202u, 3u);
    probe_io[i] = vec4f(n, r, wt.w[0], wt.w[1]);
  } else {
    let h = terrain_height(q.x, q.z);
    probe_io[i] = vec4f(h, ground_color(q.x, q.z, h, 1.0));
  }
}
