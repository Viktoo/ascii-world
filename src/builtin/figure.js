// Built-in human body (and the template other upright peoples' bodies are
// written from). Faces +z, stands on y = 0; its own maths is for a person
// 1.75 m tall, scaled by height.
// Look: a = height (m), b = build, c = skin tone, d = top hue, e = bottom
// hue (hues round the wheel 0 … 1; 1 … 1.2 fades to no colour), f = walk
// phase (radians), g = shape (0 a man's … 1 a woman's: shoulders, chest,
// waist, hips, jaw), h = hair length (0 shaven … 0.2 cropped … 0.5 chin …
// 0.75 shoulders … 1 the waist), i = hair colour (0 black, 0.25 dark brown,
// 0.45 brown, 0.6 auburn, 0.75 blond, 0.9 flaxen, 1 grey), j = hair style
// (0 straight, 0.5 curly, 0.8 tied back), k / l = top / bottom shade (dark …
// light), m = top: how far it covers (0 bare, 0.2 a band across the chest,
// 0.4 cropped, 0.6 to the waist, 0.8 the hips, 1 a tunic to mid-thigh),
// n = sleeves (0 none … 1 to the wrist), o = neckline (0 high collar, 0.35
// round, 0.7 a deep V, 0.9 off the shoulders), p = skirt or dress below the
// waist (0 none, 0.2 short, 0.5 knee, 0.8 calf, 1 the floor), q = trouser
// legs (0 none, 0.25 shorts, 0.55 knee, 1 ankle), r = shoes (0 bare feet,
// 0.2 sandals, 0.45 shoes, 0.7 ankle boots, 1 knee boots), s = beard (0 …
// 1 long), t = underwear (0 none, 1 worn), u = trim hue (belt, hems,
// collars, shoes, hat), v = hat (0 none, 0.2 a kerchief, 0.4 a cap, 0.6 a
// brimmed hat, 0.8 a wide straw hat, 1 a tall hat), w = eye colour (0 near
// black, 0.25 brown, 0.45 hazel, 0.6 green, 0.8 grey, 1 blue), x = skin
// undertone (0 rosy … 0.5 neutral … 1 golden or olive), y = eyes (0 round
// and deep-set under a strong brow, with a crease over the lid … 1 narrower,
// tilted up at the outer corner, a fold over the lid, a low flat brow),
// z = nose (0 narrow with a high bridge … 1 broad with a low bridge).
// Clothes are shells a little outside the skin, so taking one off (its
// slider to 0) shows what is under it.
// Roles (live pose): s0/s1 = left/right arm raised sideways (0 down … 1 up),
// s2/s3 = left/right arm reaching forward (0 … 1), s4 = lean forward (radians),
// s5 = head nod, s6 = crouch (0 … 1), s8 = grip (0 open hands … 1 fists).
// The engine also poses the face: s9 = mouth open (talking), s10 = eyes shut
// (a blink, sleep), s11 = a clock in seconds that hair stirs with.
// parts(i, _, _, k): where the right hand (0), left hand (1), the middle of
// the head (2) and of the chest (3) are in this pose, by the same maths.
export const meta = { name: "figure", bounds: [0.95, 2.4, 0.95], tags: ["builtin", "figure", "body"],
  body: { height: 1.75, eye: 1.65, radius: 0.35, reach: 2.4, grip: [0.3, 1.0, 0.3],
          roles: ["raise", "reach", "lean", "head", "crouch", "grip"], gait: "biped", arms: true,
          look: { height: [1.3, 2.0], build: [0.7, 1.4], skin: [0, 1], top: [0, 1.2], bottom: [0, 1.2],
                  shape: [0, 1], hair: [0, 1], hair_color: [0, 1], hair_style: [0, 1], top_shade: [0, 1], bottom_shade: [0, 1],
                  top_cover: [0, 1, 0.75], sleeves: [0, 1, 0.8], neckline: [0, 1, 0.35], skirt: [0, 1, 0], legs: [0, 1, 1],
                  shoes: [0, 1, 0.5], beard: [0, 1, 0.1], underwear: [0, 1, 1], trim: [0, 1.2], hat: [0, 1, 0],
                  eyes: [0, 1, 0.1], undertone: [0, 1], eye_shape: [0, 1], nose: [0, 1] } } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const w = clamp(k.b, 0.7, 1.4);
  const fem = smoothstep(0.3, 0.7, k.g);
  const px = x / h;
  const py = y / h;
  const pz = z / h;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const still = 1 - clamp(max(k.s2, k.s3) * 2 + crouch, 0, 1);
  const sw = sin(k.f) * 0.32 * (1 - crouch);
  const kz = crouch * 0.32;
  const ky = 0.5 - crouch * 0.12;
  const th = 0.009;
  // What is worn.
  const tc = clamp(k.m, 0, 1);
  const sk = clamp(k.p, 0, 1);
  const lg = clamp(k.q, 0, 1);
  const sh = clamp(k.r, 0, 1);
  const uw = clamp(k.t, 0, 1);
  const band = tc > 0.04 && tc <= 0.28;
  const hem = tc > 0.28 ? mix(1.17, 0.72, (tc - 0.28) / 0.72) : 1.19;
  // Legs: thigh, calf and foot, each in trousers (or boots) as far as they go.
  const lx = (0.09 + 0.012 * fem) * w;
  const legEnd = lg > 0.04 ? 0.88 - lg * 0.8 : 9;
  const boot = sh > 0.55 ? 0.1 + (sh - 0.55) * 0.95 : 0.0;
  let legs = 10;
  for (let i = 0; i < 2; i++) {
    const side = i * 2 - 1;
    const s = sw * side;
    const ax = side * lx;
    const thigh = capsule(px, py, pz, ax, hip, 0, ax * 0.95, ky, kz - s * 0.5, (0.078 + 0.008 * fem) * w);
    const calf = capsule(px, py, pz, ax * 0.95, ky, kz - s * 0.5, ax, 0.1, -s, 0.056 * w);
    let leg = smoothUnion(thigh, calf, 0.04);
    // Trousers over the legs; boots up the calf.
    leg = min(leg, max(leg - th, legEnd - py));
    leg = min(leg, max(leg - th * 1.4, py - boot));
    let foot = 10;
    if (py < 0.3) {
      foot = capsule(px, py, pz, ax, 0.07, -s - 0.02, ax * 1.05, 0.04, -s + 0.13, 0.038 * w);
      foot = smoothUnion(foot, sphere(px - ax, py - 0.045, pz + s, 0.045), 0.03);
      if (sh > 0.3) {
        foot = foot - th * 1.6;
      } else if (sh > 0.08) {
        // A sandal: a sole under the foot.
        foot = min(foot, max(foot - th * 1.6, py - 0.025));
      }
    }
    legs = min(legs, smoothUnion(leg, foot, 0.02));
  }
  // Hips, seat and belly, in the legs' frame.
  const pelvis = smoothUnion(
    ellipsoid(px, py - hip - 0.06, pz + 0.005, (0.15 + 0.03 * fem) * w, 0.12, 0.1 * w),
    min(sphere(px - lx * 0.8, py - hip + 0.01, pz + 0.055, (0.075 + 0.015 * fem) * w), sphere(px + lx * 0.8, py - hip + 0.01, pz + 0.055, (0.075 + 0.015 * fem) * w)),
    0.04
  );
  // Upper body: lean forward around the hip.
  const q = rotX(px, py - hip, pz, -clamp(k.s4, -0.3, 0.6));
  const ux = q.x;
  const uy = q.y + 0.9;
  const uz = q.z;
  const sx = (0.24 - 0.035 * fem) * w;
  const chest = ellipsoid(ux, uy - 1.3, uz + 0.012 * (1 - fem), (0.165 - 0.028 * fem) * w, 0.17, (0.1 - 0.012 * (1 - fem)) * w);
  const belly = ellipsoid(ux, uy - 1.08, uz, (0.135 - 0.025 * fem) * w, 0.15, 0.092 * w);
  let torso = smoothUnion(chest, belly, 0.13 - 0.06 * fem);
  // A man's chest: broad and flat, pectorals over the ribs.
  if (fem < 0.95) {
    const pec = (1 - fem) * (0.7 + 0.5 * clamp(w - 0.8, 0, 0.6));
    torso = smoothUnion(torso, roundBox(ux, uy - 1.33, uz - 0.035 * w, 0.13 * w * pec, 0.06 * pec, 0.05 * w * pec, 0.04 * pec), 0.05);
  }
  torso = smoothUnion(torso, capsule(ux, uy, uz, -sx * 0.85, 1.43, -0.01, sx * 0.85, 1.43, -0.01, (0.056 - 0.008 * fem) * w), 0.05);
  if (fem > 0.05) {
    const bs = fem * (0.85 + 0.35 * hash(k.seed, 7));
    torso = smoothUnion(torso, min(
      ellipsoid(ux + 0.07 * w, uy - 1.255, uz - 0.07 * w, 0.06 * bs, 0.058 * bs, 0.05 * bs),
      ellipsoid(ux - 0.07 * w, uy - 1.255, uz - 0.07 * w, 0.06 * bs, 0.058 * bs, 0.05 * bs)), 0.03);
  }
  torso = smoothUnion(torso, pelvis, 0.05);
  const neck = capsule(ux, uy, uz, 0, 1.42, -0.01, 0, 1.57, 0.0, (0.05 - 0.008 * fem) * w);
  torso = smoothUnion(torso, neck, 0.03);
  // Torso clothes: the top (between its hem and its neckline), then what
  // is worn at the waist and below it (trousers, a skirt's band, briefs).
  const nk = clamp(k.o, 0, 1);
  // Up over the shoulders to a hole round the neck (wider as the neckline
  // opens), dipping in front; a high collar rises up the neck.
  let top = length2(ux, uz + 0.01) > 0.068 + 0.03 * nk ? 1.62 : 1.45 + max(0.1 - nk, 0) * 0.8;
  if (uz > 0) {
    // Round or scooped, then a V as it plunges.
    const nd = 0.02 + nk * 0.22;
    const nw = 0.06 + 0.1 * nk;
    top = min(top, nk < 0.55 ? (abs(ux) < nw ? 1.47 - nd * (1 - (ux * ux) / (nw * nw)) : 1.62) : 1.47 - nd + abs(ux) * nd / nw);
  }
  if (nk > 0.85) {
    top = 1.34;
  }
  if (band) {
    top = 1.34;
  }
  let cover = tc > 0.04 ? max(hem - uy, (uy - top) * 0.6) : 9;
  const waist = lg > 0.04 || sk > 0.04 ? 1.03 : uw > 0.5 ? 0.97 : 0;
  cover = min(cover, uy - waist);
  if (uw > 0.5 && fem > 0.5 && tc <= 0.04) {
    cover = min(cover, max(1.19 - uy, uy - 1.33));
  }
  torso = min(torso, max(torso - th, cover));
  // Arms: each from the shoulder, narrowing to the wrist; past it, a hand:
  // a flat palm, four fingers curling under as the grip closes, and a thumb.
  // Drawn in full only near the hand; from afar, a knob. Sleeves cover
  // the arm from the shoulder as far as they go.
  const la = clamp(k.s0, 0, 1) * 3.0;
  const ra = clamp(k.s1, 0, 1) * 3.0;
  const lf = clamp(k.s2, 0, 1) * 1.45;
  const rf = clamp(k.s3, 0, 1) * 1.45;
  const sl = tc > 0.28 ? 0.04 + clamp(k.n, 0, 1) * 0.92 : 0;
  const sl0 = nk > 0.85 ? 0.14 : 0;
  const g = clamp(k.s8, 0, 1);
  let arms = 10;
  for (let i = 0; i < 2; i++) {
    const side = i * 2 - 1;
    const a1 = i == 0 ? la : ra;
    const f1 = i == 0 ? lf : rf;
    const hx = side * (sx + 0.04 * w + 0.56 * sin(a1));
    const hy = 1.42 - 0.56 * cos(a1) * cos(f1);
    const hz = 0.56 * cos(a1) * sin(f1) + side * sw * 0.8 * still;
    const ax = side * sx;
    const al = max(length3(hx - ax, hy - 1.42, hz), 0.001);
    const dx = (hx - ax) / al;
    const dy = (hy - 1.42) / al;
    const dz = hz / al;
    // The forearm narrows to the wrist.
    const t = ((ux - ax) * dx + (uy - 1.42) * dy + uz * dz) / al;
    const tw = clamp(t, 0, 1 - 0.05 / al);
    let arm = length3(ux - ax - dx * tw * al, uy - 1.42 - dy * tw * al, uz - dz * tw * al) - (0.05 - 0.007 * fem) * w * (1 - 0.42 * smoothstep(0.45, 1, tw));
    arm = min(arm, max(arm - th, sl > 0 ? max(t - sl, sl0 - t) : 9));
    const near = length3(ux - hx, uy - hy, uz - hz);
    if (near > 0.22) {
      arms = min(arms, arm);
      // Never past the hand: an open one reaches further.
      arms = min(arms, near - 0.06 - (1 - g) * 0.09);
    } else {
      // Palm-down: from "down and a little back", made square to the arm.
      const rd = -dy - 0.3 * dz;
      let nx = -dx * rd;
      let ny = -1 - dy * rd;
      let nz = -0.3 - dz * rd;
      const nl = max(length3(nx, ny, nz), 0.001);
      nx = nx / nl;
      ny = ny / nl;
      nz = nz / nl;
      const bx = dy * nz - dz * ny;
      const by = dz * nx - dx * nz;
      const bz = dx * ny - dy * nx;
      // The hand is drawn in its own frame: s across the knuckles, v out of
      // the palm, u along the fingers.
      const qx = ux - hx;
      const qy = uy - hy;
      const qz = uz - hz;
      const hs = qx * bx + qy * by + qz * bz;
      const hv = qx * nx + qy * ny + qz * nz;
      const hu = qx * dx + qy * dy + qz * dz;
      // A flat palm, a little thicker at the heel.
      const palm = roundBox(hs, hv + 0.002, hu - 0.002, 0.037 * w, (0.0115 + 0.003 * clamp(-hu * 20, 0, 1)) * w, 0.045, 0.011);
      // Four fingers of three joints, the middle one longest, the little
      // one short and thin; they fan a little when open and curl into the
      // palm as the grip closes, every joint bending a little at rest.
      const b1 = 0.14 + g * 1.45;
      const b2 = b1 + 0.16 + g * 1.6;
      const b3 = b2 + 0.1 + g * 1.05;
      let fingers = 10;
      let knuckles = 10;
      for (let j = 0; j < 4; j++) {
        const pinky = step(2.5, j);
        const fl = 0.092 - 0.008 * abs(j - 1) - 0.012 * pinky;
        const fr = (0.0098 - 0.0009 * pinky) * w;
        const sp = (j - 1.5) * 0.07 * (1 - g);
        const cs = cos(sp);
        const ss = sin(sp);
        const ks = (j - 1.5) * 0.0205 * w;
        const ku = 0.043 - 0.003 * abs(j - 1.2);
        const m1s = ks + ss * cos(b1) * fl * 0.46;
        const m1v = sin(b1) * fl * 0.46;
        const m1u = ku + cs * cos(b1) * fl * 0.46;
        const m2s = m1s + ss * cos(b2) * fl * 0.31;
        const m2v = m1v + sin(b2) * fl * 0.31;
        const m2u = m1u + cs * cos(b2) * fl * 0.31;
        const m3s = m2s + ss * cos(b3) * fl * 0.23;
        const m3v = m2v + sin(b3) * fl * 0.23;
        const m3u = m2u + cs * cos(b3) * fl * 0.23;
        fingers = min(fingers, min(capsule(hs, hv, hu, ks, 0, ku, m1s, m1v, m1u, fr), min(capsule(hs, hv, hu, m1s, m1v, m1u, m2s, m2v, m2u, fr * 0.93), capsule(hs, hv, hu, m2s, m2v, m2u, m3s, m3v, m3u, fr * 0.84))));
        knuckles = min(knuckles, length3(hs - ks, hv + 0.003, hu - ku + 0.004) - fr * 1.12);
      }
      // The thumb comes off the heel of the palm on the side toward the
      // body, angled out; it folds across the front of a fist.
      const ts = -side;
      const t1s = mix(0.55, 0.3, g) * ts;
      const t1v = mix(0.3, 0.5, g);
      const t1u = mix(0.78, 0.85, g);
      const t2s = mix(0.38, -0.15, g) * ts;
      const t2v = mix(0.18, 0.6, g);
      const t2u = mix(0.9, 0.75, g);
      const t3s = mix(0.22, -0.95, g) * ts;
      const t3v = mix(0.2, 0.1, g);
      const t3u = mix(0.95, 0.25, g);
      const l1 = length3(t1s, t1v, t1u);
      const l2 = length3(t2s, t2v, t2u);
      const l3 = length3(t3s, t3v, t3u);
      const p0s = ts * 0.026 * w;
      const p0v = 0.006;
      const p0u = -0.032;
      const p1s = p0s + t1s / l1 * 0.046;
      const p1v = p0v + t1v / l1 * 0.046;
      const p1u = p0u + t1u / l1 * 0.046;
      const p2s = p1s + t2s / l2 * 0.033;
      const p2v = p1v + t2v / l2 * 0.033;
      const p2u = p1u + t2u / l2 * 0.033;
      const p3s = p2s + t3s / l3 * 0.028;
      const p3v = p2v + t3v / l3 * 0.028;
      const p3u = p2u + t3u / l3 * 0.028;
      const thumb = min(capsule(hs, hv, hu, p0s, p0v, p0u, p1s, p1v, p1u, 0.0135 * w), min(capsule(hs, hv, hu, p1s, p1v, p1u, p2s, p2v, p2u, 0.0112 * w), capsule(hs, hv, hu, p2s, p2v, p2u, p3s, p3v, p3u, 0.0102 * w)));
      const hand = smoothUnion(smoothUnion(palm, knuckles, 0.006), min(fingers, thumb), 0.006);
      arms = min(arms, smoothUnion(arm, hand, 0.014));
    }
  }
  let body = smoothUnion(min(legs, arms), torso, 0.05);
  // A skirt (or a tunic's tail) hangs from the waist, flaring, swinging
  // with the stride.
  const tun = tc > 0.82 ? (0.86 - hem) : 0;
  const drop = max(sk > 0.04 ? 0.12 + sk * 0.86 : 0, tun);
  if (drop > 0.01) {
    const top0 = hip + 0.13;
    const r0 = (0.155 + 0.03 * fem) * w;
    const r1 = r0 + 0.03 + 0.2 * min(drop, 0.7) * (1 - crouch * 0.5);
    const hh = drop * 0.5;
    const tt = clamp((top0 - py) / max(drop, 0.01), 0, 1);
    const sz = pz - sw * 0.12 * tt - kz * tt;
    body = min(body, cappedCone(px, py - top0 + hh, sz * 1.18, hh, r1, r0) / 1.18);
  }
  // The head, nodding about the neck.
  const nod = clamp(k.s5, -1, 1);
  const hq = rotX(ux, uy - 1.56, uz, nod * 0.35);
  const hx = hq.x;
  const hy = hq.y - 0.06;
  const hz = hq.z;
  const hl = clamp(k.h, 0, 1);
  const style = clamp(k.j, 0, 1);
  const curly = style > 0.3 && style < 0.65 ? 1 : 0;
  const tied = style >= 0.65 ? 1 : 0;
  const swayx = 0.012 * sin(k.s11 * 1.7 + k.seed * 6) + 0.025 * sin(k.f) * (1 - still * 0.5);
  const swayz = 0.01 * sin(k.s11 * 1.1 + k.seed * 3) + 0.02 * abs(sin(k.f));
  const hr = length3(hx, hy, hz);
  // A hat: a kerchief, a cap, a brimmed hat, a wide one, a tall one.
  const hk = clamp(k.v, 0, 1);
  let hat = 10;
  if (hk > 0.08) {
    if (hr > 0.42) {
      hat = hr - 0.32;
    } else if (hk < 0.3) {
      hat = ellipsoid(hx, hy - 0.022, hz + 0.014, 0.104, 0.119, 0.117);
      hat = max(hat, min(hz - 0.02, 0.068 - hy + abs(hx) * 0.25));
      hat = max(hat, -0.06 - hy);
      hat = min(hat, sphere(hx, hy + 0.05, hz + 0.105, 0.022));
    } else if (hk < 0.5) {
      hat = max(ellipsoid(hx, hy - 0.05, hz + 0.004, 0.107, 0.08, 0.117), 0.045 - hy);
      hat = min(hat, max(ellipsoid(hx, hy - 0.05, hz - 0.085, 0.085, 0.009, 0.07), 0.05 - hz));
    } else {
      const wide = hk > 0.7 && hk < 0.9 ? 1 : 0;
      const tall = hk >= 0.9 ? 1 : 0;
      const ch = 0.045 + 0.02 * (1 - wide) + 0.045 * tall;
      hat = cappedCone(hx, hy - 0.05 - ch, hz + 0.005, ch, 0.1, 0.085 + 0.005 * tall);
      const br = 0.15 + 0.11 * wide - 0.02 * tall;
      hat = min(hat, cylinder(hx, hy - 0.052 + length2(hx, hz) * 0.12 * wide, hz + 0.005, br, 0.004));
    }
  }
  let head = hat;
  if (hr > 0.26) {
    head = hr - 0.2;
  } else {
    // Skull, face and jaw; a nose, brow, ears, lips.
    const cranium = ellipsoid(hx, hy - 0.015, hz + 0.01, 0.092, 0.108, 0.106);
    const jaw = ellipsoid(hx, hy + 0.04, hz - 0.03, 0.07 + 0.01 * (1 - fem), 0.075, 0.075);
    head = smoothUnion(cranium, jaw, 0.04);
    const open = clamp(k.s9, 0, 1);
    head = smoothUnion(head, sphere(hx, hy + 0.088 + open * 0.01, hz - 0.066, 0.027 - 0.004 * fem), 0.02);
    // The brow and the eyes under it: deep-set under a strong brow, or
    // shallow under a low flat one.
    const es = clamp(k.y, 0, 1);
    head = smoothUnion(head, capsule(hx, hy, hz, -0.046, 0.04 - 0.003 * es, 0.086 - 0.006 * es, 0.046, 0.04 - 0.003 * es, 0.086 - 0.006 * es, 0.011 - 0.004 * es), 0.012);
    // The eyes: balls the face swells over with no seam round them; the lids
    // are drawn on them by color.
    const ball = length3(abs(hx) - 0.034, hy - 0.011, hz - 0.081) - 0.017;
    head = smoothUnion(head, ball, 0.006);
    // The nose: narrow and high-bridged, or broad and low-bridged.
    const nb = clamp(k.z, 0, 1);
    const nose = capsule(hx, hy, hz, 0, 0.018, 0.099 - 0.016 * nb, 0, -0.022, 0.118 - 0.01 * nb - 0.004 * fem, 0.0100 + 0.004 * nb - 0.002 * fem);
    head = smoothUnion(head, min(nose, ellipsoid(hx, hy + 0.025, hz - 0.1, 0.014 + 0.012 * nb, 0.011 + 0.001 * nb, 0.012)), 0.01);
    head = smoothUnion(head, ellipsoid(abs(hx) - 0.092, hy + 0.0, hz + 0.005, 0.013, 0.03, 0.019), 0.008);
    const lips = min(ellipsoid(hx, hy + 0.051, hz - 0.095, 0.024, 0.0075, 0.01), ellipsoid(hx, hy + 0.066 + open * 0.012, hz - 0.091, 0.022, 0.0085 + 0.002 * fem, 0.011));
    head = smoothUnion(head, lips, 0.006);
    // A beard on the jaw, a moustache above the lip.
    const bd = clamp(k.s, 0, 1) * (1 - fem);
    if (bd > 0.3) {
      let beard = ellipsoid(hx, hy + 0.07 + 0.03 * bd, hz - 0.045, 0.078, 0.04 + 0.05 * bd, 0.068);
      beard = max(beard, hy + 0.04);
      beard = min(beard, ellipsoid(hx, hy + 0.044, hz - 0.098, 0.028, 0.007, 0.01));
      head = smoothUnion(head, beard, 0.01);
    }
    // The mouth, open as they talk.
    head = smoothSubtract(head, ellipsoid(hx, hy + 0.059 + open * 0.007, hz - 0.098, 0.018, 0.0025 + 0.012 * open, 0.016), 0.004);
    // Hair: a cap over the skull (the face left clear), and below it what
    // falls loose or is tied back.
    if (hl > 0.05) {
      const thick = 0.004 + 0.008 * smoothstep(0.1, 0.45, hl) + 0.02 * curly;
      let cap = ellipsoid(hx, hy - 0.022, hz + 0.014, 0.093 + thick, 0.108 + thick, 0.106 + thick);
      cap = max(cap, min(hz - 0.03, 0.058 - hy + abs(hx) * 0.3));
      cap = max(cap, -0.03 - smoothstep(0.15, 0.5, hl) * 0.05 + hz * 0.3 - hy);
      // Under a hat only what shows below it; a kerchief hides it all.
      if (hk > 0.08) {
        cap = max(cap, hk < 0.3 ? 1 : hy - 0.05);
      }
      if (curly > 0) {
        cap = cap - (noise3(hx * 70, hy * 70, hz * 70) - 0.5) * 0.012;
      }
      head = min(head, cap);
    }
  }
  // Long hair falls behind the head and over the back; tied, a bun and a tail.
  if (hl > 0.38 && tied < 1 && hr < 0.9) {
    const len = mix(0.1, 0.62, (hl - 0.38) / 0.62);
    const tt = clamp(-hy / len, 0, 1);
    const dz = hz + 0.05 + 0.065 * clamp(-hy / 0.22, 0, 1) - swayz * tt;
    const dx = hx - swayx * tt;
    let fall = roundBox(dx, hy + len * 0.5 - 0.02, dz, 0.092 + 0.014 * curly + 0.02 * tt, len * 0.5, 0.034 + 0.014 * curly - 0.016 * tt, 0.02);
    fall = max(fall, min(hz - 0.035, 0.058 - hy));
    if (curly > 0 && fall < 0.05) {
      fall = fall - (noise3(hx * 50, hy * 50, hz * 50) - 0.5) * 0.016;
    }
    head = min(head, fall);
  }
  if (hl > 0.3 && tied > 0 && hr < 0.9) {
    head = min(head, sphere(hx, hy - 0.025, hz + 0.115, 0.03 + 0.02 * hl));
    if (hl > 0.55) {
      const tl = 0.12 + 0.35 * hl;
      head = min(head, capsule(hx, hy, hz, 0, -0.03, -0.14, swayx * 2, -0.03 - tl, -0.16 - swayz * 2 - 0.03 * hl, 0.022));
    }
  }
  return smoothUnion(body, head, 0.025) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const w = clamp(k.b, 0.7, 1.4);
  const fem = smoothstep(0.3, 0.7, k.g);
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const px = x / h;
  const py = y / h;
  const pz = z / h;
  const q = rotX(px, py - hip, pz, -clamp(k.s4, -0.3, 0.6));
  const ux = q.x;
  const uy = q.y + 0.9;
  const uz = q.z;
  // Skin: pale … brown … deep brown, rosy or golden beneath.
  const tone = clamp(k.c, 0, 1);
  const warm = clamp(k.x, 0, 1);
  const pale = mix(rgb(246, 210, 196), rgb(240, 212, 166), warm);
  const mid = mix(rgb(176, 120, 100), rgb(184, 132, 82), warm);
  const deep = mix(rgb(62, 40, 36), rgb(72, 48, 28), warm);
  const skin = mix(mix(pale, mid, clamp(tone * 2, 0, 1)), deep, clamp(tone * 2 - 1, 0, 1));
  // Hair: black, browns, auburn, blond, flaxen, then grey.
  const hc = clamp(k.i, 0, 1);
  let hairc = mix(rgb(22, 18, 16), rgb(58, 38, 26), clamp(hc / 0.25, 0, 1));
  hairc = mix(hairc, rgb(105, 70, 42), clamp((hc - 0.25) / 0.2, 0, 1));
  hairc = mix(hairc, rgb(140, 58, 28), clamp((hc - 0.45) / 0.15, 0, 1));
  hairc = mix(hairc, rgb(196, 160, 96), clamp((hc - 0.6) / 0.15, 0, 1));
  hairc = mix(hairc, rgb(224, 210, 170), clamp((hc - 0.75) / 0.15, 0, 1));
  hairc = mix(hairc, rgb(176, 174, 170), clamp((hc - 0.9) / 0.1, 0, 1));
  // Cloth: a hue at a shade; past 1 the colour fades to grey.
  const tgrey = clamp(k.d - 1, 0, 0.2) * 5;
  const topv = mix(0.18, 0.92, clamp(k.k, 0, 1));
  const topc = mix(hsv(k.d, 0.5 * (1 - abs(k.k - 0.5)), topv), rgb(255, 255, 255) * topv, tgrey);
  const bgrey = clamp(k.e - 1, 0, 0.2) * 5;
  const botv = mix(0.15, 0.85, clamp(k.l, 0, 1));
  const botc = mix(hsv(k.e, 0.45 * (1 - abs(k.l - 0.5)), botv), rgb(255, 255, 255) * botv, bgrey);
  const trimc = mix(hsv(k.u, 0.55, 0.55), rgb(120, 116, 110), clamp(k.u - 1, 0, 0.2) * 5);
  const under = mix(rgb(232, 226, 214), trimc, 0.15);
  const tc = clamp(k.m, 0, 1);
  const sk = clamp(k.p, 0, 1);
  const lg = clamp(k.q, 0, 1);
  const sh = clamp(k.r, 0, 1);
  const uw = clamp(k.t, 0, 1);
  const nk = clamp(k.o, 0, 1);
  const band = tc > 0.04 && tc <= 0.28;
  const hem = tc > 0.28 ? mix(1.17, 0.72, (tc - 0.28) / 0.72) : 1.19;
  const topp = topc;
  // The head.
  const nod = clamp(k.s5, -1, 1);
  const hq = rotX(ux, uy - 1.56, uz, nod * 0.35);
  const hx = hq.x;
  const hy = hq.y - 0.06;
  const hz = hq.z;
  const hl = clamp(k.h, 0, 1);
  const style = clamp(k.j, 0, 1);
  const curly = style > 0.3 && style < 0.65 ? 1 : 0;
  const tied = style >= 0.65 ? 1 : 0;
  const still = 1 - clamp(max(k.s2, k.s3) * 2 + crouch, 0, 1);
  const swayx = 0.012 * sin(k.s11 * 1.7 + k.seed * 6) + 0.025 * sin(k.f) * (1 - still * 0.5);
  const swayz = 0.01 * sin(k.s11 * 1.1 + k.seed * 3) + 0.02 * abs(sin(k.f));
  const hr = length3(hx, hy, hz);
  // A hat: a kerchief, a cap, a brimmed hat, a wide one, a tall one.
  const hk = clamp(k.v, 0, 1);
  let hat = 10;
  if (hk > 0.08) {
    if (hr > 0.42) {
      hat = hr - 0.32;
    } else if (hk < 0.3) {
      hat = ellipsoid(hx, hy - 0.022, hz + 0.014, 0.104, 0.119, 0.117);
      hat = max(hat, min(hz - 0.02, 0.068 - hy + abs(hx) * 0.25));
      hat = max(hat, -0.06 - hy);
      hat = min(hat, sphere(hx, hy + 0.05, hz + 0.105, 0.022));
    } else if (hk < 0.5) {
      hat = max(ellipsoid(hx, hy - 0.05, hz + 0.004, 0.107, 0.08, 0.117), 0.045 - hy);
      hat = min(hat, max(ellipsoid(hx, hy - 0.05, hz - 0.085, 0.085, 0.009, 0.07), 0.05 - hz));
    } else {
      const wide = hk > 0.7 && hk < 0.9 ? 1 : 0;
      const tall = hk >= 0.9 ? 1 : 0;
      const ch = 0.045 + 0.02 * (1 - wide) + 0.045 * tall;
      hat = cappedCone(hx, hy - 0.05 - ch, hz + 0.005, ch, 0.1, 0.085 + 0.005 * tall);
      const br = 0.15 + 0.11 * wide - 0.02 * tall;
      hat = min(hat, cylinder(hx, hy - 0.052 + length2(hx, hz) * 0.12 * wide, hz + 0.005, br, 0.004));
    }
  }
  if (hat < 0.006) {
    // A band round the crown.
    return hk > 0.5 && hy < 0.075 && hy > 0.058 ? trimc * 0.55 : trimc * (0.9 + 0.2 * noise3(hx * 90, hy * 90, hz * 90));
  }
  if (hl > 0.38 && tied < 1 && hr < 0.9) {
    const len = mix(0.1, 0.62, (hl - 0.38) / 0.62);
    const tt = clamp(-hy / len, 0, 1);
    const dz = hz + 0.05 + 0.065 * clamp(-hy / 0.22, 0, 1) - swayz * tt;
    const dx = hx - swayx * tt;
    let fall = roundBox(dx, hy + len * 0.5 - 0.02, dz, 0.092 + 0.014 * curly + 0.02 * tt, len * 0.5, 0.034 + 0.014 * curly - 0.016 * tt, 0.02);
    fall = max(fall, min(hz - 0.035, 0.058 - hy));
    if (fall < 0.012 + 0.008 * curly) {
      return hairc * (0.85 + 0.3 * noise3(hx * 40, hy * 200, hz * 40));
    }
  }
  if (hl > 0.3 && tied > 0 && hr < 0.9) {
    const tl = 0.12 + 0.35 * hl;
    const bun = sphere(hx, hy - 0.025, hz + 0.115, 0.03 + 0.02 * hl);
    const tail = hl > 0.55 ? capsule(hx, hy, hz, 0, -0.03, -0.14, swayx * 2, -0.03 - tl, -0.16 - swayz * 2 - 0.03 * hl, 0.022) : 10;
    if (min(bun, tail) < 0.008) {
      return hairc * (0.85 + 0.3 * noise3(hx * 120, hy * 120, hz * 120));
    }
  }
  if (hr < 0.2 && uy > 1.47) {
    const open = clamp(k.s9, 0, 1);
    const shut = clamp(k.s10, 0, 1);
    // Hair on the skull.
    if (hl > 0.05) {
      const thick = 0.004 + 0.008 * smoothstep(0.1, 0.45, hl) + 0.02 * curly;
      let cap = ellipsoid(hx, hy - 0.022, hz + 0.014, 0.093 + thick, 0.108 + thick, 0.106 + thick);
      cap = max(cap, min(hz - 0.03, 0.058 - hy + abs(hx) * 0.3));
      cap = max(cap, -0.03 - smoothstep(0.15, 0.5, hl) * 0.05 + hz * 0.3 - hy);
      if (hk > 0.08) {
        cap = max(cap, hk < 0.3 ? 1 : hy - 0.05);
      }
      if (cap < 0.004 + 0.008 * curly) {
        return hairc * (0.85 + 0.3 * noise3(hx * 50, hy * 160, hz * 50));
      }
    }
    // A little colour in the cheeks (over the lower lids too).
    const cheek = length3(abs(hx) - 0.05, hy + 0.025, hz - 0.07);
    const face = mix(skin, rgb(214, 120, 110), clamp(0.025 - cheek, 0, 0.025) * (6 + 6 * fem));
    // Eyes: white, an iris, the pupil; lids that come down to blink.
    const ex = abs(hx) - 0.034;
    const ey = hy - 0.011;
    const ez = hz - 0.081;
    if (length3(ex, ey, ez) < 0.019 && ez > 0.0) {
      // Narrower eyes tilt up at the outer corner (ex > 0) under a fold.
      const es = clamp(k.y, 0, 1);
      const tilt = ex * 0.15 * es;
      const lid = 0.0068 - 0.0036 * es - shut * 0.019 - abs(ex) * 0.2 + tilt;
      const lidLo = -0.0068 + 0.0016 * es + abs(ex) * 0.15 + tilt;
      // Shut, the lashes meet in a line along the lower lid.
      if (lid + 0.0025 < lidLo + 0.001 && abs(ey - lidLo) < 0.0007 && abs(ex) < 0.013) {
        return rgb(34, 24, 22);
      }
      if (ey > lid + 0.0025 || ey < lidLo) {
        // The lid, shaded only near its edge so it melts into the face; a
        // crease over a round eye's lid, a smooth fold over a narrow one.
        const edge = clamp(max(ey - lid - 0.0025, lidLo - ey) / 0.003, 0, 1);
        const fade = clamp((0.019 - length3(ex, ey, ez)) / 0.004, 0, 1);
        const crease = ey > lid + 0.0045 && ey < lid + 0.0065 ? 0.2 * (1 - es / 0.6) * fade : 0;
        return face * (0.86 + 0.14 * edge - max(crease, 0));
      }
      if (ey > lid - 0.0005) {
        return rgb(34, 24, 22);
      }
      const ir = length2(ex, ey + 0.0005);
      if (ir < 0.0038) {
        return rgb(14, 12, 12);
      }
      if (ir < 0.0082) {
        const ec = clamp(k.w, 0, 1);
        let iris = mix(rgb(34, 22, 16), rgb(92, 58, 34), clamp(ec / 0.25, 0, 1));
        iris = mix(iris, rgb(120, 92, 46), clamp((ec - 0.25) / 0.2, 0, 1));
        iris = mix(iris, rgb(90, 116, 70), clamp((ec - 0.45) / 0.15, 0, 1));
        iris = mix(iris, rgb(112, 120, 126), clamp((ec - 0.6) / 0.2, 0, 1));
        iris = mix(iris, rgb(70, 112, 160), clamp((ec - 0.8) / 0.2, 0, 1));
        return iris * (0.8 + 0.4 * (ir / 0.0082));
      }
      return rgb(236, 230, 222);
    }
    // Brows.
    if (hz > 0.07 && abs(hx) > 0.012 && abs(hx) < 0.056 && abs(hy - 0.04 + abs(hx) * 0.1) < 0.0055 - fem * 0.0015) {
      return mix(hairc, rgb(30, 24, 20), 0.35);
    }
    // The mouth: lips, and inside it the teeth and the dark.
    const my = hy + 0.059 + open * 0.007;
    if (abs(hx) < 0.03 && hz > 0.075 && my > -0.019 - open * 0.01 && my < 0.014) {
      const inside = ellipsoid(hx, my, hz - 0.098, 0.018, 0.0025 + 0.012 * open, 0.016);
      if (inside < 0.002 && hz < 0.096) {
        return my < -0.002 - open * 0.004 ? rgb(70, 22, 26) : rgb(222, 214, 198);
      }
      // Lips: fuller in the middle, a bow on the upper one.
      const lw = 0.025 - 0.004 * fem;
      const lh = (my > 0 ? 0.0085 - 0.0025 * smoothstep(0.0, 0.006, 0.006 - abs(hx)) : 0.0105 + 0.002 * fem) + open * 0.009;
      if ((hx * hx) / (lw * lw) + (my * my) / (lh * lh) < 1) {
        return mix(skin, rgb(176, 78, 82), 0.38 + 0.25 * fem);
      }
    }
    // Beard and moustache; stubble.
    const bd = clamp(k.s, 0, 1) * (1 - fem);
    if (bd > 0.3) {
      let beard = ellipsoid(hx, hy + 0.07 + 0.03 * bd, hz - 0.045, 0.078, 0.04 + 0.05 * bd, 0.068);
      beard = max(beard, hy + 0.04);
      beard = min(beard, ellipsoid(hx, hy + 0.044, hz - 0.098, 0.028, 0.007, 0.01));
      if (beard < 0.004) {
        return hairc * (0.75 + 0.3 * noise3(hx * 90, hy * 150, hz * 90));
      }
    } else if (bd > 0.12 && hy < -0.035 && hz > -0.02) {
      return mix(skin, hairc, 0.3);
    }
    return face;
  }
  // A skirt or a tunic's tail.
  const sw = sin(k.f) * 0.32 * (1 - crouch);
  const kz = crouch * 0.32;
  const tun = tc > 0.82 ? (0.86 - hem) : 0;
  const drop = max(sk > 0.04 ? 0.12 + sk * 0.86 : 0, tun);
  if (drop > 0.01) {
    const top0 = hip + 0.13;
    const r0 = (0.155 + 0.03 * fem) * w;
    const r1 = r0 + 0.03 + 0.2 * min(drop, 0.7) * (1 - crouch * 0.5);
    const hh = drop * 0.5;
    const tt = clamp((top0 - py) / max(drop, 0.01), 0, 1);
    const sz = pz - sw * 0.12 * tt - kz * tt;
    if (cappedCone(px, py - top0 + hh, sz * 1.18, hh, r1, r0) / 1.18 < 0.02) {
      const c = sk > 0.04 && sk * 0.86 + 0.12 >= tun ? botc : topp;
      return py < top0 - drop + 0.02 ? mix(c, trimc, 0.5) : c;
    }
  }
  // Feet and shoes.
  if (py < 0.13) {
    if (sh > 0.3) {
      return mix(rgb(52, 38, 30), trimc, 0.3) * (py < 0.022 ? 0.6 : 1.0);
    }
    if (sh > 0.08 && py < 0.03) {
      return rgb(96, 70, 48);
    }
    return skin;
  }
  // Arms: sleeve as far as it goes, then the hand.
  if (py > 0.6) {
    const sx = (0.24 - 0.035 * fem) * w;
    const sl = tc > 0.28 ? 0.04 + clamp(k.n, 0, 1) * 0.92 : 0;
    const sl0 = nk > 0.85 ? 0.14 : 0;
    for (let i = 0; i < 2; i++) {
      const side = i * 2 - 1;
      const ra = clamp(i == 0 ? k.s0 : k.s1, 0, 1) * 3.0;
      const rf = clamp(i == 0 ? k.s2 : k.s3, 0, 1) * 1.45;
      const hx2 = side * (sx + 0.04 * w + 0.56 * sin(ra));
      const hy2 = 1.42 - 0.56 * cos(ra) * cos(rf);
      const hz2 = 0.56 * cos(ra) * sin(rf) + side * sw * 0.8 * still;
      const ax = hx2 - side * sx;
      const ay = hy2 - 1.42;
      const al2 = max(ax * ax + ay * ay + hz2 * hz2, 0.0001);
      const t = clamp(((ux - side * sx) * ax + (uy - 1.42) * ay + uz * hz2) / al2, 0, 1.4);
      const off = length3(ux - side * sx - ax * min(t, 1), uy - 1.42 - ay * min(t, 1), uz - hz2 * min(t, 1));
      if (off < 0.066 * w || (t > 0.95 && off < 0.15)) {
        if (sl <= 0 || t > sl || t < sl0) {
          return skin;
        }
        return mix(topp, trimc, t > sl - 0.035 && sl < 0.9 ? 0.6 : 0);
      }
    }
  }
  // Legs: trousers as far as they go, boots up the calf.
  if (py < hip - 0.04) {
    const legEnd = lg > 0.04 ? 0.88 - lg * 0.8 : 9;
    const boot = sh > 0.55 ? 0.1 + (sh - 0.55) * 0.95 : 0.0;
    if (py < boot) {
      return mix(rgb(52, 38, 30), trimc, 0.3);
    }
    if (py > legEnd) {
      return py < legEnd + 0.02 ? botc * 0.85 : botc;
    }
    if (uw > 0.5 && py > hip - 0.1) {
      return under;
    }
    return skin;
  }
  // The torso: top, then the waistband and what is below it.
  // Up over the shoulders to a hole round the neck (wider as the neckline
  // opens), dipping in front; a high collar rises up the neck.
  let top = length2(ux, uz + 0.01) > 0.068 + 0.03 * nk ? 1.62 : 1.45 + max(0.1 - nk, 0) * 0.8;
  if (uz > 0) {
    // Round or scooped, then a V as it plunges.
    const nd = 0.02 + nk * 0.22;
    const nw = 0.06 + 0.1 * nk;
    top = min(top, nk < 0.55 ? (abs(ux) < nw ? 1.47 - nd * (1 - (ux * ux) / (nw * nw)) : 1.62) : 1.47 - nd + abs(ux) * nd / nw);
  }
  if (nk > 0.85 || band) {
    top = 1.34;
  }
  if (tc > 0.04 && uy > hem && uy < top) {
    if (uy > top - 0.018) {
      return mix(topp, trimc, 0.5);
    }
    return topp;
  }
  if ((lg > 0.04 || sk > 0.04) && uy < 1.03) {
    return uy > 0.99 ? trimc * 0.8 : botc;
  }
  if (uw > 0.5 && (uy < 0.97 || (fem > 0.5 && tc <= 0.04 && uy > 1.19 && uy < 1.33))) {
    return under;
  }
  return skin;
}

