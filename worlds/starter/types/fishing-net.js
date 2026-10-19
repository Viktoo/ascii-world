// A fisherman's net bundled up for carrying: a heap of green mesh with a
// line of orange cork floats along it.
export const meta = {
  name: "fishing net",
  bounds: [0.4, 0.2, 0.3],
  tags: ["item", "tool"],
  props: { mass: 3, burns: 0.7 },
};

export function sdf(x, y, z, k) {
  return min(ellipsoid(x, y - 0.0, z, 0.38, 0.18, 0.28) + (noise3(x * 8, y * 8, z * 8) - 0.5) * 0.04, sphere(x - 0.25, y - 0.12, z, 0.06));
}

export function color(x, y, z, k) {
  if (fract(x * 4.0 + z * 2.0) < 0.15) return rgb(240, 140, 40);
  return fract(x * 20 + z * 20) < 0.3 ? rgb(40, 80, 50) : rgb(70, 120, 80);
}
