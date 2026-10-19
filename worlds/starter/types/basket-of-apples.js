// A bushel basket heaped with red and gold apples from the orchards.
export const meta = {
  name: "basket of apples",
  bounds: [0.25, 0.32, 0.25],
  tags: ["item", "food"],
  props: { mass: 8, edible: 0.8, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const basket = max(cappedCone(x, y - 0.14, z, 0.14, 0.17, 0.23), -cappedCone(x, y - 0.17, z, 0.14, 0.16, 0.22));
  const heap = max(ellipsoid(x, y - 0.26, z, 0.21, 0.07, 0.21) + (noise3(x * 18, y * 18, z * 18) - 0.5) * 0.05, -y + 0.2);
  return min(basket, heap);
}

export function color(x, y, z, k) {
  if (y > 0.24) return noise3(x * 20, y * 20, z * 20) > 0.55 ? rgb(226, 180, 50) : rgb(190, 30, 30);
  return fract(y * 14.0) < 0.3 ? rgb(150, 110, 64) : rgb(196, 156, 100);
}
