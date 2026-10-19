// The florist's stand outside her shop: tiered wooden steps of zinc buckets
// full of roses, sunflowers, lilies, carnations and lavender.
export const meta = {
  name: "flower stand",
  bounds: [1.0, 1.2, 0.6],
  tags: ["prop", "shop"],
  props: { mass: 60, alive: 1, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const steps = min(box(x, y - 0.2, z - 0.25, 0.95, 0.2, 0.3), box(x, y - 0.5, z + 0.2, 0.95, 0.5, 0.25));
  const bx = x - 0.32 * floor(x / 0.32 + 0.5);
  const lowb = max(ellipsoid(bx, y - 0.65, z - 0.25, 0.14, 0.25, 0.14), abs(x) - 0.9);
  const highb = max(ellipsoid(bx, y - 1.05, z + 0.2, 0.14, 0.2, 0.14), abs(x) - 0.9);
  return min(steps, min(lowb, highb));
}

export function color(x, y, z, k) {
  if ((y > 0.4 && z < 0.0) || y > 1.0) {
    if (noise3(x * 14, y * 14, z * 14) > 0.4) return hsv(fract(floor(x / 0.32 + 0.5) * 0.23 + floor(y * 2.0) * 0.4), 0.7, 0.95);
    return rgb(60, 120, 50);
  }
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
