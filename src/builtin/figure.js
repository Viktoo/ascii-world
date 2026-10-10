// Built-in human body (and the template other upright peoples' bodies are
// written from). Look: a = height (m), b = build, c = skin tone,
// d = shirt hue, e = trousers hue, f = walk phase (radians). Faces +z.
// Roles (live pose): s0/s1 = left/right arm raised sideways (0 down … 1 up),
// s2/s3 = left/right arm reaching forward (0 … 1), s4 = lean forward (radians),
// s5 = head nod, s6 = crouch (0 … 1), s8 = grip (0 open hands … 1 fists).
export const meta = { name: "figure", bounds: [0.95, 2.4, 0.95], tags: ["builtin", "figure", "body"],
  body: { height: 1.75, eye: 1.65, radius: 0.35, reach: 2.4, grip: [0.3, 1.0, 0.3],
          roles: ["raise", "reach", "lean", "head", "crouch", "grip"], gait: "biped", arms: true,
          look: { height: [1.3, 2.0], build: [0.7, 1.4], skin: [0, 1], shirt: [0, 1], trousers: [0, 1] } } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const w = clamp(k.b, 0.7, 1.4);
  const px = x / h;
  const py = y / h;
  const pz = z / h;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const still = 1 - clamp(max(k.s2, k.s3) * 2 + crouch, 0, 1);
  const sw = sin(k.f) * 0.32 * (1 - crouch);
  const kz = crouch * 0.32;
  const ky = 0.5 - crouch * 0.12;
  const legs = min(
    capsule(px, py, pz, -0.1 * w, hip, 0, -0.1 * w, ky, kz + sw * 0.5, 0.075 * w),
    capsule(px, py, pz, -0.1 * w, ky, kz + sw * 0.5, -0.1 * w, 0.09, sw, 0.07 * w),
    capsule(px, py, pz, 0.1 * w, hip, 0, 0.1 * w, ky, kz - sw * 0.5, 0.075 * w),
    capsule(px, py, pz, 0.1 * w, ky, kz - sw * 0.5, 0.1 * w, 0.09, -sw, 0.07 * w)
  );
  // Upper body: lean forward around the hip.
  const q = rotX(px, py - hip, pz, -clamp(k.s4, -0.3, 0.6));
  const ux = q.x;
  const uy = q.y + 0.9;
  const uz = q.z;
  const torso = roundBox(ux, uy - 1.2, uz, 0.18 * w, 0.28, 0.11 * w, 0.07);
  const la = clamp(k.s0, 0, 1) * 3.0;
  const ra = clamp(k.s1, 0, 1) * 3.0;
  const lf = clamp(k.s2, 0, 1) * 1.45;
  const rf = clamp(k.s3, 0, 1) * 1.45;
  const lhx = -0.25 * w - 0.04 * w - 0.56 * sin(la);
  const lhy = 1.42 - 0.56 * cos(la) * cos(lf);
  const lhz = 0.56 * cos(la) * sin(lf) - sw * 0.8 * still;
  const rhx = 0.25 * w + 0.04 * w + 0.56 * sin(ra);
  const rhy = 1.42 - 0.56 * cos(ra) * cos(rf);
  const rhz = 0.56 * cos(ra) * sin(rf) + sw * 0.8 * still;
  // Each arm runs from the shoulder to the wrist; past it, a hand: palm,
  // fingers (two pairs) curling under as the grip closes, and a thumb.
  // Drawn in full only near the hand; from afar, a knob.
  const g = clamp(k.s8, 0, 1);
  let arms = 10;
  for (let i = 0; i < 2; i++) {
    const side = i * 2 - 1;
    const hx = i == 0 ? lhx : rhx;
    const hy = i == 0 ? lhy : rhy;
    const hz = i == 0 ? lhz : rhz;
    const sx = side * 0.25 * w;
    const al = max(length3(hx - sx, hy - 1.42, hz), 0.001);
    const dx = (hx - sx) / al;
    const dy = (hy - 1.42) / al;
    const dz = hz / al;
    arms = min(arms, capsule(ux, uy, uz, sx, 1.42, 0, hx - dx * 0.06, hy - dy * 0.06, hz - dz * 0.06, 0.052 * w));
    const near = length3(ux - hx, uy - hy, uz - hz);
    if (near > 0.22) {
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
      // A flat palm: two rods side by side.
      const pw = 0.016 * w;
      const palm = min(
        capsule(ux, uy, uz, hx - dx * 0.04 - bx * pw, hy - dy * 0.04 - by * pw, hz - dz * 0.04 - bz * pw, hx + dx * 0.035 - bx * pw, hy + dy * 0.035 - by * pw, hz + dz * 0.035 - bz * pw, 0.027 * w),
        capsule(ux, uy, uz, hx - dx * 0.04 + bx * pw, hy - dy * 0.04 + by * pw, hz - dz * 0.04 + bz * pw, hx + dx * 0.035 + bx * pw, hy + dy * 0.035 + by * pw, hz + dz * 0.035 + bz * pw, 0.027 * w)
      );
      const a1 = g * 1.5;
      const a2 = g * 3.1;
      const c1 = cos(a1);
      const s1 = sin(a1);
      const c2 = cos(a2);
      const s2 = sin(a2);
      let fingers = 10;
      for (let j = 0; j < 2; j++) {
        const o = (j - 0.5) * 0.034 * w;
        const kx = hx + dx * 0.04 + bx * o;
        const ky = hy + dy * 0.04 + by * o;
        const kz = hz + dz * 0.04 + bz * o;
        const mx = kx + (dx * c1 + nx * s1) * 0.045;
        const my = ky + (dy * c1 + ny * s1) * 0.045;
        const mz = kz + (dz * c1 + nz * s1) * 0.045;
        const tx = mx + (dx * c2 + nx * s2) * 0.04;
        const ty = my + (dy * c2 + ny * s2) * 0.04;
        const tz = mz + (dz * c2 + nz * s2) * 0.04;
        fingers = min(fingers, min(capsule(ux, uy, uz, kx, ky, kz, mx, my, mz, 0.016 * w), capsule(ux, uy, uz, mx, my, mz, tx, ty, tz, 0.015 * w)));
      }
      // The thumb sits on the side toward the body; it folds across a fist.
      const tbx = hx - bx * side * 0.03 * w;
      const tby = hy - by * side * 0.03 * w;
      const tbz = hz - bz * side * 0.03 * w;
      const ox = dx * 0.045 - bx * side * 0.03;
      const oy = dy * 0.045 - by * side * 0.03;
      const oz = dz * 0.045 - bz * side * 0.03;
      const fx = dx * 0.04 + nx * 0.035 + bx * side * 0.02;
      const fy = dy * 0.04 + ny * 0.035 + by * side * 0.02;
      const fz = dz * 0.04 + nz * 0.035 + bz * side * 0.02;
      const thumb = capsule(ux, uy, uz, tbx, tby, tbz, tbx + mix(ox, fx, g), tby + mix(oy, fy, g), tbz + mix(oz, fz, g), 0.016 * w);
      arms = min(arms, smoothUnion(palm, min(fingers, thumb), 0.012));
    }
  }
  const nod = clamp(k.s5, -1, 1);
  const hy = uy - 1.62 + nod * 0.03;
  const hz = uz - nod * 0.05;
  const head = sphere(ux, hy, hz, 0.12);
  const hair = sphere(ux, hy - 0.04, hz + 0.015, 0.125);
  const nose = sphere(ux, hy + 0.01, hz - 0.115, 0.025);
  const body = smoothUnion(min(legs, arms), torso, 0.06);
  return min(smoothUnion(body, min(head, hair), 0.05), nose) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const q = rotX(x / h, y / h - hip, z / h, -clamp(k.s4, -0.3, 0.6));
  const py = y / h;
  const uy = q.y + 0.9;
  const uz = q.z;
  const nod = clamp(k.s5, -1, 1);
  const skin = mix(rgb(244, 208, 178), rgb(96, 62, 42), clamp(k.c, 0, 1));
  const hairc = mix(rgb(30, 22, 18), rgb(200, 160, 90), hash(k.seed) * hash(k.seed, 1));
  if (uy > 1.52 + nod * 0.03 && py > hip) {
    if (uy > 1.69 + nod * 0.03 || (uz < -0.02 + nod * 0.05 && uy > 1.58)) {
      return hairc;
    }
    return skin;
  }
  if (py < 0.13) {
    return rgb(46, 36, 30);
  }
  // Arms: sleeve to the wrist (or the elbow, for some), then the hand.
  if (py > hip) {
    const w = clamp(k.b, 0.7, 1.4);
    const ux = q.x;
    const still = 1 - clamp(max(k.s2, k.s3) * 2 + crouch, 0, 1);
    const sw = sin(k.f) * 0.32 * (1 - crouch);
    const short = hash(k.seed, 3) < 0.4 ? 0.5 : 0.86;
    for (let i = 0; i < 2; i++) {
      const side = i * 2 - 1;
      const ra = clamp(i == 0 ? k.s0 : k.s1, 0, 1) * 3.0;
      const rf = clamp(i == 0 ? k.s2 : k.s3, 0, 1) * 1.45;
      const sx = side * 0.25 * w;
      const hx = side * (0.29 * w + 0.56 * sin(ra));
      const hy = 1.42 - 0.56 * cos(ra) * cos(rf);
      const hz = 0.56 * cos(ra) * sin(rf) + side * sw * 0.8 * still;
      const ax = hx - sx;
      const ay = hy - 1.42;
      const al2 = max(ax * ax + ay * ay + hz * hz, 0.0001);
      const t = clamp(((ux - sx) * ax + (uy - 1.42) * ay + uz * hz) / al2, 0, 1.4);
      const off = length3(ux - sx - ax * min(t, 1), uy - 1.42 - ay * min(t, 1), uz - hz * min(t, 1));
      if (off < 0.075 * w || (t > 0.95 && off < 0.15)) {
        if (t > short) {
          return skin;
        }
        return mix(hsv(k.d, 0.55, 0.78), rgb(30, 26, 22), t > short - 0.04 ? 0.25 : 0);
      }
    }
  }
  if (py >= hip) {
    return hsv(k.d, 0.55, 0.78);
  }
  return hsv(k.e, 0.35, 0.42);
}
