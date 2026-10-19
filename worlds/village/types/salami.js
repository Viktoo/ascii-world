// A whole salami in its string net, for slicing.
export const meta = {
  name: "salami",
  bounds: [0.04, 0.04, 0.17],
  tags: ["item", "food"],
  props: { mass: 0.6, edible: 0.7 },
};

export function sdf(x, y, z, k) {
  return capsule(x, y, z, 0, 0, -0.13, 0, 0, 0.13, 0.04);
}

export function color(x, y, z, k) {
  return fract(z * 20.0) < 0.15 ? rgb(240, 236, 226) : rgb(150, 70, 60);
}
