// A small white cup of espresso on its saucer.
export const meta = {
  name: "cup of espresso",
  bounds: [0.06, 0.06, 0.06],
  tags: ["item", "food"],
  props: { mass: 0.15, edible: 0.1, fragile: 0.7, heat: 60 },
};

export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.005, z, 0.055, 0.005), cappedCone(x, y - 0.035, z, 0.025, 0.025, 0.032));
}

export function color(x, y, z, k) {
  if (y > 0.058) return rgb(60, 36, 20);
  return rgb(246, 246, 244);
}
