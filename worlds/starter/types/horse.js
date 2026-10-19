// A strong, gentle horse, standing on y = 0 and facing +z. About 1.6 m at
// the withers; the ear tips reach about 2 m.
// Look: a = legs, b = body length, c = ears, d = coat hue, e = coat shade.
// f = walk phase (a diagonal trot).
// Roles:
//   s0 / s1 = left / right front hoof raised (pawing)
//   s2 / s3 = left / right foreleg reaching forward
//   s4 = lean (forehand down; negative lifts it)
//   s5 = head (down to graze; negative throws it up)
//   s6 = crouch (lie down)
//   s7 = spread (tail lifted and swished)
export const meta = {
  name: "horse",
  bounds: [0.5, 2.4, 1.65],
  tags: ["body", "body"],
  props: { mass: 500, alive: 1, burns: 0.2 },
  sound: { hard: 0.5, dry: 0.5, ring: 0.1 },
  body: {
    from: "quadruped",
    height: 1.9, eye: 1.7, radius: 0.6, reach: 1.2,
    grip: [0, 1.45, 1.4],
    seat: [0, 1.45, 0.1],
    roles: ["raise", "reach", "lean", "head", "crouch", "spread"],
    gait: "quad", flies: false, arms: false,
    look: { legs: [0.85, 1.0], length: [0.6, 0.9], ears: [0.5, 0.7], hue: [0.04, 0.09], shade: [0.15, 0.75] }
  }
};

