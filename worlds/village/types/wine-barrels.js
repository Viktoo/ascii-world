// Three oak wine barrels on their sides on a rack, each with a spigot, the
// osteria's house red and white.
export const meta = {
  name: "wine barrels",
  bounds: [1.3, 1.2, 0.5],
  tags: ["furniture", "prop"],
  props: { mass: 300, burns: 0.5, fuel: 0.4 },
};

export function sdf(x, y, z, k) {
  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  const barrels = max(max(length2(bx, y - 0.62) - 0.38 - 0.05 * (1.0 - abs(z) / 0.45), abs(z) - 0.45), abs(x) - 1.25);
  const rack = box(x, y - 0.12, z, 1.25, 0.12, 0.4);
  return min(barrels, rack);
}

export function color(x, y, z, k) {
  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  if (abs(abs(z) - 0.32) < 0.03 || abs(z) > 0.43) return rgb(60, 60, 62);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
