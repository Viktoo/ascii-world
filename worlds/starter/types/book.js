// A cloth-bound book of stories, its corners worn.
export const meta = {
  name: "book",
  bounds: [0.12, 0.03, 0.09],
  tags: ["item"],
  props: { mass: 0.5, burns: 0.9 },
};

export function sdf(x, y, z, k) {
  return box(x, y - 0.025, z, 0.11, 0.025, 0.085);
}

export function color(x, y, z, k) {
  if (abs(x) < 0.1 && abs(z) < 0.08 && abs(y - 0.025) < 0.018) return rgb(240, 232, 210);
  return hsv(hash(k.seed), 0.6, 0.5);
}