export function sdf(x, y, z, k) {
  const L = 0.75 + (clamp(k.a, 0.5, 1.2) - 0.85) * 0.6;
  const rz = 0.62 + clamp(k.b, 0, 1) * 0.22;
  const c = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, -0.3, 0.6);
  const hipY = L + 0.32 - c * (L + 0.32 - 0.4);
  const shY = hipY + 0.06 - c * 0.04 - lean * 0.35;
  const zs = rz * 0.7;
  const zh = -rz * 0.7;

  // Barrel, tilted from hips to shoulders, and the rounded rump.
  const tilt = atan2(hipY - shY, zs - zh);
  const q = rotX(x, y - (hipY + shY) * 0.5, z, tilt);
  const torso = ellipsoid(q.x, q.y, q.z, 0.3, 0.32, rz);
  const rump = ellipsoid(x, y - hipY - 0.04, z - zh + 0.05, 0.31, 0.33, 0.38);

  // Legs: a diagonal trot. Hind legs bend back at the hock.
  const swA = sin(k.f) * 0.22 * (1 - c);
  const liftA = max(0, cos(k.f)) * 0.1 * (1 - c);
  const liftB = max(0, -cos(k.f)) * 0.1 * (1 - c);
  const rl = clamp(k.s0, 0, 1);
  const rr = clamp(k.s1, 0, 1);
  const fl = clamp(k.s2, 0, 1);
  const fr = clamp(k.s3, 0, 1);
  const flY = 0.06 + liftA + rl * 0.4 + fl * 0.15;
  const flZ = zs + swA + fl * 0.45 + rl * 0.15 + c * 0.25;
  const frY = 0.06 + liftB + rr * 0.4 + fr * 0.15;
  const frZ = zs - swA + fr * 0.45 + rr * 0.15 + c * 0.25;
  const hlY = 0.06 + liftB;
  const hlZ = zh - swA + c * 0.2;
  const hrY = 0.06 + liftA;
  const hrZ = zh + swA + c * 0.2;
  const hkLY = hlY + (hipY - 0.1 - hlY) * 0.38;
  const hkLZ = (zh + hlZ) * 0.5 - 0.14 * (1 - c * 0.6);
  const hkRY = hrY + (hipY - 0.1 - hrY) * 0.38;
  const hkRZ = (zh + hrZ) * 0.5 - 0.14 * (1 - c * 0.6);
  const legs = min(
    cone(x, y, z, -0.16, shY - 0.15, zs, -0.16, flY, flZ, 0.1, 0.05),
    cone(x, y, z, 0.16, shY - 0.15, zs, 0.16, frY, frZ, 0.1, 0.05),
    cone(x, y, z, -0.16, hipY - 0.1, zh, -0.15, hkLY, hkLZ, 0.12, 0.065),
    cone(x, y, z, -0.15, hkLY, hkLZ, -0.15, hlY, hlZ, 0.065, 0.05),
    cone(x, y, z, 0.16, hipY - 0.1, zh, 0.15, hkRY, hkRZ, 0.12, 0.065),
    cone(x, y, z, 0.15, hkRY, hkRZ, 0.15, hrY, hrZ, 0.065, 0.05)
  );

  // Neck and long head; the head drops to graze or is thrown up.
  const hd = clamp(k.s5, -1, 1);
  const dn = max(hd, 0);
  const upv = max(-hd, 0);
  const pollY = mix(shY + 0.62, 0.62, dn) + upv * 0.12;
  const pollZ = zs + 0.42 + dn * 0.35 - upv * 0.08;
  const hang = 0.85 + dn * 0.6 - upv * 0.5;
  const mY = pollY - sin(hang) * 0.55;
  const mZ = pollZ + cos(hang) * 0.55;
  const neck = cone(x, y, z, 0, shY + 0.02, zs - 0.08, 0, pollY - 0.04, pollZ - 0.04, 0.22, 0.12);
  const headc = cone(x, y, z, 0, pollY, pollZ, 0, mY, mZ, 0.11, 0.075);
  const cheek = ellipsoid(x, y - pollY + 0.08, z - pollZ - 0.04, 0.09, 0.12, 0.13);
  const mane = capsule(x, y, z, 0, shY + 0.26, zs - 0.12, 0, pollY + 0.07, pollZ - 0.06, 0.045);
  const eh = 0.1 + clamp(k.c, 0, 1) * 0.08;
  const ears = min(
    cone(x, y, z, -0.06, pollY + 0.05, pollZ, -0.075, pollY + 0.05 + eh, pollZ - 0.03, 0.035, 0.01),
    cone(x, y, z, 0.06, pollY + 0.05, pollZ, 0.075, pollY + 0.05 + eh, pollZ - 0.03, 0.035, 0.01)
  );

  // Tail: hangs long; lifts and swishes with spread.
  const sp = clamp(k.s7, -1, 1);
  const tY = max(hipY - 0.6 + abs(sp) * 0.7, 0.12);
  const tZ = zh - 0.5 - abs(sp) * 0.3;
  const tX = sp * 0.2 + sin(k.f) * 0.04;
  const tail = cone(x, y, z, 0, hipY + 0.2, zh - 0.36, tX, tY, tZ, 0.05, 0.08);

  const body = smoothUnion(smoothUnion(torso, rump, 0.12), legs, 0.06);
  const head = smoothUnion(neck, smoothUnion(headc, cheek, 0.05), 0.06);
  const all = smoothUnion(body, head, 0.08);
  return min(smoothUnion(all, mane, 0.03), ears, smoothUnion(tail, rump, 0.04));
}

