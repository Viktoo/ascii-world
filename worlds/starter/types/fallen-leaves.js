// A drift of fallen leaves on the ground, red maple, gold birch and brown
// oak, blown into a loose patch.
export const meta = {
  name: "fallen leaves",
  bounds: [1.7, 0.3, 1.7],
  tags: ["prop", "leaves", "nonsolid"],
  props: { mass: 1, burns: 0.95 },
};

export function sdf(x, y, z, k) {
  const r = length2(x, z) - 1.0 - (noise3(x * 1.4, k.seed * 0.01, z * 1.4) - 0.5) * 1.2;
  return max(max(r, y - 0.03), -y - 0.25);
}

export function color(x, y, z, k) {
  const g = hash(floor(x * 8.0), floor(z * 8.0), k.seed);
  if (g < 0.15) return rgb(110, 76, 40);
  return hsv(g * 0.13, 0.8, 0.5 + 0.3 * hash(floor(z * 8.0), floor(x * 8.0)));
}
