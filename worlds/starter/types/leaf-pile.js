// A heap of raked-up autumn leaves, red, orange, gold and brown, the kind
// children jump into; a rake leaning on it.
export const meta = {
  name: "leaf pile",
  bounds: [1.3, 0.75, 1.2],
  tags: ["prop", "leaves", "nonsolid"],
  props: { mass: 15, burns: 0.95, fuel: 0.3 },
};

export function sdf(x, y, z, k) {
  const heap = ellipsoid(x, y - 0.05, z, 1.2, 0.6, 1.05) + (noise3(x * 3, y * 3, z * 3) - 0.5) * 0.25;
  const rake = capsule(x, y, z, 1.0, 0.0, 0.4, 0.55, 1.35, 0.1, 0.02);
  return min(max(heap * 0.8, -y), max(rake, y - 0.74));
}

export function color(x, y, z, k) {
  if (length2(x - 0.8, z - 0.3) < 0.35 && y > 0.3) return rgb(150, 110, 70);
  const g = hash(floor(x * 9.0), floor(y * 9.0) * 13 + floor(z * 9.0));
  return hsv(0.0 + g * 0.12, 0.8, 0.45 + 0.35 * noise3(x * 12, y * 12, z * 12));
}
