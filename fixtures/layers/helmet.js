// An iron helmet for the human body: a shell over the skull that follows
// the body's height, lean and nod, open at the face.
export const meta = { name: "iron helmet", bounds: [0.95, 2.4, 0.95], tags: ["layer", "armour"], fits: "figure",
  props: { mass: 2, burns: 0, conducts: 1 } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const hip = 0.9 - clamp(k.s6, 0, 1) * 0.38;
  const q = rotX(x / h, y / h - hip, z / h, -clamp(k.s4, -0.3, 0.6));
  const hq = rotX(q.x, q.y + 0.9 - 1.56, q.z, clamp(k.s5, -1, 1) * 0.35);
  const hy = hq.y - 0.06;
  const dome = ellipsoid(hq.x, hy - 0.01, hq.z + 0.005, 0.118, 0.13, 0.125);
  const shell = max(dome, -ellipsoid(hq.x, hy - 0.01, hq.z + 0.005, 0.104, 0.116, 0.111));
  // Down to the ears; the face left open.
  return max(max(shell, -0.02 - hy), -max(0.06 - hq.z, hy)) * h;
}

export function color(x, y, z, k) {
  return mix(rgb(120, 124, 132), rgb(190, 195, 205), noise3(x * 14, y * 14, z * 14));
}
