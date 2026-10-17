// Built-in phantom body: a person, risen wrong. Written from the human figure
// (what fits a person fits it), but tall and wasted, stooped and with one arm
// always reaching (its rest stance, so what it wears follows), the head hanging
// forward and over to one side, the other arm hanging dead, a dragging left leg.
// Skin like candle wax gone grey at the edges; a skull pressed out through the
// face, deep sockets with a pinpoint of cold light far back in each (out when
// it dies, or for a flicker as it blinks), the jaw hanging too low in a long
// black maw; lank black hair hanging over the face; arms too long, bent at
// the elbow, ending in long hooked fingers; clothes gone grey and torn.
// Look: a = height (m), b = build, c = skin (wax … ash), d = shirt hue,
// e = trousers hue, f = walk phase (radians). Faces +z.
// Roles (live pose): s0/s1 = left/right arm raised sideways (0 down … 1 up),
// s2/s3 = left/right arm reaching forward (0 … 1), s4 = lean forward (radians),
// s5 = head nod, s6 = crouch (0 … 1). s10 = eyes shut (the lights go out).
// parts(i, _, _, k): its right hand (0), left hand (1), the middle of its
// head (2) and chest (3), where sdf() draws them: what it holds is in its
// hand, and what it wears follows its hanging head.
export const meta = { name: "phantom", bounds: [1.1, 2.6, 1.2], tags: ["builtin", "body"],
  props: { mass: 55, alive: 1 },
  body: { height: 2.0, eye: 1.76, radius: 0.33, reach: 2.6, grip: [0.3, 1.0, 0.3], from: "figure",
          roles: ["raise", "reach", "lean", "head", "crouch"], gait: "biped", arms: true,
          rest: { lean: 0.3, reach_r: 0.6, head: 0.25 },
          dead: { raise_l: 0.12, raise_r: 0.22, lean: -0.3, head: -0.5 },
          look: { height: [1.9, 2.2], build: [0.65, 0.85], skin: [0, 1], shirt: [0, 1], trousers: [0, 1] } } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.9, 2.2) / 1.75;
  const w = clamp(k.b, 0.65, 0.85);
  const px = x / h;
  const py = y / h;
  const pz = z / h;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  // A shamble: the right leg steps, the left drags short behind it.
  const sw = sin(k.f) * (1 - crouch);
  const lsw = sw * 0.12 - 0.05;
  const rsw = -sw * 0.26;
  const kz = 0.05 + crouch * 0.32;
  const ky = 0.48 - crouch * 0.12;
  const legs = min(
    capsule(px, py, pz, -0.1 * w, hip, 0, -0.11 * w, ky, kz + lsw * 0.5, 0.06 * w),
    capsule(px, py, pz, -0.11 * w, ky, kz + lsw * 0.5, -0.13 * w, 0.07, lsw - 0.03, 0.045 * w),
    capsule(px, py, pz, 0.1 * w, hip, 0, 0.1 * w, ky, kz + rsw * 0.5, 0.06 * w),
    capsule(px, py, pz, 0.1 * w, ky, kz + rsw * 0.5, 0.11 * w, 0.07, rsw, 0.045 * w),
    // Bare bony feet, the left turned in, scraping.
    capsule(px, py, pz, -0.13 * w, 0.04, lsw - 0.03, -0.08 * w, 0.025, lsw + 0.12, 0.035),
    capsule(px, py, pz, 0.11 * w, 0.04, rsw, 0.12 * w, 0.025, rsw + 0.14, 0.035)
  );
  // Stooped from the hip, the whole upper body tipped a little to the left.
  const q0 = rotX(px, py - hip, pz, -clamp(k.s4, -0.3, 0.6));
  const q = rotZ(q0.x, q0.y, q0.z, 0.07);
  // A chest of ribs over a waist sunk to nothing; shoulder blades jut.
  const chest = ellipsoid(q.x, q.y - 0.42, q.z, 0.17 * w, 0.17, 0.1 * w);
  const waist = roundBox(q.x, q.y - 0.16, q.z, 0.11 * w, 0.17, 0.065 * w, 0.05);
  const blades = ellipsoid(abs(q.x) - 0.08 * w, q.y - 0.46, q.z + 0.07 * w, 0.06, 0.07, 0.03);
  const torso = smoothUnion(smoothUnion(chest, waist, 0.08), blades, 0.03);
  // The left shoulder slumped lower than the right; arms too long, bent.
  const lsy = 0.5;
  const rsy = 0.55;
  const arm = 0.8;
  // The left arm hangs dead, a little forward.
  const la = clamp(k.s0, 0, 1) * 3.0;
  const ra = clamp(k.s1, 0, 1) * 3.0;
  const lf = 0.15 + clamp(k.s2, 0, 1) * 1.3;
  const rf = clamp(k.s3, 0, 1) * 1.45;
  const lhx = -0.22 * w - arm * sin(la);
  const lhy = lsy - arm * cos(la) * cos(lf);
  const lhz = arm * cos(la) * sin(lf);
  const rhx = 0.22 * w + arm * sin(ra) - 0.04;
  const rhy = rsy - arm * cos(ra) * cos(rf);
  const rhz = arm * cos(ra) * sin(rf);
  // Elbows bowed out and back, knobbly.
  const lex = (-0.2 * w + lhx) * 0.5 - 0.05;
  const ley = (lsy + lhy) * 0.5;
  const lez = lhz * 0.5 - 0.06;
  const rex = (0.2 * w + rhx) * 0.5 + 0.05;
  const rey = (rsy + rhy) * 0.5;
  const rez = rhz * 0.5 - 0.06;
  const arms = min(
    capsule(q.x, q.y, q.z, -0.2 * w, lsy, 0, lex, ley, lez, 0.04 * w),
    capsule(q.x, q.y, q.z, lex, ley, lez, lhx, lhy, lhz, 0.03 * w),
    capsule(q.x, q.y, q.z, 0.2 * w, rsy, 0, rex, rey, rez, 0.04 * w),
    capsule(q.x, q.y, q.z, rex, rey, rez, rhx, rhy, rhz, 0.03 * w),
    sphere(q.x - lex, q.y - ley, q.z - lez, 0.034 * w),
    sphere(q.x - rex, q.y - rey, q.z - rez, 0.034 * w)
  );
  // Long fingers, three to a hand, hooked at the ends: the dead hand
  // hangs them down, the reaching one curls them forward.
  const fingers = min(
    min(
      capsule(q.x, q.y, q.z, lhx, lhy, lhz, lhx - 0.03, lhy - 0.17, lhz + 0.03, 0.011),
      capsule(q.x, q.y, q.z, lhx, lhy, lhz, lhx, lhy - 0.2, lhz + 0.045, 0.011),
      capsule(q.x, q.y, q.z, lhx, lhy, lhz, lhx + 0.03, lhy - 0.17, lhz + 0.035, 0.011),
      capsule(q.x, q.y, q.z, lhx, lhy - 0.2, lhz + 0.045, lhx + 0.005, lhy - 0.25, lhz + 0.09, 0.008)
    ),
    min(
      capsule(q.x, q.y, q.z, rhx, rhy, rhz, rhx - 0.03, rhy - 0.05, rhz + 0.13, 0.011),
      capsule(q.x, q.y, q.z, rhx, rhy, rhz, rhx, rhy - 0.06, rhz + 0.15, 0.011),
      capsule(q.x, q.y, q.z, rhx, rhy, rhz, rhx + 0.03, rhy - 0.05, rhz + 0.13, 0.011),
      capsule(q.x, q.y, q.z, rhx, rhy - 0.06, rhz + 0.15, rhx, rhy - 0.11, rhz + 0.17, 0.008)
    )
  );
  // The head hangs forward on its neck, over to one side.
  const nod = clamp(k.s5, -1, 1);
  const hx = 0.05;
  const hy = 0.74 - nod * 0.03;
  const hz = 0.12 + nod * 0.04;
  const neck = capsule(q.x, q.y, q.z, 0, 0.56, 0, hx * 0.7, hy - 0.06, hz - 0.05, 0.035);
  const r = rotZ(q.x - hx, q.y - hy, q.z - hz, -0.3);
  // A long narrow skull, the bone pressed out at the cheeks and brow.
  let head = ellipsoid(r.x, r.y - 0.01, r.z, 0.085, 0.115, 0.1);
  head = smoothUnion(head, ellipsoid(abs(r.x) - 0.048, r.y + 0.022, r.z - 0.066, 0.026, 0.018, 0.022), 0.015);
  head = smoothUnion(head, capsule(r.x, r.y, r.z, -0.05, 0.035, 0.078, 0.05, 0.035, 0.078, 0.014), 0.012);
  // Hollow cheeks under the bone.
  head = smoothSubtract(head, ellipsoid(abs(r.x) - 0.062, r.y + 0.065, r.z - 0.08, 0.022, 0.03, 0.03), 0.015);
  // The jaw, hanging far too low.
  const jaw = ellipsoid(r.x, r.y + 0.15, r.z - 0.03, 0.05, 0.045, 0.065);
  head = smoothUnion(head, jaw, 0.04);
  // Deep sockets, and the long maw between cheek and jaw.
  head = smoothSubtract(head, ellipsoid(abs(r.x) - 0.034, r.y - 0.005, r.z - 0.09, 0.022, 0.019, 0.03), 0.006);
  head = smoothSubtract(head, ellipsoid(r.x, r.y + 0.105, r.z - 0.085, 0.026, 0.06, 0.04), 0.008);
  // Where the nose was, two slits.
  head = smoothSubtract(head, ellipsoid(abs(r.x) - 0.009, r.y + 0.045, r.z - 0.098, 0.006, 0.014, 0.012), 0.004);
  // A pinpoint of light far back in each socket.
  const eyes = sphere(abs(r.x) - 0.034, r.y - 0.003, r.z - 0.07, 0.006 + 0.0015 * sign(r.x));
  // Lank black hair from the crown, hanging past the jaw, parted over the
  // face; one hank across the left eye.
  const scalp = max(ellipsoid(r.x, r.y - 0.02, r.z + 0.006, 0.093, 0.122, 0.108), -(r.y - 0.04 - max(r.z - 0.02, 0) * 0.6));
  const hair = min(
    scalp,
    // Sheets of it down each side of the face and over the shoulders,
    // wrapping round, ragged at the ends.
    ellipsoid(abs(r.x) - 0.07, r.y + 0.11 + 0.025 * sin(r.z * 70 + r.x * 40), r.z + 0.01, 0.03, 0.2, 0.08),
    ellipsoid(r.x, r.y + 0.12, r.z + 0.075, 0.1, 0.23, 0.035),
    // A hank across the left eye.
    capsule(r.x, r.y, r.z, -0.04, 0.085, 0.08, -0.05, -0.2, 0.125, 0.005)
  );
  const upper = smoothUnion(torso, min(arms, fingers), 0.035);
  const body = smoothUnion(legs, upper, 0.06);
  const skull = smoothUnion(neck, smoothUnion(head, hair, 0.006), 0.03);
  return min(smoothUnion(body, skull, 0.04), eyes) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 1.9, 2.2) / 1.75;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const py = y / h;
  const q0 = rotX(x / h, py - hip, z / h, -clamp(k.s4, -0.3, 0.6));
  const q = rotZ(q0.x, q0.y, q0.z, 0.07);
  const nod = clamp(k.s5, -1, 1);
  const skin = mix(rgb(214, 212, 198), rgb(140, 142, 140), clamp(k.c, 0, 1));
  const rot = noise3(x * 6, y * 6, z * 6 + k.seed);
  const vein = noise3(x * 30, y * 30, z * 30 + k.seed);
  // Waxy, grey-green where it is rotting, with the veins showing dark.
  const flesh = mix(mix(skin, rgb(104, 118, 108), clamp(rot * 0.7 - 0.1, 0, 0.45)), rgb(70, 76, 96), 0.3 * (1 - smoothstep(0.005, 0.02, abs(vein - 0.5))));
  // The head and neck.
  if (q.y > 0.58 && py > hip) {
    const r = rotZ(q.x - 0.05, q.y - 0.74 + nod * 0.03, q.z - 0.12 - nod * 0.04, -0.3);
    // The lights: cold, small, far back; out when the eyes shut.
    const ex = abs(r.x) - 0.034;
    const ey = r.y - 0.003;
    const ez = r.z - 0.07;
    if (ex * ex + ey * ey + ez * ez < 0.0001) {
      if (k.s10 > 0.5) {
        return rgb(12, 12, 14);
      }
      return glow(rgb(190, 225, 255), 1);
    }
    // Hair: anything outside the skin of the skull, or the hanks over it.
    const inSkull = (r.x * r.x) / 0.0081 + ((r.y - 0.01) * (r.y - 0.01)) / 0.0144 + (r.z * r.z) / 0.0110;
    const hank = length2(r.x + 0.04 + (0.085 - r.y) * 0.035, r.z - 0.08 - (0.085 - r.y) * 0.157);
    if ((inSkull > 1.03 && (abs(r.x) > 0.06 || r.z < 0.0 || r.y > 0.045)) || (hank < 0.009 && r.y < 0.09 && r.y > -0.21)) {
      return mix(rgb(10, 9, 9), rgb(30, 29, 30), clamp(noise3(r.x * 300, r.y * 14, r.z * 300), 0, 1));
    }
    // The sockets: black in the depth, bruised round the rim.
    const sock = (ex * ex) / 0.00048 + ((r.y - 0.005) * (r.y - 0.005)) / 0.00036 + ((r.z - 0.09) * (r.z - 0.09)) / 0.0009;
    if (sock < 1.6) {
      return mix(rgb(6, 4, 8), rgb(70, 50, 66), clamp((sock - 0.6), 0, 1));
    }
    // The maw: black inside, a row of long yellowed teeth along its top.
    const mx = r.x;
    const my = r.y + 0.105;
    const mz = r.z - 0.085;
    const maw = (mx * mx) / 0.00068 + (my * my) / 0.0036 + (mz * mz) / 0.0016;
    if (maw < 1.25) {
      if (my > 0.03 && my < 0.058 && fract(mx * 95 + 0.25) < 0.55) {
        return rgb(176, 160, 118);
      }
      if (my < -0.045 && my > -0.062 && fract(mx * 95) < 0.5) {
        return rgb(150, 136, 100);
      }
      return rgb(14, 6, 8);
    }
    // Bone-white where the skull presses through, shadow under it.
    const cheek = clamp(1 - length2(abs(r.x) - 0.048, r.y + 0.022) / 0.03, 0, 1);
    return mix(mix(flesh, rgb(84, 80, 86), clamp((0.04 - r.y) * 3 - 0.1, 0, 0.35)), rgb(232, 228, 214), cheek * 0.4);
  }
  if (py < 0.08) {
    return mix(flesh, rgb(36, 30, 28), 0.55);
  }
  // Hands and the long fingers, the nails black.
  if (py > hip && q.y < 0.2 && abs(q.x) > 0.16) {
    return q.y < -0.12 ? rgb(24, 20, 20) : flesh;
  }
  // What it was buried in, gone grey and stained, torn through to the
  // ribs in places.
  if (py >= hip && (rot > 0.56 || (q.y < 0.04 && noise3(x * 22, 0, z * 22 + k.seed) > 0.45))) {
    if (fract(q.y * 26) < 0.42 && q.y > 0.3 && q.y < 0.55 && abs(q.x) < 0.14 && q.z > 0.02) {
      return rgb(196, 192, 176);
    }
    return mix(flesh, rgb(60, 56, 54), 0.45);
  }
  const stain = clamp(noise3(x * 3, y * 9, z * 3 + k.seed * 2) - 0.45, 0, 1);
  if (py >= hip) {
    return mix(hsv(k.d, 0.1, 0.25 + 0.05 * rot), rgb(46, 34, 28), stain);
  }
  return mix(hsv(k.e, 0.08, 0.18 + 0.04 * rot), rgb(40, 30, 26), stain);
}

