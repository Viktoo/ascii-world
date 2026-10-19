// A golden apple pie with a lattice top, still warm from the bakery's oven.
export const meta = {
  name: "apple pie",
  bounds: [0.15, 0.05, 0.15],
  tags: ["item", "food"],
  props: { mass: 1.0, edible: 0.9, burns: 0.4 },
};

export function sdf(x, y, z, k) {
  return min(cappedCone(x, y - 0.025, z, 0.025, 0.12, 0.14), ellipsoid(x, y - 0.045, z, 0.13, 0.02, 0.13));
}

export function color(x, y, z, k) {
  if (y > 0.05) return (fract(x * 12.0) < 0.3 || fract(z * 12.0) < 0.3) ? rgb(220, 160, 80) : rgb(150, 70, 30);
  return rgb(200, 140, 70);
}
