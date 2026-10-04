// Built-in night walker: the dark's own body. Too tall, hunched, all bone,
// arms hanging nearly to the ground, legs that bend the wrong way, a long
// faceless skull with two red eyes that glow. Faces +z.
// Look: a = height (m), b = build (gaunt … thin), c = shade (black … soot),
// d = eye glow. f = walk phase (radians).
// Roles: s0/s1 = left/right arm raised sideways, s2/s3 = left/right arm
// reaching forward, s4 = lean (more hunched), s5 = head down, s6 = crouch.
export const meta = { name: "night walker", bounds: [1.1, 2.8, 1.3], tags: ["builtin", "body"],
  props: { mass: 60, alive: 1 },
  body: { height: 2.3, eye: 1.85, radius: 0.35, reach: 1.6, grip: [0.25, 0.4, 0.5],
          roles: ["raise", "reach", "lean", "head", "crouch"], gait: "biped", arms: true,
          look: { height: [2.1, 2.6], build: [0.75, 1.0], shade: [0, 1], eyes: [0.7, 1] } } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 2.1, 2.6) / 2.3;
  const w = clamp(k.b, 0.75, 1.0);
  const px = x / h;
  const py = y / h;
  const pz = z / h;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 1.05 - crouch * 0.45;
  const sw = sin(k.f) * 0.28 * (1 - crouch);
  // Legs bend backwards: hip, knee forward, ankle back, long toes.
  const kn = hip * 0.58;
  const legs = min(
    capsule(px, py, pz, -0.1 * w, hip, 0, -0.12 * w, kn, 0.14 + sw * 0.4, 0.05 * w),
    capsule(px, py, pz, -0.12 * w, kn, 0.14 + sw * 0.4, -0.12 * w, 0.18, -0.12 + sw, 0.04 * w),
    capsule(px, py, pz, -0.12 * w, 0.18, -0.12 + sw, -0.12 * w, 0.03, 0.1 + sw, 0.03),
    capsule(px, py, pz, 0.1 * w, hip, 0, 0.12 * w, kn, 0.14 - sw * 0.4, 0.05 * w),
    capsule(px, py, pz, 0.12 * w, kn, 0.14 - sw * 0.4, 0.12 * w, 0.18, -0.12 - sw, 0.04 * w),
    capsule(px, py, pz, 0.12 * w, 0.18, -0.12 - sw, 0.12 * w, 0.03, 0.1 - sw, 0.03)
  );
  // Hunched: the spine leans forward from the hip.
  const lean = 0.5 + clamp(k.s4, -0.3, 0.6);
  const q = rotX(px, py - hip, pz, -lean);
  const spine = capsule(q.x, q.y, q.z, 0, 0, 0, 0, 0.72, 0, 0.07 * w);
  const ribs = ellipsoid(q.x, q.y - 0.5, q.z, 0.17 * w, 0.22, 0.11 * w);
  // A long skull jutting forward on a thin neck.
  const nod = clamp(k.s5, -1, 1);
  const hy = 0.86 - nod * 0.08;
  const hz = 0.2 + nod * 0.03;
  const neck = capsule(q.x, q.y, q.z, 0, 0.72, 0, 0, hy, hz - 0.08, 0.04);
  const head = ellipsoid(q.x, q.y - hy, q.z - hz, 0.085, 0.1, 0.17);
  const eyes = min(sphere(q.x - 0.045, q.y - hy - 0.02, q.z - hz - 0.13, 0.025), sphere(q.x + 0.045, q.y - hy - 0.02, q.z - hz - 0.13, 0.025));
  // Arms hang from the shoulders, nearly to the ground.
  const sy = hip + 0.68 * cos(lean);
  const sz = 0.68 * sin(lean);
  const la = clamp(k.s0, 0, 1) * 2.6;
  const ra = clamp(k.s1, 0, 1) * 2.6;
  const lf = clamp(k.s2, 0, 1) * 1.4;
  const rf = clamp(k.s3, 0, 1) * 1.4;
  const arm = 1.05;
  const lhx = -0.2 * w - arm * sin(la);
  const lhy = max(sy - arm * cos(la) * cos(lf), 0.12);
  const lhz = sz + arm * cos(la) * sin(lf) + 0.1 - sw * 0.5;
  const rhx = 0.2 * w + arm * sin(ra);
  const rhy = max(sy - arm * cos(ra) * cos(rf), 0.12);
  const rhz = sz + arm * cos(ra) * sin(rf) + 0.1 + sw * 0.5;
  const arms = min(
    capsule(px, py, pz, -0.2 * w, sy, sz, (lhx - 0.2 * w) * 0.5 - 0.06, (sy + lhy) * 0.5, (sz + lhz) * 0.5 - 0.05, 0.035 * w),
    capsule(px, py, pz, (lhx - 0.2 * w) * 0.5 - 0.06, (sy + lhy) * 0.5, (sz + lhz) * 0.5 - 0.05, lhx, lhy, lhz, 0.03 * w),
    capsule(px, py, pz, 0.2 * w, sy, sz, (rhx + 0.2 * w) * 0.5 + 0.06, (sy + rhy) * 0.5, (sz + rhz) * 0.5 - 0.05, 0.035 * w),
    capsule(px, py, pz, (rhx + 0.2 * w) * 0.5 + 0.06, (sy + rhy) * 0.5, (sz + rhz) * 0.5 - 0.05, rhx, rhy, rhz, 0.03 * w)
  );
  // Long fingers, two to a hand.
  const claws = min(
    capsule(px, py, pz, lhx, lhy, lhz, lhx - 0.04, max(lhy - 0.2, 0.02), lhz + 0.06, 0.012),
    capsule(px, py, pz, lhx, lhy, lhz, lhx + 0.03, max(lhy - 0.2, 0.02), lhz + 0.08, 0.012),
    capsule(px, py, pz, rhx, rhy, rhz, rhx + 0.04, max(rhy - 0.2, 0.02), rhz + 0.06, 0.012),
    capsule(px, py, pz, rhx, rhy, rhz, rhx - 0.03, max(rhy - 0.2, 0.02), rhz + 0.08, 0.012)
  );
  const torso = smoothUnion(spine, ribs, 0.05);
  const upper = smoothUnion(torso, smoothUnion(neck, head, 0.03), 0.03);
  const body = smoothUnion(smoothUnion(upper, legs, 0.05), min(arms, claws), 0.03);
  return min(body, eyes) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 2.1, 2.6) / 2.3;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 1.05 - crouch * 0.45;
  const lean = 0.5 + clamp(k.s4, -0.3, 0.6);
  const q = rotX(x / h, y / h - hip, z / h, -lean);
  const nod = clamp(k.s5, -1, 1);
  const hy = 0.86 - nod * 0.08;
  const hz = 0.2 + nod * 0.03;
  // The eyes are the only light on it.
  const ex = abs(q.x) - 0.045;
  const ey = q.y - hy - 0.02;
  const ez = q.z - hz - 0.13;
  if (ex * ex + ey * ey + ez * ez < 0.0016) {
    return glow(rgb(255, 26 + 30 * hash(k.seed), 14), clamp(k.d, 0.7, 1));
  }
  const v = 0.03 + 0.07 * clamp(k.c, 0, 1);
  return hsv(0.02 * hash(k.seed, 2), 0.12, v + 0.02 * noise3(x * 9, y * 9, z * 9 + k.seed));
}
