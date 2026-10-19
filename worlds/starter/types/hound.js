// A dog standing along +z: a deep chest and a tucked waist on straight legs,
// a head with a long muzzle and a dark nose, ears hanging soft or standing
// up, a feathered tail; golden, brown, black, grey or white, lighter on the
// chest. Look: a = leg length, b = body length, c = ears (floppy … upright),
// d = coat hue (1 … 1.2 fades to grey), e = coat shade. f = walk phase.
// Roles: s0/s1 = left/right front paw raised, s2/s3 = left/right front leg
// reaching, s4 = lean (a play bow), s5 = head down, s6 = crouch (sit …
// lie), s7 = spread (tail up and wagging).
export const meta = { name: "hound", bounds: [0.34, 1.12, 0.98], tags: ["body"],
  props: { mass: 25, alive: 1, burns: 0.2 },
  body: { height: 1.0, eye: 0.84, radius: 0.3, reach: 0.9, grip: [0, 0.6, 0.72], seat: [0, 0.72, -0.02],
          roles: ["raise", "reach", "lean", "head", "crouch", "spread"], gait: "quad", arms: false,
          look: { legs: [0, 1], length: [0, 1], ears: [0, 1], hue: [0, 1.2], shade: [0, 1] } } };

export function sdf(x, y, z, k) {
  const leg = 0.32 + clamp(k.a, 0, 1) * 0.24;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.14;
  const crouch = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, 0, 0.6);
  const sw = sin(k.f) * 0.11 * (1 - crouch);
  const hipY = leg + 0.1 - crouch * (leg - 0.08);
  const shY = leg + 0.14 - crouch * (leg - 0.22) * 0.6 - lean * 0.3;
  const zs = rz * 0.62;
  const zh = -rz * 0.66;
  const tilt = atan2(hipY - shY, zs - zh);
  const q = rotX(x, y - (hipY + shY) * 0.5, z, tilt);
  // A deep chest forward, a tucked waist behind.
  const chest = ellipsoid(q.x, q.y + 0.02, q.z - rz * 0.35, 0.15, 0.18, rz * 0.6);
  const rear = ellipsoid(q.x, q.y + 0.03, q.z + rz * 0.4, 0.13, 0.13, rz * 0.55);
  const torso = smoothUnion(chest, rear, 0.08);
  const rl = clamp(k.s0, 0, 1);
  const rr = clamp(k.s1, 0, 1);
  const fl = clamp(k.s2, 0, 1);
  const fr = clamp(k.s3, 0, 1);
  const fold = crouch * 0.18;
  const fly = 0.04 + rl * 0.22 + fl * 0.14;
  const fry = 0.04 + rr * 0.22 + fr * 0.14;
  const legs = min(
    min(capsule(x, y, z, -0.085, shY - 0.08, zs, -0.085, fly, zs + sw + fl * 0.3 + rl * 0.08, 0.042),
        capsule(x, y, z, 0.085, shY - 0.08, zs, 0.085, fry, zs - sw + fr * 0.3 + rr * 0.08, 0.042)),
    min(capsule(x, y, z, -0.085, hipY - 0.05, zh, -0.085, 0.04, zh - sw + fold, 0.05),
        capsule(x, y, z, 0.085, hipY - 0.05, zh, 0.085, 0.04, zh + sw + fold, 0.05)));
  const hd = clamp(k.s5, -0.5, 1);
  const hy = mix(shY + 0.24, 0.16, max(hd, 0)) - min(hd, 0) * 0.1;
  const hz = zs + 0.16 + max(hd, 0) * 0.16;
  const neck = capsule(x, y, z, 0, shY + 0.02, zs - 0.04, 0, hy - 0.02, hz - 0.02, 0.075);
  const skull = ellipsoid(x, y - hy, z - hz, 0.09, 0.085, 0.1);
  const muzzle = capsule(x, y, z, 0, hy - 0.035, hz + 0.05, 0, hy - 0.045, hz + 0.18, 0.045);
  const head = smoothUnion(skull, muzzle, 0.04);
  // Ears: hanging soft beside the head, or (c high) standing up.
  const up = clamp(k.c, 0, 1);
  const ax = abs(x);
  const hang = ellipsoid(ax - 0.09, y - hy + 0.05, z - hz + 0.02, 0.025, 0.08, 0.05);
  const stand = triangle(ax, y, z, 0.03, hy + 0.06, hz - 0.03, 0.085, hy + 0.04, hz - 0.03, 0.065, hy + 0.15, hz - 0.04, 0.015);
  const ears = up > 0.5 ? stand : hang;
  // The tail, feathered, held out and up, wagging with s7.
  const sp = clamp(k.s7, -1, 1);
  const tl = 0.24 + clamp(k.b, 0, 1) * 0.1;
  const rise = -0.5 + abs(sp) * 1.3;
  const tail = capsule(x, y, z, 0, hipY + 0.08, zh - rz * 0.3, sp * 0.14, hipY + 0.08 + sin(rise) * tl, zh - rz * 0.3 - cos(rise) * tl, 0.035);
  const body = smoothUnion(smoothUnion(torso, legs, 0.05), smoothUnion(neck, head, 0.05), 0.06);
  return min(min(body, ears), smoothUnion(tail, torso, 0.03));
}

export function color(x, y, z, k) {
  const lit = clamp(k.e, 0, 1);
  const grey = clamp((k.d - 1) * 5, 0, 1);
  const sat = (0.3 + 0.35 * hash(k.seed, 3)) * (1 - grey) * (1 - clamp((lit - 0.85) * 6.6, 0, 1)) * clamp(lit * 8, 0, 1);
  // Each dog its own coat: golden, brown, black, grey or cream.
  const kind = floor(hash(k.seed, 11) * 5.0);
  let coat = hsv(k.d, sat, 0.08 + 0.87 * lit);
  if (kind == 0) coat = rgb(206, 156, 80);
  if (kind == 1) coat = rgb(30, 28, 28);
  if (kind == 2) coat = rgb(120, 118, 116);
  if (kind == 3) coat = rgb(226, 214, 186);
  const light = mix(coat, rgb(240, 230, 210), 0.5);
  const leg = 0.32 + clamp(k.a, 0, 1) * 0.24;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.14;
  const zs = rz * 0.62;
  const hd = clamp(k.s5, -0.5, 1);
  const shY = leg + 0.14 - clamp(k.s6, 0, 1) * (leg - 0.22) * 0.6 - clamp(k.s4, 0, 0.6) * 0.3;
  const hy = mix(shY + 0.24, 0.16, max(hd, 0)) - min(hd, 0) * 0.1;
  const hz = zs + 0.16 + max(hd, 0) * 0.16;
  if (z > hz + 0.2) return rgb(26, 22, 20);
  if (abs(abs(x) - 0.045) < 0.016 && abs(y - hy - 0.02) < 0.016 && z > hz + 0.06) return rgb(30, 18, 10);
  // A lighter chest and muzzle, and darker ears on some.
  if ((z > zs - 0.05 && y < shY + 0.05 && y > leg * 0.6 && abs(x) < 0.09) || (z > hz + 0.08 && y < hy - 0.03)) return light;
  if (abs(x) > 0.08 && y > hy - 0.13 && y < hy + 0.16 && abs(z - hz) < 0.08 && hash(k.seed, 6) > 0.5) return coat * 0.6;
  return coat * (0.92 + 0.08 * noise3(x * 12, y * 30, z * 12));
}
