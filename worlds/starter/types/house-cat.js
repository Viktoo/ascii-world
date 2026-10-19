// A house cat lying along +z: a plump, soft body on short legs, a round head
// with a small muzzle, tall pointed ears and big eyes, a long tail that
// rises in a curve behind; tabby-striped, ginger, grey, black or white.
// Look: a = leg length, b = body length, c = ears, d = coat hue (1 … 1.2
// fades to grey), e = coat shade (black … white). f = walk phase.
// Roles: s0/s1 = left/right front paw raised, s2/s3 = left/right front leg
// reaching, s4 = lean (a stretch, front down), s5 = head down, s6 = crouch
// (sit … curled up), s7 = spread (tail up and swishing).
export const meta = { name: "house cat", bounds: [0.32, 0.8, 0.85], tags: ["body"],
  props: { mass: 4.5, alive: 1, burns: 0.2 },
  body: { height: 0.72, eye: 0.6, radius: 0.22, reach: 0.5, grip: [0, 0.36, 0.42], seat: [0, 0.46, -0.02],
          roles: ["raise", "reach", "lean", "head", "crouch", "spread"], gait: "quad", arms: false,
          look: { legs: [0, 1], length: [0, 1], ears: [0, 1], hue: [0, 1.2], shade: [0, 1] } } };

export function sdf(x, y, z, k) {
  const leg = 0.2 + clamp(k.a, 0, 1) * 0.12;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.1;
  const crouch = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, 0, 0.6);
  const sw = sin(k.f) * 0.08 * (1 - crouch);
  const hipY = leg + 0.1 - crouch * (leg - 0.06);
  const shY = leg + 0.1 - crouch * (leg - 0.14) * 0.6 - lean * 0.22;
  const zs = rz * 0.62;
  const zh = -rz * 0.62;
  const tilt = atan2(hipY - shY, zs - zh);
  const q = rotX(x, y - (hipY + shY) * 0.5, z, tilt);
  const torso = smoothUnion(ellipsoid(q.x, q.y, q.z, 0.12, 0.12, rz), ellipsoid(q.x, q.y + 0.01, q.z + rz * 0.35, 0.135, 0.125, rz * 0.55), 0.06);
  const rl = clamp(k.s0, 0, 1);
  const rr = clamp(k.s1, 0, 1);
  const fl = clamp(k.s2, 0, 1);
  const fr = clamp(k.s3, 0, 1);
  const fold = crouch * 0.14;
  const fly = 0.03 + rl * 0.16 + fl * 0.1;
  const fry = 0.03 + rr * 0.16 + fr * 0.1;
  const legs = min(
    min(capsule(x, y, z, -0.065, shY - 0.04, zs, -0.065, fly, zs + sw + fl * 0.22 + rl * 0.06, 0.032),
        capsule(x, y, z, 0.065, shY - 0.04, zs, 0.065, fry, zs - sw + fr * 0.22 + rr * 0.06, 0.032)),
    min(capsule(x, y, z, -0.07, hipY - 0.03, zh, -0.07, 0.03, zh - sw + fold, 0.04),
        capsule(x, y, z, 0.07, hipY - 0.03, zh, 0.07, 0.03, zh + sw + fold, 0.04)));
  // A round head close on the shoulders, dropping with s5.
  const hd = clamp(k.s5, -0.5, 1);
  const hy = mix(shY + 0.17, 0.1, max(hd, 0)) - min(hd, 0) * 0.08;
  const hz = zs + 0.1 + max(hd, 0) * 0.12;
  const neck = capsule(x, y, z, 0, shY + 0.02, zs - 0.02, 0, hy - 0.02, hz - 0.02, 0.06);
  const head = smoothUnion(ellipsoid(x, y - hy, z - hz, 0.095, 0.085, 0.085), ellipsoid(x, y - hy + 0.025, z - hz - 0.07, 0.045, 0.035, 0.035), 0.03);
  const eh = 0.045 + clamp(k.c, 0, 1) * 0.035;
  const ax = abs(x);
  const ears = triangle(ax, y, z, 0.035, hy + 0.06, hz - 0.01, 0.085, hy + 0.05, hz - 0.015, 0.068, hy + 0.06 + eh, hz - 0.02, 0.012);
  // The tail: out from the rump and up in a curve, swishing with s7.
  const sp = clamp(k.s7, -1, 1);
  const tx = sp * 0.1 + sin(k.f * 0.5) * 0.03;
  const lift = 0.15 + abs(sp) * 0.2 - crouch * 0.12;
  const t1 = capsule(x, y, z, 0, hipY + 0.04, zh - rz * 0.3, tx * 0.5, hipY + 0.04 + lift * 0.5, zh - rz * 0.3 - 0.16, 0.026);
  const t2 = capsule(x, y, z, tx * 0.5, hipY + 0.04 + lift * 0.5, zh - rz * 0.3 - 0.16, tx, hipY + 0.06 + lift * 1.4, zh - rz * 0.3 - 0.2, 0.022);
  const body = smoothUnion(smoothUnion(torso, legs, 0.04), smoothUnion(neck, head, 0.04), 0.05);
  return min(min(body, ears), smoothUnion(min(t1, t2), torso, 0.03));
}

