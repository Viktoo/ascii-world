// A tin hand lantern with a glass chimney, its wick burning warm.
export const meta = {
  name: "hand lantern",
  bounds: [0.08, 0.3, 0.08],
  tags: ["item", "tool"],
  props: { mass: 1.2, fragile: 0.5, light: 1, heat: 30 },
};

export function sdf(x, y, z, k) {
  return min(min(cylinder(x, y - 0.03, z, 0.07, 0.03), cylinder(x, y - 0.13, z, 0.045, 0.07)), max(abs(length2(x, y - 0.24) - 0.05) - 0.006, abs(z) - 0.006));
}

export function color(x, y, z, k) {
  if (abs(y - 0.13) < 0.07) return glow(rgb(255, 200, 110), 0.9);
  return rgb(80, 80, 76);
}
