// Today's newspaper, folded: the fishing forecast, the football, the feast of San Pietro.
export const meta = {
  name: "newspaper",
  bounds: [0.17, 0.02, 0.12],
  tags: ["item"],
  props: { mass: 0.2, burns: 0.95 },
};

export function sdf(x, y, z, k) {
  return box(x, y - 0.01, z, 0.16, 0.01, 0.11);
}

export function color(x, y, z, k) {
  return fract(z * 18.0) < 0.4 ? rgb(70, 70, 70) : rgb(236, 232, 220);
}
