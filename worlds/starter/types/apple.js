// A crisp red apple from the orchards, its stalk and a leaf still on.
export const meta = {
  name: "apple",
  bounds: [0.05, 0.1, 0.05],
  tags: ["item", "food"],
  props: { mass: 0.2, edible: 0.6, burns: 0.3 },
};

export function sdf(x, y, z, k) {
  const a = smoothSubtract(sphere(x, y - 0.042, z, 0.042), sphere(x, y - 0.09, z, 0.016), 0.012);
  return min(a, capsule(x, y, z, 0, 0.07, 0, 0.006, 0.095, 0.0, 0.004));
}

export function color(x, y, z, k) {
  if (y > 0.075) return rgb(90, 70, 40);
  return mix(rgb(176, 24, 28), rgb(226, 80, 44), noise3(x * 40, y * 40, z * 40)) * (0.85 + 0.15 * step(0.03, y));
}
