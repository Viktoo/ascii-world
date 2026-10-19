// A Mediterranean shrub: rosemary and myrtle mounds, and broom heaped with
// yellow flowers in spring, a whole stand flowering alike.
export const meta = {
  name: "flowering shrub",
  bounds: [1.3, 1.3, 1.3],
  tags: ["bush"],
  props: { mass: 25, burns: 0.85, alive: 1, fuel: 0.8 },
};

export function sdf(x, y, z, k) {
  const s = 0.7 + hash(k.seed) * 0.5;
  const a = ellipsoid(x, y - 0.45 * s, z, 0.95 * s, 0.6 * s, 0.85 * s);
  const b = sphere(x - 0.45 * s, y - 0.35 * s, z + 0.3 * s, 0.5 * s);
  const c = sphere(x + 0.4 * s, y - 0.3 * s, z - 0.4 * s, 0.45 * s);
  const d = smoothUnion(smoothUnion(a, b, 0.25), c, 0.25);
  return (d + (noise3(x * 4, y * 4, z * 4) - 0.5) * 0.22) * 0.8;
}

export function color(x, y, z, k) {
  const n = noise3(x * 5, y * 5, z * 5);
  if (k.a > 0.45 && n > 0.55) return rgb(250, 212, 40);
  if (k.a < 0.2 && n > 0.7) return rgb(170, 140, 220);
  return hsv(0.25 + hash(k.seed, 2) * 0.06, 0.5, 0.3 + n * 0.18);
}
