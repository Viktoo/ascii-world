// The gelato counter: a glass case of tubs heaped with ice cream in pistachio,
// strawberry, lemon, chocolate, stracciatella and more, cones stacked on top.
export const meta = {
  name: "gelato counter",
  bounds: [1.1, 1.3, 0.4],
  tags: ["furniture", "counter", "shop"],
  props: { mass: 150, light: 1 },
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 0.45, z, 1.05, 0.45, 0.36);
  const glass = max(box(x, y - 1.0, z - 0.05, 1.05, 0.1, 0.3), (z - 0.1) + (y - 0.9) * 0.6);
  const tubs = max(ellipsoid(x - 0.3 * floor(x / 0.3 + 0.5), y - 0.95, z - 0.1 * 0.0, 0.13, 0.06, 0.25), abs(x) - 1.0);
  const cones = max(cappedCone(x - 0.8, y - 1.18, z + 0.2, 0.1, 0.02, 0.05), 0.0 - 0.0 - 1.0);
  return min(min(body, tubs), min(glass * 1.0, cones));
}

export function color(x, y, z, k) {
  if (y > 0.86 && y < 1.03 && abs(z) < 0.3) return hsv(fract(floor(x / 0.3 + 0.5) * 0.17 + 0.05), 0.45, 0.95);
  if (y > 1.05) return rgb(210, 160, 90);
  if (y > 0.9) return glow(rgb(220, 236, 240), 0.2);
  return rgb(240, 236, 228);
}
