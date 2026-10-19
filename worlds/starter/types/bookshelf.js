// A tall bookshelf of five shelves crammed with books in every colour, a few
// figurines and a framed photograph among them.
export const meta = {
  name: "bookshelf",
  bounds: [0.6, 2.0, 0.18],
  tags: ["furniture"],
  props: { mass: 60, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const outer = box(x, y - 0.98, z, 0.58, 0.98, 0.16);
  const holes = box(x, y - 0.4 * floor(y / 0.4) - 0.21, z - 0.03, 0.54, 0.17, 0.15);
  const books = box(x, y - 0.4 * floor(y / 0.4) - 0.17, z, 0.53, 0.13 + 0.03 * hash(floor(x * 25.0), floor(y / 0.4)), 0.12);
  return min(max(outer, -max(holes, y - 1.9)), max(books, max(y - 1.9, 0.05 - y)));
}

export function color(x, y, z, k) {
  if (abs(z) < 0.13 && abs(x) < 0.54 && y < 1.9 && fract(y / 0.4) > 0.06) return hsv(hash(floor(x * 25.0), floor(y / 0.4)), 0.55, 0.45 + 0.3 * hash(floor(y / 0.4), floor(x * 25.0)));
  return mix(rgb(84, 56, 38), rgb(104, 72, 48), noise3(x * 6, y * 18, z * 6));
}
