// Built-in four-legged body (dogs, cats, horses, wolves, goats, deer…), about
// 1 m to the top of the head at scale 1; species set the scale. Faces +z.
// Look: a = leg length, b = body length, c = ears (short … tall), d = coat
// hue, e = coat shade (dark … light). f = walk phase (radians).
// Roles: s0/s1 = left/right front paw raised, s2/s3 = left/right front leg
// reaching forward, s4 = lean (front down, as in a play bow), s5 = head down
// (grazing, nodding), s6 = crouch (sit … lie), s7 = spread (tail up / wag).
export const meta = { name: "quadruped", bounds: [0.36, 1.2, 0.98], tags: ["builtin", "body"],
  props: { mass: 30, alive: 1, burns: 0.2 },
  body: { height: 1.0, eye: 0.82, radius: 0.3, reach: 0.9, grip: [0, 0.5, 0.66], seat: [0, 0.74, -0.02],
          roles: ["raise", "reach", "lean", "head", "crouch", "spread"], gait: "quad", arms: false,
          look: { legs: [0, 1], length: [0, 1], ears: [0, 1], hue: [0, 1], shade: [0, 1] } } };

export function sdf(x, y, z, k) {
  const leg = 0.34 + clamp(k.a, 0, 1) * 0.26;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.15;
  const crouch = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, 0, 0.6);
  const sw = sin(k.f) * 0.11 * (1 - crouch);
  const hipY = leg + 0.1 - crouch * (leg - 0.08);
  const shY = leg + 0.1 - crouch * (leg - 0.2) * 0.6 - lean * 0.3;
  const zs = rz * 0.68;
  const zh = -rz * 0.68;
  // Torso: an ellipsoid tilted from hips to shoulders.
  const tilt = atan2(hipY - shY, zs - zh);
  const q = rotX(x, y - (hipY + shY) * 0.5, z, tilt);
  const torso = ellipsoid(q.x, q.y, q.z, 0.16, 0.15, rz);
  // Legs: hip/shoulder to foot. Front feet lift (raise) and reach.
  const rl = clamp(k.s0, 0, 1);
  const rr = clamp(k.s1, 0, 1);
  const fl = clamp(k.s2, 0, 1);
  const fr = clamp(k.s3, 0, 1);
  const fold = crouch * 0.18;
  const fly = 0.04 + rl * 0.22 + fl * 0.14;
  const fry = 0.04 + rr * 0.22 + fr * 0.14;
  const legs = min(
    capsule(x, y, z, -0.09, shY - 0.05, zs, -0.09, fly, zs + sw + fl * 0.3 + rl * 0.08, 0.045),
    capsule(x, y, z, 0.09, shY - 0.05, zs, 0.09, fry, zs - sw + fr * 0.3 + rr * 0.08, 0.045),
    capsule(x, y, z, -0.09, hipY - 0.05, zh, -0.09, 0.04, zh - sw + fold, 0.05),
    capsule(x, y, z, 0.09, hipY - 0.05, zh, 0.09, 0.04, zh + sw + fold, 0.05)
  );
  // Neck and head: the head drops towards the ground with s5.
  const hd = clamp(k.s5, -0.5, 1);
  const hy = mix(shY + 0.26, 0.14, max(hd, 0)) - min(hd, 0) * 0.1;
  const hz = zs + 0.14 + max(hd, 0) * 0.16;
  const neck = capsule(x, y, z, 0, shY + 0.04, zs - 0.02, 0, hy, hz, 0.06);
  const head = ellipsoid(x, y - hy, z - hz, 0.085, 0.08, 0.1);
  const snout = capsule(x, y, z, 0, hy - 0.03, hz + 0.05, 0, hy - 0.035, hz + 0.15, 0.04);
  const eh = 0.03 + clamp(k.c, 0, 1) * 0.1;
  const ears = min(
    ellipsoid(x + 0.055, y - hy - 0.06 - eh * 0.5, z - hz + 0.02, 0.025, eh, 0.018),
    ellipsoid(x - 0.055, y - hy - 0.06 - eh * 0.5, z - hz + 0.02, 0.025, eh, 0.018)
  );
  // Tail: hangs, or rises and swings with s7.
  const sp = clamp(k.s7, -1, 1);
  const tl = 0.14 + clamp(k.b, 0, 1) * 0.2;
  const up = -0.7 + abs(sp) * 1.6;
  const tail = capsule(x, y, z, 0, hipY + 0.06, zh - rz * 0.25, sp * 0.12, hipY + 0.06 + sin(up) * tl, zh - rz * 0.25 - cos(up) * tl, 0.028);
  const body = smoothUnion(smoothUnion(torso, legs, 0.05), smoothUnion(neck, head, 0.04), 0.05);
  return min(smoothUnion(body, snout, 0.03), ears, smoothUnion(tail, torso, 0.03));
}

export function color(x, y, z, k) {
  const coat = hsv(k.d, 0.25 + 0.35 * hash(k.seed, 3), 0.2 + 0.7 * clamp(k.e, 0, 1));
  const light = mix(coat, rgb(240, 232, 220), 0.55);
  const leg = 0.34 + clamp(k.a, 0, 1) * 0.26;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.15;
  const zs = rz * 0.68;
  const hd = clamp(k.s5, -0.5, 1);
  const shY = leg + 0.1 - clamp(k.s6, 0, 1) * (leg - 0.2) * 0.6 - clamp(k.s4, 0, 0.6) * 0.3;
  const hy = mix(shY + 0.26, 0.14, max(hd, 0)) - min(hd, 0) * 0.1;
  const hz = zs + 0.14 + max(hd, 0) * 0.16;
  // Nose and eyes.
  if (z > hz + 0.16) {
    return rgb(30, 24, 22);
  }
  if (abs(abs(x) - 0.045) < 0.016 && abs(y - hy - 0.02) < 0.016 && z > hz + 0.05) {
    return rgb(20, 16, 14);
  }
  // Lighter belly and socks, and a patch or two.
  if (y < 0.12 || (y < leg + 0.02 && abs(x) < 0.07 && abs(z) < rz * 0.5)) {
    return mix(coat, light, step(0.5, hash(k.seed, 5)));
  }
  if (noise3(x * 6 + k.seed, y * 6, z * 6) > 0.72 && hash(k.seed, 7) > 0.5) {
    return light;
  }
  return coat;
}