export function color(x, y, z, k) {
  const L = 0.75 + (clamp(k.a, 0.5, 1.2) - 0.85) * 0.6;
  const rz = 0.62 + clamp(k.b, 0, 1) * 0.22;
  const c = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, -0.3, 0.6);
  const hipY = L + 0.32 - c * (L + 0.32 - 0.4);
  const shY = hipY + 0.06 - c * 0.04 - lean * 0.35;
  const zs = rz * 0.7;
  const zh = -rz * 0.7;
  const hd = clamp(k.s5, -1, 1);
  const dn = max(hd, 0);
  const upv = max(-hd, 0);
  const pollY = mix(shY + 0.62, 0.62, dn) + upv * 0.12;
  const pollZ = zs + 0.42 + dn * 0.35 - upv * 0.08;
  const hang = 0.85 + dn * 0.6 - upv * 0.5;
  const mY = pollY - sin(hang) * 0.55;
  const mZ = pollZ + cos(hang) * 0.55;

  // Coat.
  const lit = clamp(k.e, 0, 1);
  const grey = clamp((k.d - 1) * 5, 0, 1);
  const sat = (0.5 + 0.2 * hash(k.seed, 3)) * (1 - grey) * (1 - clamp((lit - 0.85) * 6.6, 0, 1)) * clamp(lit * 8, 0, 1);
  const coat = hsv(k.d, sat, 0.08 + 0.8 * lit);
  // Mane and tail: black points, or flaxen on some.
  const flax = step(0.75, hash(k.seed, 11));
  const hair = mix(mix(coat, rgb(28, 22, 20), 0.75), rgb(225, 200, 150), flax);
  const white = rgb(238, 232, 222);

  // Muzzle and eyes.
  if (length3(x, y - mY, z - mZ) < 0.1) {
    return rgb(40, 32, 30);
  }
  const eY = pollY - sin(hang) * 0.12;
  const eZ = pollZ + cos(hang) * 0.12;
  if (abs(abs(x) - 0.1) < 0.03 && length2(y - eY, z - eZ) < 0.035) {
    return rgb(18, 14, 12);
  }
  // Blaze down the face on some.
  const px = y - pollY;
  const pz = z - pollZ;
  const t = -px * sin(hang) + pz * cos(hang);
  const front = px * cos(hang) + pz * sin(hang);
  if (hash(k.seed, 5) > 0.55 && abs(x) < 0.035 && t > 0.08 && t < 0.5 && front > 0.04) {
    return white;
  }
  // Mane.
  const mane = capsule(x, y, z, 0, shY + 0.26, zs - 0.12, 0, pollY + 0.07, pollZ - 0.06, 0.045);
  if (mane < 0.02) {
    return hair;
  }
  // Tail.
  const sp = clamp(k.s7, -1, 1);
  const tY = max(hipY - 0.6 + abs(sp) * 0.7, 0.12);
  const tZ = zh - 0.5 - abs(sp) * 0.3;
  const tX = sp * 0.2 + sin(k.f) * 0.04;
  const tail = cone(x, y, z, 0, hipY + 0.2, zh - 0.36, tX, tY, tZ, 0.05, 0.08);
  if (tail < 0.02) {
    return hair;
  }
  // Hooves, socks or dark points on the lower legs.
  if (y < 0.1 && c < 0.5) {
    return rgb(50, 42, 36);
  }
  if (y < L * 0.42 && c < 0.5) {
    if (hash(k.seed, 7) > 0.6) {
      return white;
    }
    return mix(coat, hair, 0.6 * (1 - flax));
  }
  // A little dappling.
  const n = noise3(x * 5 + k.seed, y * 5, z * 5);
  return mix(coat, mix(coat, white, 0.2), step(0.7, n) * step(0.5, hash(k.seed, 9)));
}

export function parts(x, y, z, k) {
  const L = 0.75 + (clamp(k.a, 0.5, 1.2) - 0.85) * 0.6;
  const rz = 0.62 + clamp(k.b, 0, 1) * 0.22;
  const c = clamp(k.s6, 0, 1);
  const lean = clamp(k.s4, -0.3, 0.6);
  const hipY = L + 0.32 - c * (L + 0.32 - 0.4);
  const shY = hipY + 0.06 - c * 0.04 - lean * 0.35;
  const zs = rz * 0.7;
  const hd = clamp(k.s5, -1, 1);
  const dn = max(hd, 0);
  const upv = max(-hd, 0);
  const pollY = mix(shY + 0.62, 0.62, dn) + upv * 0.12;
  const pollZ = zs + 0.42 + dn * 0.35 - upv * 0.08;
  const hang = 0.85 + dn * 0.6 - upv * 0.5;
  // 0 and 1: the mouth. 2: the middle of the head.
  if (x < 1.5) {
    return vec3(0, pollY - sin(hang) * 0.5, pollZ + cos(hang) * 0.5);
  }
  if (x < 2.5) {
    return vec3(0, pollY - sin(hang) * 0.25, pollZ + cos(hang) * 0.25);
  }
  // 3: the middle of the back.
  return vec3(0, (hipY + shY) * 0.5 + 0.32, 0.05);
}
