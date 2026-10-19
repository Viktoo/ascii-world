// A fisherman's net bundled up for carrying: folds of green mesh with a line
// of orange cork floats along one edge and a coil of rope tied round it.
export const meta = {
  name: "fishing net",
  bounds: [0.42, 0.24, 0.32],
  tags: ["item", "tool"],
  props: { mass: 3, burns: 0.7 },
};

export function sdf(x, y, z, k) {
  const heap = (ellipsoid(x, y - 0.04, z, 0.38, 0.17, 0.28) + (noise3(x * 6, y * 6, z * 6) - 0.5) * 0.08) * 0.8;
  const fx = x - 0.12 * floor(x / 0.12 + 0.5);
  const floats = max(ellipsoid(fx, y - 0.14 + x * 0.1, z - 0.2 + abs(x) * 0.15, 0.04, 0.03, 0.03), abs(x) - 0.34);
  const rope = max(abs(length2(x - 0.1, y - 0.04) - 0.15) - 0.014, abs(z) - 0.022);
  return min(max(heap, -y), min(floats, max(rope, -y)));
}

export function color(x, y, z, k) {
  if (abs(length2(x - 0.1, y - 0.04) - 0.15) < 0.02 && abs(z) < 0.03) return fract(atan2(y - 0.05, x - 0.1) * 6.0) < 0.5 ? rgb(196, 170, 120) : rgb(170, 146, 100);
  if (abs(z - 0.2 + abs(x) * 0.15) < 0.04 && y > 0.1) return rgb(240, 130, 40);
  const m = step(0.82, fract(x * 18.0 + z * 18.0)) + step(0.82, fract(x * 18.0 - z * 18.0));
  return m > 0.0 ? rgb(90, 140, 100) : rgb(36, 70, 46) * (0.8 + 0.2 * noise3(x * 10, y * 10, z * 10));
}
