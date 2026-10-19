// A drift of fallen leaves on the ground: red maple, gold birch and brown
// oak leaves in a loose patch, thickest in its middle.
export const meta = {
  name: "leaf litter",
  bounds: [1.1, 0.3, 1.1],
  tags: ["leaves", "small", "nonsolid"],
  props: { mass: 1, burns: 0.95, alive: 0 },
};

export function sdf(x, y, z, k) {
  const r = length2(x, z) - 0.65 - (noise3(x * 2.2, k.seed * 0.01, z * 2.2) - 0.5) * 0.7;
  // Sunk a little into the ground, so on a slope it lies on it, not over it.
  return max(max(r, y - 0.035 + length2(x, z) * 0.03), -y - 0.25);
}

export function color(x, y, z, k) {
  const g = hash(floor(x * 9.0), floor(z * 9.0), k.seed);
  if (g < 0.15) return rgb(110, 76, 40);
  return hsv(g * 0.13, 0.82, 0.5 + 0.3 * hash(floor(z * 9.0), floor(x * 9.0)));
}
