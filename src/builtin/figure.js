// Built-in character figure. Params: a = height (m), b = build, c = skin tone,
// d = shirt hue, e = trousers hue, f = walk phase (radians). Faces +z.
export const meta = { name: "figure", bounds: [0.7, 2.1, 0.7], tags: ["builtin", "figure"] };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const w = clamp(k.b, 0.7, 1.4);
  const px = x / h;
  const py = y / h;
  const pz = z / h;
  const sw = sin(k.f) * 0.32;
  const legs = min(
    capsule(px, py, pz, -0.1 * w, 0.9, 0, -0.1 * w, 0.09, sw, 0.075 * w),
    capsule(px, py, pz, 0.1 * w, 0.9, 0, 0.1 * w, 0.09, -sw, 0.075 * w)
  );
  const torso = roundBox(px, py - 1.2, pz, 0.18 * w, 0.28, 0.11 * w, 0.07);
  const arms = min(
    capsule(px, py, pz, -0.25 * w, 1.42, 0, -0.29 * w, 0.86, -sw * 0.8, 0.055 * w),
    capsule(px, py, pz, 0.25 * w, 1.42, 0, 0.29 * w, 0.86, sw * 0.8, 0.055 * w)
  );
  const head = sphere(px, py - 1.62, pz, 0.12);
  const hair = sphere(px, py - 1.66, pz + 0.015, 0.125);
  const nose = sphere(px, py - 1.61, pz - 0.115, 0.025);
  const body = smoothUnion(min(legs, arms), torso, 0.06);
  return min(smoothUnion(body, min(head, hair), 0.05), nose) * h;
}

export function color(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const py = y / h;
  const pz = z / h;
  const skin = mix(rgb(244, 208, 178), rgb(96, 62, 42), clamp(k.c, 0, 1));
  const hairc = mix(rgb(30, 22, 18), rgb(200, 160, 90), hash(k.seed) * hash(k.seed, 1));
  if (py > 1.52) {
    if (py > 1.69 || (pz < -0.02 && py > 1.58)) {
      return hairc;
    }
    return skin;
  }
  if (py < 0.13) {
    return rgb(46, 36, 30);
  }
  if (py < 0.9 && abs(x / h) > 0.2) {
    return skin;
  }
  if (py >= 0.9) {
    return hsv(k.d, 0.55, 0.78);
  }
  return hsv(k.e, 0.35, 0.42);
}
