// Clay pots of autumn mums, rust, gold and wine red, standing by a doorway
// or on porch steps, two or three together.
export const meta = {
  name: "pots of mums",
  bounds: [0.6, 0.65, 0.35],
  tags: ["flower", "prop"],
  props: { mass: 12, alive: 1, fragile: 0.5 },
};

export function sdf(x, y, z, k) {
  const px = x - 0.42 * floor(x / 0.42 + 0.5);
  const pots = max(cappedCone(px, y - 0.15, z, 0.15, 0.1, 0.15), abs(x) - 0.58);
  const leaves = max(ellipsoid(px, y - 0.38, z, 0.2, 0.14, 0.2) + (noise3(x * 8, y * 8, z * 8) - 0.5) * 0.06, abs(x) - 0.6);
  return min(pots, leaves);
}

export function color(x, y, z, k) {
  if (y < 0.3) return rgb(186, 92, 54);
  const n = noise3(x * 12, y * 12, z * 12);
  if (n > 0.42) return floor(x / 0.42 + 0.5 + k.seed) % 2 == 0 ? rgb(200, 90, 30) : (floor(x / 0.42 + 0.5 + k.seed) % 3 == 1 ? rgb(236, 180, 40) : rgb(150, 30, 50));
  return rgb(56, 110, 44);
}
