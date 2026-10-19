// A length of white picket fence, 2 m, round a cottage garden.
export const meta = {
  name: "picket fence",
  bounds: [1.05, 1.0, 0.08],
  tags: ["prop", "fence"],
  props: { mass: 40, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const px = x - 0.16 * floor(x / 0.16 + 0.5);
  const pickets = max(max(box(px, y - 0.45, z, 0.04, 0.45, 0.015), (y - 0.86) + abs(px) * 1.5), abs(x) - 1.0);
  const rails = box(x, abs(y - 0.5) - 0.22, z + 0.03, 1.0, 0.03, 0.015);
  const posts = box(abs(x) - 0.98, y - 0.5, z, 0.04, 0.5, 0.04);
  return min(min(pickets, rails), posts);
}

export function color(x, y, z, k) {
  return rgb(244, 242, 236) * (0.94 + 0.06 * noise3(x * 6, y * 6, z * 6));
}
