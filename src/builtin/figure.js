// Built-in human body (and the template other upright peoples' bodies are
// written from). Look: a = height (m), b = build, c = skin tone,
// d = shirt hue, e = trousers hue, f = walk phase (radians). Faces +z.
// Roles (live pose): s0/s1 = left/right arm raised sideways (0 down … 1 up),
// s2/s3 = left/right arm reaching forward (0 … 1), s4 = lean forward (radians),
// s5 = head nod, s6 = crouch (0 … 1).
export const meta = { name: "figure", bounds: [0.95, 2.4, 0.95], tags: ["builtin", "figure", "body"],
  body: { height: 1.75, eye: 1.65, radius: 0.35, reach: 2.4, grip: [0.3, 1.0, 0.3],
          roles: ["raise", "reach", "lean", "head", "crouch"], gait: "biped", arms: true,
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
  const arms = min(
    capsule(ux, uy, uz, -0.25 * w, 1.42, 0, lhx, lhy, lhz, 0.055 * w),
    capsule(ux, uy, uz, 0.25 * w, 1.42, 0, rhx, rhy, rhz, 0.055 * w)
  );
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
  if (py > hip && uy < 1.36 && abs(q.x) > 0.2) {
    return skin;
  }
  if (py >= hip) {
    return hsv(k.d, 0.55, 0.78);
  }
  return hsv(k.e, 0.35, 0.42);
}
