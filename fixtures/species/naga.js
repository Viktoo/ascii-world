// A naga body: a person's chest and arms on a serpent's tail. No raised
// arms in this one (it holds them forward); it coils down instead of
// crouching. Roles: reach, lean, head, crouch, spread (tail swish).
export const meta = { name: "naga", bounds: [0.9, 1.6, 2.2], tags: ["body"],
  props: { mass: 90, alive: 1 },
  body: { height: 1.9, eye: 1.75, radius: 0.4, reach: 2.2, grip: [0.3, 1.2, 0.45],
          roles: ["reach", "lean", "head", "crouch", "spread"], gait: "slither", arms: false,
          look: { hue: [0, 1] } } };

export function sdf(x, y, z, k) {
  const coil = clamp(k.s6, 0, 1);
  const top = 1.1 - coil * 0.45;
  const sw = sin(k.f) * 0.25 + clamp(k.s7, -1, 1) * 0.3;
  const tail = min(
    capsule(x, y, z, 0, top, 0, sw * 0.4, 0.25, -0.6, 0.2),
    capsule(x, y, z, sw * 0.4, 0.25, -0.6, -sw, 0.15, -1.4, 0.15),
    capsule(x, y, z, -sw, 0.15, -1.4, sw, 0.08, -2.0, 0.08)
  );
  const q = rotX(x, y - top, z, -clamp(k.s4, -0.3, 0.6));
  const torso = roundBox(q.x, q.y - 0.35, q.z, 0.2, 0.3, 0.12, 0.07);
  const fl = clamp(k.s2, 0, 1) * 1.4;
  const fr = clamp(k.s3, 0, 1) * 1.4;
  const arms = min(
    capsule(q.x, q.y, q.z, -0.26, 0.6, 0, -0.32, 0.6 - 0.5 * cos(fl), 0.5 * sin(fl), 0.055),
    capsule(q.x, q.y, q.z, 0.26, 0.6, 0, 0.32, 0.6 - 0.5 * cos(fr), 0.5 * sin(fr), 0.055)
  );
  const nod = clamp(k.s5, -1, 1);
  const head = sphere(q.x, q.y - 0.82 + nod * 0.03, q.z - nod * 0.05, 0.12);
  return min(smoothUnion(tail, torso, 0.1), arms, head);
}

export function color(x, y, z, k) {
  if (y > 1.0 && z > -0.3) {
    return rgb(200, 170, 140);
  }
  return hsv(k.a, 0.6, 0.5 + 0.2 * step(0.5, fract(z * 4)));
}
