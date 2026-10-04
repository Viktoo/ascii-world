// A cat, written from the quadruped: low and long, a small round head with
// a short muzzle, tall pointed ears, a long tail that curls up. Faces +z.
// Look: a = legs, b = tail thickness (sleek … fluffy), c = ears, d = coat
// hue (1 … 1.2 fades to grey), e = coat shade. f = walk phase (radians).
// Roles: s0/s1 = paw raised, s2/s3 = paw reaching, s4 = lean (stretch),
// s5 = head down, s6 = crouch (sit … curl up), s7 = tail up / swish.
export const meta = { name: "cat", bounds: [0.16, 0.5, 0.52], tags: ["body"],
  props: { mass: 4.5, alive: 1, burns: 0.2 },
  body: { from: "quadruped", height: 0.32, eye: 0.27, radius: 0.12, reach: 0.4, grip: [0, 0.2, 0.25],
          roles: ["raise", "reach", "lean", "head", "crouch", "spread"], gait: "quad", arms: false,
          look: { legs: [0, 1], tail: [0, 1], ears: [0, 1], hue: [0, 1.2], shade: [0, 1] } } };

export function sdf(x, y, z, k) {
  const S = 0.34;
  const px = x / S;
  const py = y / S;
  const pz = z / S;
  const leg = 0.3 + clamp(k.a, 0, 1) * 0.12;
  const rz = 0.42;
  const crouch = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, 0, 0.6);
  const sw = sin(k.f) * 0.1 * (1 - crouch);
  const hipY = leg + 0.1 - crouch * (leg - 0.06);
  const shY = leg + 0.08 - crouch * (leg - 0.16) * 0.6 - lean * 0.3;
  const zs = rz * 0.66;
  const zh = -rz * 0.66;
  const tilt = atan2(hipY - shY, zs - zh);
  const q = rotX(px, py - (hipY + shY) * 0.5, pz, tilt);
  const torso = ellipsoid(q.x, q.y, q.z, 0.14, 0.13, rz);
  const fly = 0.04 + clamp(k.s0, 0, 1) * 0.22 + clamp(k.s2, 0, 1) * 0.12;
  const fry = 0.04 + clamp(k.s1, 0, 1) * 0.22 + clamp(k.s3, 0, 1) * 0.12;
  const fold = crouch * 0.16;
  const legs = min(
    capsule(px, py, pz, -0.08, shY - 0.05, zs, -0.08, fly, zs + sw + clamp(k.s2, 0, 1) * 0.28, 0.04),
    capsule(px, py, pz, 0.08, shY - 0.05, zs, 0.08, fry, zs - sw + clamp(k.s3, 0, 1) * 0.28, 0.04),
    capsule(px, py, pz, -0.08, hipY - 0.05, zh, -0.08, 0.04, zh - sw + fold, 0.045),
    capsule(px, py, pz, 0.08, hipY - 0.05, zh, 0.08, 0.04, zh + sw + fold, 0.045)
  );
  // A small round head on a short neck, a little muzzle.
  const hd = clamp(k.s5, -0.5, 1);
  const hy = mix(shY + 0.2, 0.12, max(hd, 0)) - min(hd, 0) * 0.08;
  const hz = zs + 0.12 + max(hd, 0) * 0.12;
  const neck = capsule(px, py, pz, 0, shY + 0.02, zs - 0.02, 0, hy - 0.02, hz - 0.02, 0.06);
  const head = sphere(px, py - hy, pz - hz, 0.1);
  const muzzle = ellipsoid(px, py - hy + 0.035, pz - hz - 0.08, 0.05, 0.035, 0.04);
  // Tall pointed ears: cones that tip out a little.
  const eh = 0.07 + clamp(k.c, 0, 1) * 0.07;
  const el = rotZ(px + 0.06, py - hy - 0.07 - eh * 0.5, pz - hz + 0.01, -0.3);
  const er = rotZ(px - 0.06, py - hy - 0.07 - eh * 0.5, pz - hz + 0.01, 0.3);
  const ears = min(cappedCone(el.x, el.y, el.z, eh * 0.5, 0.045, 0.004), cappedCone(er.x, er.y, er.z, eh * 0.5, 0.045, 0.004));
  // A long tail: up from the rump, curling forward at the tip when raised.
  const sp = clamp(k.s7, -1, 1);
  const up = 0.25 + abs(sp) * 1.0;
  const tr = 0.028 + clamp(k.b, 0, 1) * 0.05;
  const tx = sp * 0.1;
  const t0z = zh - rz * 0.3;
  const t1y = hipY + 0.05 + sin(up) * 0.3;
  const t1z = t0z - cos(up) * 0.3;
  const tail = min(
    capsule(px, py, pz, 0, hipY + 0.05, t0z, tx, t1y, t1z, tr),
    capsule(px, py, pz, tx, t1y, t1z, tx * 1.6 + sin(k.f * 0.5) * 0.03, t1y + 0.12 + abs(sp) * 0.08, t1z + 0.04 + abs(sp) * 0.1, tr * 0.9)
  );
  const body = smoothUnion(smoothUnion(torso, legs, 0.04), smoothUnion(neck, head, 0.04), 0.04);
  return min(smoothUnion(smoothUnion(body, muzzle, 0.03), ears, 0.02), smoothUnion(tail, torso, 0.03)) * S;
}

export function color(x, y, z, k) {
  const S = 0.34;
  const py = y / S;
  const lit = clamp(k.e, 0, 1);
  const grey = clamp((k.d - 1) * 5, 0, 1);
  const sat = (0.3 + 0.35 * hash(k.seed, 3)) * (1 - grey) * (1 - clamp((lit - 0.85) * 6.6, 0, 1)) * clamp(lit * 8, 0, 1);
  const coat = hsv(k.d, sat, 0.08 + 0.87 * lit);
  const light = mix(coat, rgb(240, 232, 220), 0.6);
  // Tabby stripes on some, a pale chest and paws on others.
  if (hash(k.seed, 9) > 0.5 && sin(z / S * 40 + noise3(x * 30, y * 30, z * 30) * 2) > 0.55) {
    return mix(coat, rgb(20, 16, 14), 0.45);
  }
  if (py < 0.08 && hash(k.seed, 5) > 0.4) {
    return light;
  }
  return coat;
}
