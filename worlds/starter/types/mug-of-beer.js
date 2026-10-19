// A heavy glass mug of the tavern's amber ale, a head of foam on top.
export const meta = {
  name: "mug of beer",
  bounds: [0.07, 0.17, 0.07],
  tags: ["item", "food", "drink"],
  props: { mass: 0.8, edible: 0.3, fragile: 0.6 },
};

export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.075, z, 0.05, 0.075), max(abs(length2(x - 0.065, y - 0.08) - 0.035) - 0.008, abs(z) - 0.01));
}

export function color(x, y, z, k) {
  if (y > 0.13) return rgb(250, 246, 232);
  return glow(rgb(214, 150, 40), 0.15);
}
