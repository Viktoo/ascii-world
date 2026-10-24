// Built-in night walker: the dark's own body. Too tall, hunched, all bone,
// arms hanging nearly to the ground, legs that bend the wrong way, a long
// faceless skull with two red eyes that glow, slit-narrow and sunk into it
// (out when it dies); a knotted spine. Dead, the stoop goes out of it and it
// lies flat. Faces +z.
// Look: a = height (m), b = build (gaunt … thin), c = shade (black … soot),
// d = eye glow. f = walk phase (radians).
// Roles: s0/s1 = left/right arm raised sideways, s2/s3 = left/right arm
// reaching forward, s4 = lean (more hunched; -0.5 straightens it), s5 = head
// down, s6 = crouch. s10 = eyes shut (the eyes go out).
export const meta = { name: "night walker", bounds: [1.1, 2.8, 1.3], tags: ["builtin", "body"],
  props: { mass: 60, alive: 1 },
  body: { height: 2.3, eye: 1.85, radius: 0.35, reach: 1.6, grip: [0.25, 0.4, 0.5],
          roles: ["raise", "reach", "lean", "head", "crouch"], gait: "biped", arms: true,
          dead: { raise_l: 0.25, raise_r: 0.35, lean: -0.5, head: -0.6 },
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
  const lean = 0.5 + clamp(k.s4, -0.5, 0.6);
  const q = rotX(px, py - hip, pz, -lean);
  const spine = capsule(q.x, q.y, q.z, 0, 0, 0, 0, 0.72, 0, 0.07 * w);
  const ribs = ellipsoid(q.x, q.y - 0.5, q.z, 0.17 * w, 0.22, 0.11 * w);
  // A long skull jutting forward on a thin neck.
  const nod = clamp(k.s5, -1, 1);
  const hy = 0.86 - nod * 0.08;
  const hz = 0.2 + nod * 0.03;
  const neck = capsule(q.x, q.y, q.z, 0, 0.72, 0, 0, hy, hz - 0.08, 0.04);
  let head = ellipsoid(q.x, q.y - hy, q.z - hz, 0.085, 0.1, 0.17);
  // A ridge of brow over the eyes, and sockets under it.
  head = smoothUnion(head, capsule(q.x, q.y - hy, q.z - hz, -0.055, 0.03, 0.115, 0.055, 0.03, 0.115, 0.016), 0.02);
  head = smoothSubtract(head, ellipsoid(abs(q.x) - 0.042, q.y - hy - 0.0, q.z - hz - 0.14, 0.04, 0.022, 0.034), 0.006);
  // Two slits of light in them, tipped in toward the middle, big enough
  // to be seen across a field at night.
  const eyes = ellipsoid(abs(q.x) - 0.042, q.y - hy + 0.001 - (abs(q.x) - 0.042) * 0.35, q.z - hz - 0.122, 0.036, 0.012, 0.016);
  // Knuckles of spine down the back.
  const knots = sphere(q.x, q.y - clamp(round(q.y * 14) / 14, 0.07, 0.65), q.z + 0.055 * w, 0.022 * w);
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
  const torso = smoothUnion(smoothUnion(spine, ribs, 0.05), knots, 0.02);
  const upper = smoothUnion(torso, smoothUnion(neck, head, 0.03), 0.03);
  const body = smoothUnion(smoothUnion(upper, legs, 0.05), min(arms, claws), 0.03);
  return min(body, eyes) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 2.1, 2.6) / 2.3;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 1.05 - crouch * 0.45;
  const lean = 0.5 + clamp(k.s4, -0.5, 0.6);
  const q = rotX(x / h, y / h - hip, z / h, -lean);
  const nod = clamp(k.s5, -1, 1);
  const hy = 0.86 - nod * 0.08;
  const hz = 0.2 + nod * 0.03;
  // The eyes are the only light on it.
  const ex = abs(q.x) - 0.042;
  const ey = q.y - hy + 0.001 - ex * 0.35;
  const ez = q.z - hz - 0.122;
  if ((ex * ex) / 0.0016 + (ey * ey) / 0.0002 + (ez * ez) / 0.0004 < 1) {
    if (k.s10 > 0.5) {
      return rgb(8, 6, 6);
    }
    return glow(rgb(255, 26 + 30 * hash(k.seed), 14), clamp(k.d, 0.7, 1));
  }
  const v = 0.03 + 0.07 * clamp(k.c, 0, 1);
  return hsv(0.02 * hash(k.seed, 2), 0.12, v + 0.02 * noise3(x * 9, y * 9, z * 9 + k.seed));
}
