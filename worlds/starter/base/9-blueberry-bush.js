// A low highbush blueberry, its small leaves gone deep crimson and wine red
// in the autumn, a mound of red among the rocks and the meadow's edge.
export const meta = {
  name: "blueberry bush",
  bounds: [1.2, 1.2, 1.2],
  tags: ["bush"],
  props: { mass: 20, burns: 0.85, alive: 1, fuel: 0.8 },
};

export function sdf(x, y, z, k) {
  const s = 0.7 + hash(k.seed) * 0.45;
  const a = ellipsoid(x, y - 0.45 * s, z, 0.95 * s, 0.6 * s, 0.85 * s);
  const b = sphere(x - 0.45 * s, y - 0.6 * s, z + 0.3 * s, 0.5 * s);
  return (smoothUnion(a, b, 0.3) + (noise3(x * 3, y * 3, z * 3) - 0.5) * 0.3) * 0.8;
}

export function color(x, y, z, k) {
  const n = noise3(x * 6, y * 6, z * 6);
  return hsv(0.98 + hash(k.seed, 2) * 0.04 + (n - 0.5) * 0.03, 0.75, 0.32 + n * 0.28);
}
