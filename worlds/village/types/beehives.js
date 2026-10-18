// Three straw beehive skeps on a plank bench, round domes of coiled golden
// straw with a dark doorway at the foot of each.
export const meta = {
  name: "beehives",
  bounds: [1.4, 1.15, 0.45],
  tags: ["prop"],
  props: { mass: 40, burns: 0.8, alive: 1 },
  sound: { hard: 0.1, dry: 0.9, ring: 0.0 },
};

export function sdf(x, y, z, k) {
  const legs = box(abs(x) - 1.1, y - 0.25, z, 0.06, 0.25, 0.3);
  const bench = box(x, y - 0.52, z, 1.35, 0.04, 0.4);
  const sx = x - 0.85 * floor(x / 0.85 + 0.5);
  const skeps = max(max(ellipsoid(sx, y - 0.56, z, 0.34, 0.55, 0.34), 0.56 - y), abs(x) - 1.3);
  return min(min(legs, bench), skeps);
}

export function color(x, y, z, k) {
  if (y < 0.57) return rgb(110, 82, 54);
  const sx = x - 0.85 * floor(x / 0.85 + 0.5);
  if (z > 0.2 && abs(sx) < 0.08 && y < 0.72) return rgb(30, 24, 16);
  return fract(y * 12.0) < 0.3 ? rgb(176, 136, 58) : rgb(218, 180, 90);
}
