// A big ribbed orange pumpkin from the garden, its stalk still on.
export const meta = {
  name: "pumpkin",
  bounds: [0.25, 0.4, 0.25],
  tags: ["item", "food"],
  props: { mass: 7, edible: 0.6, burns: 0.3 },
};

export function sdf(x, y, z, k) {
  const a = atan2(z, x);
  const r = 0.23 - 0.018 * abs(sin(a * 5.0));
  const body = (length3(x, (y - 0.17) * 1.35, z) - r) * 0.7;
  return min(body, capsule(x, y, z, 0, 0.26, 0, 0.02, 0.36, 0.01, 0.022));
}

export function color(x, y, z, k) {
  if (y > 0.3) return rgb(96, 110, 50);
  return mix(rgb(214, 100, 20), rgb(236, 130, 30), noise3(x * 10, y * 10, z * 10));
}
