// A shy deer of the autumn woods: long thin legs, a slim body, a tall neck,
// big sideways ears, a short white flag of a tail; some carry small antlers.
// Faces +z, about 1.25 m to the top of the head (antlers to ~1.5 m).
// Look: a = legs, b = body length, c = ears, d = coat hue, e = coat shade.
// Roles: s0/s1 = left/right foreleg raised (stamping), s2/s3 = foreleg reaching,
// s4 = lean (shoulders down), s5 = head down (grazing; negative = head up, alert),
// s6 = crouch (lying in the bracken), s7 = spread (white tail flagged up / flick).
export const meta = {
  name: "deer",
  bounds: [0.3, 1.6, 0.85],
  tags: ["body", "body", "animal"],
  props: { mass: 90, alive: 1, burns: 0.2 },
  sound: { hard: 0.3, dry: 0.5, ring: 0.05 },
  body: {
    from: "quadruped",
    height: 1.25, eye: 1.18, radius: 0.3, reach: 0.8,
    grip: [0, 1.12, 0.56],
    roles: ["raise", "reach", "lean", "head", "crouch", "spread"],
    gait: "quad", flies: false, arms: false,
    look: { legs: [0.8, 1.0], length: [0.4, 0.6], ears: [0.6, 0.9], hue: [0.06, 0.09], shade: [0.35, 0.55] }
  }
};

export function sdf(x, y, z, k) {
  const leg = 0.48 + clamp(k.a, 0, 1) * 0.14;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.16;
  const crouch = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, -0.3, 0.6);
  const sw = sin(k.f) * 0.14 * (1 - crouch);
  const hipY = leg + 0.12 - crouch * (leg - 0.1);
  const shY = leg + 0.14 - crouch * (leg - 0.18) * 0.7 - lean * 0.3;
  const zs = rz * 0.65;
  const zh = -rz * 0.65;
  // Slim torso tilted from hips to shoulders.
  const tilt = atan2(hipY - shY, zs - zh);
  const q = rotX(x, y - (hipY + shY) * 0.5, z, tilt);
  const torso = ellipsoid(q.x, q.y, q.z, 0.135, 0.15, rz);
  // Forelegs: thin tapering cones.
  const rl = clamp(k.s0, 0, 1);
  const rr = clamp(k.s1, 0, 1);
  const fl = clamp(k.s2, 0, 1);
  const fr = clamp(k.s3, 0, 1);
  const fly = 0.03 + rl * 0.3 + fl * 0.15;
  const fry = 0.03 + rr * 0.3 + fr * 0.15;
  const front = min(
    cone(x, y, z, -0.075, shY - 0.06, zs, -0.075, fly, zs + sw + fl * 0.3 + rl * 0.08, 0.05, 0.02),
    cone(x, y, z, 0.075, shY - 0.06, zs, 0.075, fry, zs - sw + fr * 0.3 + rr * 0.08, 0.05, 0.02)
  );
  // Hind legs: thigh to hock, then a thin shank to the hoof.
  const fold = crouch * 0.2;
  const flz = zh - sw + fold;
  const frz = zh + sw + fold;
  const hockY = 0.06 + (hipY - 0.04) * 0.42;
  const hind = min(
    cone(x, y, z, -0.075, hipY - 0.04, zh, -0.075, hockY, zh - 0.1 + (flz - zh) * 0.5, 0.065, 0.025),
    capsule(x, y, z, -0.075, hockY, zh - 0.1 + (flz - zh) * 0.5, -0.075, 0.03, flz, 0.021),
    cone(x, y, z, 0.075, hipY - 0.04, zh, 0.075, hockY, zh - 0.1 + (frz - zh) * 0.5, 0.065, 0.025),
    capsule(x, y, z, 0.075, hockY, zh - 0.1 + (frz - zh) * 0.5, 0.075, 0.03, frz, 0.021)
  );
  // Tall neck and a narrow head; the head drops to graze.
  const hd = clamp(k.s5, -0.5, 1);
  const hdn = max(hd, 0);
  const hy = mix(shY + 0.42, 0.16, hdn) - min(hd, 0) * 0.1;
  const hz = zs + 0.16 + hdn * 0.22;
  const neck = cone(x, y, z, 0, shY + 0.02, zs - 0.02, 0, hy - 0.02, hz - 0.03, 0.08, 0.045);
  const head = ellipsoid(x, y - hy, z - hz, 0.062, 0.065, 0.09);
  const snout = cone(x, y, z, 0, hy - 0.01, hz + 0.04, 0, hy - 0.04 - hdn * 0.1, hz + 0.17 - hdn * 0.05, 0.048, 0.024);
  // Big ears held out to the sides.
  const eh = 0.07 + clamp(k.c, 0, 1) * 0.06;
  const ears = min(
    triangle(x, y, z, 0.035, hy + 0.06, hz - 0.02, 0.04, hy + 0.02, hz - 0.06, 0.05 + eh, hy + 0.05 + eh * 0.5, hz - 0.07, 0.012),
    triangle(x, y, z, -0.035, hy + 0.06, hz - 0.02, -0.04, hy + 0.02, hz - 0.06, -0.05 - eh, hy + 0.05 + eh * 0.5, hz - 0.07, 0.012)
  );
  // Short tail: hangs, flags up when alarmed.
  const sp = clamp(k.s7, -1, 1);
  const up = -1.2 + abs(sp) * 2.2;
  const tz = -rz * 0.92;
  const tail = capsule(x, y, z, 0, hipY + 0.04, tz, sp * 0.05, hipY + 0.04 + sin(up) * 0.12, tz - cos(up) * 0.12, 0.03);
  // Antlers on some.
  const stag = step(0.5, hash(k.seed, 11));
  const ant = min(
    cone(x, y, z, 0.035, hy + 0.06, hz - 0.02, 0.12, hy + 0.28, hz + 0.02, 0.018, 0.008),
    cone(x, y, z, 0.08, hy + 0.17, hz, 0.07, hy + 0.25, hz + 0.1, 0.012, 0.006),
    cone(x, y, z, -0.035, hy + 0.06, hz - 0.02, -0.12, hy + 0.28, hz + 0.02, 0.018, 0.008),
    cone(x, y, z, -0.08, hy + 0.17, hz, -0.07, hy + 0.25, hz + 0.1, 0.012, 0.006)
  );
  const antlers = stag > 0.5 ? ant : 10;
  const body = smoothUnion(smoothUnion(torso, min(front, hind), 0.05), smoothUnion(neck, head, 0.04), 0.05);
  return min(smoothUnion(body, snout, 0.03), smoothUnion(ears, head, 0.01), smoothUnion(tail, torso, 0.03), antlers);
}

