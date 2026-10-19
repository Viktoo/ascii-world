// A wood-fired pizza margherita on a board: blistered crust, tomato, melted
// mozzarella and basil leaves.
export const meta = {
  name: "pizza",
  bounds: [0.2, 0.05, 0.2],
  tags: ["item", "food"],
  props: { mass: 0.6, edible: 0.8, burns: 0.5 },
};

export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.02, z, 0.18, 0.02), cylinder(x, y - 0.045, z, 0.16, 0.008));
}

export function color(x, y, z, k) {
  const r = length2(x, z);
  if (r > 0.155) return rgb(214, 160, 90);
  const n = noise3(x * 40, 1.0, z * 40);
  if (n > 0.75) return rgb(50, 130, 50);
  if (n > 0.45) return rgb(246, 236, 210);
  return rgb(196, 40, 28);
}