export function parts(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const w = clamp(k.b, 0.7, 1.4);
  const fem = smoothstep(0.3, 0.7, k.g);
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  // In the torso's own frame (leaning with it), as sdf() draws them.
  let u = vec3(0, 1.25, 0.02);
  if (x < 1.5) {
    const side = x < 0.5 ? 1 : -1;
    const a1 = (x < 0.5 ? clamp(k.s1, 0, 1) : clamp(k.s0, 0, 1)) * 3.0;
    const f1 = (x < 0.5 ? clamp(k.s3, 0, 1) : clamp(k.s2, 0, 1)) * 1.45;
    const still = 1 - clamp(max(k.s2, k.s3) * 2 + crouch, 0, 1);
    const sw = sin(k.f) * 0.32 * (1 - crouch);
    const sx = (0.24 - 0.035 * fem) * w;
    u = vec3(side * (sx + 0.04 * w + 0.56 * sin(a1)), 1.42 - 0.56 * cos(a1) * cos(f1), 0.56 * cos(a1) * sin(f1) + side * sw * 0.8 * still);
  } else if (x < 2.5) {
    const c = rotX(0, 0.06, 0, -clamp(k.s5, -1, 1) * 0.35);
    u = vec3(c.x, c.y + 1.56, c.z);
  }
  const p = rotX(u.x, u.y - 0.9, u.z, clamp(k.s4, -0.3, 0.6));
  return vec3(p.x, p.y + hip, p.z) * h;
}
