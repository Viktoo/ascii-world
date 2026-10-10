// Built-in phantom body: a person, risen wrong. Written from the human figure
// (what fits a person fits it), but tall and wasted, stooped and with one arm
// always reaching (its rest stance, so what it wears follows), the head hanging
// forward and over to one side, the other arm hanging dead, a dragging left leg; skin like candle wax, sunk
// sockets with two cold white lights in them, clothes gone grey and torn.
// Look: a = height (m), b = build, c = skin (wax … ash), d = shirt hue,
// e = trousers hue, f = walk phase (radians). Faces +z.
// Roles (live pose): s0/s1 = left/right arm raised sideways (0 down … 1 up),
// s2/s3 = left/right arm reaching forward (0 … 1), s4 = lean forward (radians),
// s5 = head nod, s6 = crouch (0 … 1).
export const meta = { name: "phantom", bounds: [1.0, 2.6, 1.1], tags: ["builtin", "body"],
  props: { mass: 55, alive: 1 },
  body: { height: 2.0, eye: 1.76, radius: 0.33, reach: 2.6, grip: [0.3, 1.0, 0.3], from: "figure",
          roles: ["raise", "reach", "lean", "head", "crouch"], gait: "biped", arms: true,
          rest: { lean: 0.3, reach_r: 0.6, head: 0.25 },
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
    capsule(px, py, pz, -0.1 * w, hip, 0, -0.11 * w, ky, kz + lsw * 0.5, 0.065 * w),
    capsule(px, py, pz, -0.11 * w, ky, kz + lsw * 0.5, -0.13 * w, 0.07, lsw - 0.03, 0.05 * w),
    capsule(px, py, pz, 0.1 * w, hip, 0, 0.1 * w, ky, kz + rsw * 0.5, 0.065 * w),
    capsule(px, py, pz, 0.1 * w, ky, kz + rsw * 0.5, 0.11 * w, 0.07, rsw, 0.05 * w),
    // The left foot turned in, scraping.
    capsule(px, py, pz, -0.13 * w, 0.04, lsw - 0.03, -0.08 * w, 0.03, lsw + 0.1, 0.04)
  );
  // Stooped from the hip, the whole upper body tipped a little to the left.
  const q0 = rotX(px, py - hip, pz, -clamp(k.s4, -0.3, 0.6));
  const q = rotZ(q0.x, q0.y, q0.z, 0.07);
  const torso = roundBox(q.x, q.y - 0.3, q.z, 0.16 * w, 0.28, 0.09 * w, 0.06);
  // The left shoulder slumped lower than the right.
  const lsy = 0.5;
  const rsy = 0.55;
  const arm = 0.68;
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
  const arms = min(
    capsule(q.x, q.y, q.z, -0.2 * w, lsy, 0, lhx, lhy, lhz, 0.042 * w),
    capsule(q.x, q.y, q.z, 0.2 * w, rsy, 0, rhx, rhy, rhz, 0.042 * w)
  );
  // Long fingers, curled.
  const fingers = min(
    capsule(q.x, q.y, q.z, lhx, lhy, lhz, lhx + 0.01, lhy - 0.12, lhz + 0.03, 0.014),
    capsule(q.x, q.y, q.z, rhx, rhy, rhz, rhx + 0.01, rhy - 0.06, rhz + 0.11, 0.014)
  );
  // The head hangs forward on its neck, over to one side.
  const nod = clamp(k.s5, -1, 1);
  const hx = 0.05;
  const hy = 0.74 - nod * 0.03;
  const hz = 0.12 + nod * 0.04;
  const neck = capsule(q.x, q.y, q.z, 0, 0.56, 0, hx * 0.7, hy - 0.05, hz - 0.04, 0.04);
  const r = rotZ(q.x - hx, q.y - hy, q.z - hz, -0.3);
  const skull = ellipsoid(r.x, r.y, r.z, 0.095, 0.12, 0.11);
  // A slack jaw hanging open.
  const jaw = ellipsoid(r.x, r.y + 0.1, r.z - 0.03, 0.07, 0.045, 0.075);
  const head = smoothUnion(skull, jaw, 0.025);
  // Two lights sunk in the face, set forward enough to be seen.
  const eyes = min(sphere(r.x - 0.038, r.y - 0.01, r.z - 0.088, 0.022), sphere(r.x + 0.038, r.y - 0.01, r.z - 0.088, 0.022));
  const upper = smoothUnion(torso, min(arms, fingers), 0.04);
  const body = smoothUnion(legs, upper, 0.06);
  return min(smoothUnion(body, smoothUnion(neck, head, 0.03), 0.04), eyes) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 1.9, 2.2) / 1.75;
  const crouch = clamp(k.s6, 0, 1);
  const hip = 0.9 - crouch * 0.38;
  const py = y / h;
  const q0 = rotX(x / h, py - hip, z / h, -clamp(k.s4, -0.3, 0.6));
  const q = rotZ(q0.x, q0.y, q0.z, 0.07);
  const nod = clamp(k.s5, -1, 1);
  const skin = mix(rgb(226, 222, 206), rgb(150, 150, 146), clamp(k.c, 0, 1));
  const rot = noise3(x * 6, y * 6, z * 6 + k.seed);
  // The head and neck.
  if (q.y > 0.6 && py > hip) {
    const r = rotZ(q.x - 0.05, q.y - 0.74 + nod * 0.03, q.z - 0.12 - nod * 0.04, -0.3);
    // The eyes: two cold white lights, easy to see in the dark.
    const ex = abs(r.x) - 0.038;
    const ey = r.y - 0.01;
    const ez = r.z - 0.088;
    if (ex * ex + ey * ey + ez * ez < 0.0007) {
      return glow(rgb(230, 242, 255), 1);
    }
    // Sunk, bruised sockets around them.
    if (ex * ex + ey * ey * 1.6 < 0.0022 && r.z > 0.03) {
      return rgb(52, 46, 54);
    }
    // The open mouth, dark inside.
    if (r.z > 0.02 && r.y < -0.06 && r.y > -0.12 && abs(r.x) < 0.045) {
      return rgb(30, 18, 20);
    }
    // Thin lank hair over the back of the skull.
    if (r.y > 0.07 || (r.z < -0.02 && r.y > -0.03)) {
      return rgb(28, 26, 24);
    }
    return mix(skin, rgb(120, 126, 118), clamp(rot * 0.6, 0, 0.35));
  }
  if (py < 0.1) {
    return rgb(30, 28, 26);
  }
  // Hands and the long fingers.
  if (py > hip && q.y < 0.42 && abs(q.x) > 0.17) {
    return skin;
  }
  // What it was buried in, gone grey, torn through to the skin in places.
  if (rot > 0.55 && py >= hip) {
    return mix(skin, rgb(90, 84, 78), 0.35);
  }
  if (py >= hip) {
    return hsv(k.d, 0.1, 0.27 + 0.06 * rot);
  }
  return hsv(k.e, 0.08, 0.19 + 0.04 * rot);
}
