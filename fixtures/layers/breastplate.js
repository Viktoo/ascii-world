// An iron breastplate for the human body: a shell over the torso that
// follows the body's height, crouch and lean.
export const meta = { name: "iron breastplate", bounds: [0.95, 2.4, 0.95], tags: ["layer", "armour"], fits: "figure",
  props: { mass: 12, burns: 0, conducts: 1 } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const w = clamp(k.b, 0.7, 1.4);
  const hip = 0.9 - clamp(k.s6, 0, 1) * 0.38;
  const q = rotX(x / h, y / h - hip, z / h, -clamp(k.s4, -0.3, 0.6));
  const outer = roundBox(q.x, q.y + 0.9 - 1.22, q.z, 0.2 * w, 0.24, 0.13 * w, 0.07);
  const inner = roundBox(q.x, q.y + 0.9 - 1.22, q.z, 0.18 * w, 0.27, 0.11 * w, 0.07);
  return max(outer, -inner) * h;
}

export function color(x, y, z, k) {
  return mix(rgb(150, 155, 165), rgb(210, 215, 225), noise3(x * 9, y * 9, z * 9));
}