export function parts(x, y, z, k) {
  const leg = 0.48 + clamp(k.a, 0, 1) * 0.14;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.16;
  const crouch = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, -0.3, 0.6);
  const hipY = leg + 0.12 - crouch * (leg - 0.1);
  const shY = leg + 0.14 - crouch * (leg - 0.18) * 0.7 - lean * 0.3;
  const zs = rz * 0.65;
  const hd = clamp(k.s5, -0.5, 1);
  const hdn = max(hd, 0);
  const hy = mix(shY + 0.42, 0.16, hdn) - min(hd, 0) * 0.1;
  const hz = zs + 0.16 + hdn * 0.22;
  if (x < 1.5) {
    return vec3(0, hy - 0.05 - hdn * 0.08, hz + 0.15 - hdn * 0.04);
  }
  if (x < 2.5) {
    return vec3(0, hy, hz);
  }
  return vec3(0, (hipY + shY) * 0.5 + 0.1, 0);
}

export function color(x, y, z, k) {
  const lit = clamp(k.e, 0, 1);
  const grey = clamp((k.d - 1) * 5, 0, 1);
  const sat = (0.5 + 0.2 * hash(k.seed, 3)) * (1 - grey) * clamp(lit * 8, 0, 1) * (1 - clamp((lit - 0.85) * 6.6, 0, 1));
  const coat = hsv(k.d, sat, 0.12 + 0.8 * lit);
  const cream = mix(coat, rgb(245, 238, 225), 0.75);
  const dark = mix(coat, rgb(40, 30, 25), 0.65);
  const leg = 0.48 + clamp(k.a, 0, 1) * 0.14;
  const rz = 0.3 + clamp(k.b, 0, 1) * 0.16;
  const crouch = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, -0.3, 0.6);
  const hipY = leg + 0.12 - crouch * (leg - 0.1);
  const shY = leg + 0.14 - crouch * (leg - 0.18) * 0.7 - lean * 0.3;
  const zs = rz * 0.65;
  const hd = clamp(k.s5, -0.5, 1);
  const hdn = max(hd, 0);
  const hy = mix(shY + 0.42, 0.16, hdn) - min(hd, 0) * 0.1;
  const hz = zs + 0.16 + hdn * 0.22;
  const stag = step(0.5, hash(k.seed, 11));
  // Black nose and eyes.
  if (z > hz + 0.15 - hdn * 0.05 && y < hy - 0.01) {
    return rgb(28, 22, 20);
  }
  if (abs(abs(x) - 0.05) < 0.015 && abs(y - hy - 0.015) < 0.015 && z > hz + 0.02 && z < hz + 0.07) {
    return rgb(18, 14, 12);
  }
  // Antlers.
  if (stag > 0.5 && (y > hy + 0.13 || (y > hy + 0.065 && abs(x) < 0.05))) {
    return rgb(205, 190, 160);
  }
  // Dark hooves.
  if (y < 0.05) {
    return rgb(45, 35, 30);
  }
  // White rump and tail.
  if (z < -rz * 0.85 && y > hipY - 0.15) {
    return cream;
  }
  // Pale belly and chin.
  if (y < (hipY + shY) * 0.5 - 0.08 && y > leg - 0.1 && abs(z) < rz) {
    return cream;
  }
  if (z > hz + 0.06 && y < hy - 0.035) {
    return cream;
  }
  // Inside of the ears darker at the rim.
  if (abs(x) > 0.1 && y > hy + 0.02) {
    return dark;
  }
  // A few still carry fawn spots on the back.
  if (hash(k.seed, 13) > 0.8 && y > shY - 0.03 && y < hipY + 0.2 && abs(z) < rz * 0.8 && noise3(x * 22 + k.seed, y * 22, z * 22) > 0.7) {
    return cream;
  }
  // Lower legs a shade darker.
  if (y < leg * 0.5) {
    return mix(coat, dark, 0.4);
  }
  return coat;
}