export function parts(x, y, z, k) {
  const h = clamp(k.a, 1.9, 2.2) / 1.75;
  const w = clamp(k.b, 0.65, 0.85);
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  // In the stooped torso's frame, as sdf() draws them.
  let q = vec3(0, 0.42, 0);
  if (x < 0.5) {
    const ra = clamp(k.s1, 0, 1) * 3.0;
    const rf = clamp(k.s3, 0, 1) * 1.45;
    q = vec3(0.22 * w + 0.8 * sin(ra) - 0.04, 0.55 - 0.8 * cos(ra) * cos(rf), 0.8 * cos(ra) * sin(rf));
  } else if (x < 1.5) {
    const la = clamp(k.s0, 0, 1) * 3.0;
    const lf = 0.15 + clamp(k.s2, 0, 1) * 1.3;
    q = vec3(-0.22 * w - 0.8 * sin(la), 0.5 - 0.8 * cos(la) * cos(lf), 0.8 * cos(la) * sin(lf));
  } else if (x < 2.5) {
    const nod = clamp(k.s5, -1, 1);
    q = vec3(0.05, 0.74 - nod * 0.03, 0.12 + nod * 0.04);
  }
  const q0 = rotZ(q.x, q.y, q.z, -0.07);
  const p = rotX(q0.x, q0.y, q0.z, clamp(k.s4, -0.3, 0.6));
  return vec3(p.x, p.y + hip, p.z) * h;
}
