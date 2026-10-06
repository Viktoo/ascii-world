// Built-in phantom body: a person, almost. Written from the human figure
// (what fits a person fits it), but too tall and too thin, arms too long, a
// long neck, the head tipped to one side; skin like candle wax, sunk dark
// eyes with a faint pale light far back in them, clothes gone grey.
// Look: a = height (m), b = build, c = skin (wax … ash), d = shirt hue,
// e = trousers hue, f = walk phase (radians). Faces +z.
// Roles (live pose): s0/s1 = left/right arm raised sideways (0 down … 1 up),
// s2/s3 = left/right arm reaching forward (0 … 1), s4 = lean forward (radians),
// s5 = head nod, s6 = crouch (0 … 1).
export const meta = { name: "phantom", bounds: [1.0, 2.6, 1.0], tags: ["builtin", "body"],
  props: { mass: 55, alive: 1 },
  body: { height: 2.0, eye: 1.86, radius: 0.33, reach: 2.6, grip: [0.3, 1.0, 0.3], from: "figure",
          roles: ["raise", "reach", "lean", "head", "crouch"], gait: "biped", arms: true,
          look: { height: [1.9, 2.2], build: [0.65, 0.85], skin: [0, 1], shirt: [0, 1], trousers: [0, 1] } } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.9, 2.2) / 1.75;
  const w = clamp(k.b, 0.65, 0.85);
  const px = x / h;
  const py = y / h;
  const pz = z / h;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const still = 1 - clamp(max(k.s2, k.s3) * 2 + crouch, 0, 1);
  // A slow, even stride.
  const sw = sin(k.f) * 0.26 * (1 - crouch);
  const kz = crouch * 0.32;
  const ky = 0.5 - crouch * 0.12;
  const legs = min(
    capsule(px, py, pz, -0.09 * w, hip, 0, -0.09 * w, ky, kz + sw * 0.5, 0.06 * w),
    capsule(px, py, pz, -0.09 * w, ky, kz + sw * 0.5, -0.09 * w, 0.08, sw, 0.05 * w),
    capsule(px, py, pz, 0.09 * w, hip, 0, 0.09 * w, ky, kz - sw * 0.5, 0.06 * w),
    capsule(px, py, pz, 0.09 * w, ky, kz - sw * 0.5, 0.09 * w, 0.08, -sw, 0.05 * w)
  );
  // Upper body: a slight stoop always, more when it leans.
  const q = rotX(px, py - hip, pz, -clamp(k.s4 + 0.12, -0.3, 0.7));
  const ux = q.x;
  const uy = q.y + 0.9;
  const uz = q.z;
  const torso = roundBox(ux, uy - 1.2, uz, 0.15 * w, 0.29, 0.085 * w, 0.06);
  // Arms too long: the hands hang past the knees.
  const arm = 0.7;
  const la = clamp(k.s0, 0, 1) * 3.0;
  const ra = clamp(k.s1, 0, 1) * 3.0;
  const lf = clamp(k.s2, 0, 1) * 1.45;
  const rf = clamp(k.s3, 0, 1) * 1.45;
  const lhx = -0.21 * w - 0.03 * w - arm * sin(la);
  const lhy = 1.42 - arm * cos(la) * cos(lf);
  const lhz = arm * cos(la) * sin(lf) - sw * 0.6 * still;
  const rhx = 0.21 * w + 0.03 * w + arm * sin(ra);
  const rhy = 1.42 - arm * cos(ra) * cos(rf);
  const rhz = arm * cos(ra) * sin(rf) + sw * 0.6 * still;
  const arms = min(
    capsule(ux, uy, uz, -0.21 * w, 1.42, 0, lhx, lhy, lhz, 0.04 * w),
    capsule(ux, uy, uz, 0.21 * w, 1.42, 0, rhx, rhy, rhz, 0.04 * w)
  );
  // Long fingers.
  const fingers = min(
    capsule(ux, uy, uz, lhx, lhy, lhz, lhx - 0.02, lhy - 0.13, lhz + 0.02, 0.012),
    capsule(ux, uy, uz, rhx, rhy, rhz, rhx + 0.02, rhy - 0.13, rhz + 0.02, 0.012)
  );
  // A long neck, the head tipped over to one side.
  const nod = clamp(k.s5, -1, 1);
  const tip = 0.09;
  const hx = ux - tip;
  const hy = uy - 1.68 + nod * 0.03;
  const hz = uz - nod * 0.05;
  const neck = capsule(ux, uy, uz, 0, 1.47, 0, tip * 0.6, 1.62, 0, 0.035);
  const head = ellipsoid(hx + (hy) * 0.35, hy, hz, 0.1, 0.13, 0.11);
  const body = smoothUnion(min(legs, min(arms, fingers)), torso, 0.05);
  return smoothUnion(body, smoothUnion(neck, head, 0.03), 0.04) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 1.9, 2.2) / 1.75;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const q = rotX(x / h, y / h - hip, z / h, -clamp(k.s4 + 0.12, -0.3, 0.7));
  const py = y / h;
  const ux = q.x;
  const uy = q.y + 0.9;
  const uz = q.z;
  const nod = clamp(k.s5, -1, 1);
  const skin = mix(rgb(226, 222, 206), rgb(150, 150, 146), clamp(k.c, 0, 1));
  // The head and neck.
  if (uy > 1.47 && py > hip) {
    const hx = ux - 0.09 + (uy - 1.68 + nod * 0.03) * 0.35;
    const hy = uy - 1.68 + nod * 0.03;
    const hz = uz - nod * 0.05;
    // Two sunk eyes, dark, with a faint pale light far back.
    const ex = abs(hx) - 0.04;
    const ey = hy - 0.015;
    const ez = hz + 0.1;
    if (ex * ex + ey * ey + ez * ez < 0.0009) {
      return glow(rgb(200, 210, 230), 0.25);
    }
    // A thin mouth, too wide.
    if (hz < -0.08 && abs(hy + 0.06) < 0.008 && abs(hx) < 0.06) {
      return rgb(40, 30, 30);
    }
    // Lank dark hair.
    if (hy > 0.06 || (hz > 0.02 && hy > -0.06)) {
      return rgb(22, 20, 20);
    }
    return skin;
  }
  if (py < 0.11) {
    return rgb(30, 28, 26);
  }
  // Hands and the long fingers.
  if (py > hip && uy < 1.3 && abs(q.x) > 0.17) {
    return skin;
  }
  // What it was buried in, gone grey.
  if (py >= hip) {
    return hsv(k.d, 0.12, 0.3 + 0.05 * noise3(x * 7, y * 7, z * 7 + k.seed));
  }
  return hsv(k.e, 0.1, 0.2);
}
