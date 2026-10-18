// A little lemon tree in a big terracotta pot: a slim trunk, a round dark
// green crown hung with bright yellow lemons.
export const meta = {
  name: "lemon tree",
  bounds: [0.85, 2.35, 0.85],
  tags: ["tree", "prop"],
  props: { mass: 80, alive: 1, burns: 0.5 },
};

export function sdf(x, y, z, k) {
  const pot = cappedCone(x, y - 0.3, z, 0.3, 0.3, 0.42);
  const trunk = cylinder(x, y - 0.9, z, 0.05, 0.4);
  const crown = sphere(x, y - 1.55, z, 0.62) + (noise3(x * 4, y * 4, z * 4) - 0.5) * 0.2;
  return min(min(pot, trunk), crown * 0.8);
}

export function color(x, y, z, k) {
  if (y < 0.62) return rgb(190, 96, 56);
  if (y < 1.0) return rgb(100, 76, 50);
  return noise3(x * 9, y * 9, z * 9) > 0.72 ? rgb(250, 222, 40) : rgb(40, 96, 40);
}
