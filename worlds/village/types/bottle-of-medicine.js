// A brown glass bottle of syrup with a paper label and a cork.
export const meta = {
  name: "bottle of medicine",
  bounds: [0.04, 0.08, 0.04],
  tags: ["item"],
  props: { mass: 0.2, fragile: 0.8 },
};

export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.05, z, 0.035, 0.05), cylinder(x, y - 0.12, z, 0.012, 0.03));
}

export function color(x, y, z, k) {
  if (abs(y - 0.05) < 0.02) return rgb(240, 236, 220);
  return rgb(120, 70, 30);
}
