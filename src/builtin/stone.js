// Built-in hand-sized stone.
export const meta = { name: "stone", bounds: [0.13, 0.1, 0.13], tags: ["builtin", "stone", "small", "nonsolid"], props: { mass: 0.7, bounce: 0.35, conducts: 0.3 } };

export function sdf(x, y, z, k) {
  const sx = 0.85 + hash(k.seed) * 0.3;
  return ellipsoid(x, y + 0.01, z, 0.11 * sx, 0.075, 0.1);
}

export function color(x, y, z, k) {
  const g = 110 + hash(k.seed, 2) * 50 + noise3(x * 20, y * 20, z * 20) * 30;
  return rgb(g, g * 0.97, g * 0.93);
}
