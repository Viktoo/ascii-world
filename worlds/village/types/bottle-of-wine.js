// A bottle of the house red, a straw-wrapped fiasco.
export const meta = {
  name: "bottle of wine",
  bounds: [0.08, 0.3, 0.08],
  tags: ["item", "food"],
  props: { mass: 1.2, edible: 0.2, fragile: 0.8 },
};

export function sdf(x, y, z, k) {
  return min(sphere(x, y - 0.08, z, 0.08), cylinder(x, y - 0.2, z, 0.02, 0.08));
}

export function color(x, y, z, k) {
  if (y < 0.11) return fract(y * 30.0) < 0.5 ? rgb(196, 160, 90) : rgb(170, 130, 70);
  return rgb(60, 20, 30);
}
