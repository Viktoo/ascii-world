// A leather ball, stitched in red and white panels, scuffed from the
// children's games in the piazza and on the quay.
export const meta = {
  name: "leather ball",
  bounds: [0.12, 0.12, 0.12],
  tags: ["item", "toy"],
  props: { mass: 0.4, toy: 1, bounce: 0.7 },
};

export function sdf(x, y, z, k) {
  return sphere(x, y, z, 0.11);
}

export function color(x, y, z, k) {
  return fract(atan2(z, x) * 0.95) < 0.5 ? rgb(206, 50, 40) : rgb(240, 236, 226);
}