export function color(x, y, z, k) {
  const lit = clamp(k.e, 0, 1);
  const grey = clamp((k.d - 1) * 5, 0, 1);
  const sat = (0.35 + 0.35 * hash(k.seed, 3)) * (1 - grey) * (1 - clamp((lit - 0.85) * 6.6, 0, 1)) * clamp(lit * 8, 0, 1);
  // Each cat its own coat: ginger, grey, black, white or brown tabby.
  const kind = floor(hash(k.seed, 11) * 5.0);
  let coat = hsv(k.d, sat, 0.08 + 0.87 * lit);
  if (kind == 0) coat = hsv(0.07, 0.72, 0.78);
  if (kind == 1) coat = rgb(128, 130, 136);
  if (kind == 2) coat = rgb(28, 26, 28);
  if (kind == 3) coat = rgb(240, 236, 228);
  const light = mix(coat, rgb(244, 238, 228), 0.6);
  const leg = 0.2 + clamp(k.a, 0, 1) * 0.12;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.1;
  const zs = rz * 0.62;
  const hd = clamp(k.s5, -0.5, 1);
  const shY = leg + 0.1 - clamp(k.s6, 0, 1) * (leg - 0.14) * 0.6 - clamp(k.s4, 0, 0.6) * 0.22;
  const hy = mix(shY + 0.17, 0.1, max(hd, 0)) - min(hd, 0) * 0.08;
  const hz = zs + 0.1 + max(hd, 0) * 0.12;
  // Nose, eyes (green or amber), inner ears.
  if (z > hz + 0.1 && abs(y - hy + 0.015) < 0.014 && abs(x) < 0.014) return rgb(200, 120, 120);
  if (abs(abs(x) - 0.04) < 0.017 && abs(y - hy - 0.01) < 0.016 && z > hz + 0.055) {
    if (abs(abs(x) - 0.04) < 0.005) return rgb(10, 10, 10);
    return hash(k.seed, 9) > 0.5 ? rgb(130, 170, 60) : rgb(214, 160, 40);
  }
  if (y > hy + 0.065 && abs(abs(x) - 0.06) < 0.012 && z > hz - 0.012) return rgb(220, 160, 150);
  // White chest and socks on some; tabby stripes on others.
  const bib = hash(k.seed, 5) > 0.5;
  if (bib && (y < 0.06 || (z > zs && y < shY + 0.02 && abs(x) < 0.07))) return light;
  if (hash(k.seed, 7) > 0.45) {
    const band = sin(z * 38.0 + noise3(x * 8, y * 8, z * 8) * 3.0 + y * 6.0);
    if (band > 0.45 && y > 0.1) return coat * 0.55;
  }
  return coat * (0.92 + 0.08 * noise3(x * 30, y * 30, z * 30));
}
