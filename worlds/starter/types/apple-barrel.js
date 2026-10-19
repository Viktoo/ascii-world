// An open wooden barrel on the store's porch, heaped with red apples for sale.
export const meta = {
  name: "apple barrel",
  bounds: [0.38, 0.95, 0.38],
  tags: ["prop", "shop", "source"],
  props: { mass: 60, edible: 0.8, burns: 0.5 },
};

export function sdf(x, y, z, k) {
  const barrel = max(length2(x, z) - 0.33 - 0.04 * (1.0 - abs(y - 0.42) / 0.42), abs(y - 0.42) - 0.42);
  const heap = ellipsoid(x, y - 0.84, z, 0.3, 0.1, 0.3) + (noise3(x * 16, y * 16, z * 16) - 0.5) * 0.06;
  return min(barrel, heap);
}

export function color(x, y, z, k) {
  if (y > 0.8) return rgb(190, 30, 30);
  if (abs(abs(y - 0.42) - 0.28) < 0.03) return rgb(60, 60, 62);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
