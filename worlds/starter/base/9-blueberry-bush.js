// A highbush blueberry at the meadow's edge: a mound of small leaves turning
// crimson, orange and wine red with autumn, some still green, hung with
// clusters of ripe dusty-blue berries.
export const meta = {
  name: "blueberry bush",
  bounds: [1.2, 1.2, 1.2],
  tags: ["bush", "source"],
  props: { mass: 20, burns: 0.85, alive: 1, fuel: 0.8, edible: 0.3 },
};

export function sdf(x, y, z, k) {
  const s = 0.7 + hash(k.seed) * 0.45;
  const a = ellipsoid(x, y - 0.45 * s, z, 0.95 * s, 0.6 * s, 0.85 * s);
  const b = sphere(x - 0.45 * s, y - 0.6 * s, z + 0.3 * s, 0.5 * s);
  const c = sphere(x + 0.4 * s, y - 0.55 * s, z - 0.35 * s, 0.45 * s);
  const bush = smoothUnion(smoothUnion(a, b, 0.3), c, 0.3);
  const leafy = (bush + (noise3(x * 3, y * 3, z * 3) - 0.5) * 0.3 + (noise3(x * 9, y * 9, z * 9) - 0.5) * 0.08) * 0.75;
  // Berries: little spheres in clusters standing out of the leaves.
  if (leafy > 0.12) return leafy;
  const bx = x - 0.07 * floor(x / 0.07 + 0.5);
  const by = y - 0.07 * floor(y / 0.07 + 0.5);
  const bz = z - 0.07 * floor(z / 0.07 + 0.5);
  const on = step(0.56, noise3(x * 2.5 + k.seed * 0.1, y * 2.5, z * 2.5)) * step(hash(floor(x / 0.07 + 0.5), floor(y / 0.07 + 0.5) * 17 + floor(z / 0.07 + 0.5), k.seed), 0.6);
  const berry = on > 0.5 ? max(sphere(bx, by, bz, 0.022), abs(leafy) - 0.03) : 10.0;
  return min(leafy, berry);
}

export function color(x, y, z, k) {
  // Berries, in clusters over the outside.
  const cluster = noise3(x * 2.5 + k.seed * 0.1, y * 2.5, z * 2.5);
  const bx = x - 0.07 * floor(x / 0.07 + 0.5);
  const by = y - 0.07 * floor(y / 0.07 + 0.5);
  const bz = z - 0.07 * floor(z / 0.07 + 0.5);
  if (cluster > 0.56 && hash(floor(x / 0.07 + 0.5), floor(y / 0.07 + 0.5) * 17 + floor(z / 0.07 + 0.5), k.seed) < 0.6 && length3(bx, by, bz) < 0.026) {
    const b = hash(floor(z / 0.07 + 0.5), floor(x / 0.07 + 0.5));
    return mix(rgb(56, 70, 128), rgb(110, 124, 170), b) * (by > 0.012 ? 1.2 : 1.0);
  }
  const n = noise3(x * 6, y * 6, z * 6);
  const m = noise3(x * 1.2, y * 1.2, z * 1.2);
  const leaf = hash(floor(x * 20.0), floor(z * 20.0) * 7 + floor(y * 20.0));
  if (m < 0.3 && leaf < 0.5) return hsv(0.22, 0.5, 0.32 + n * 0.2);
  return hsv(fract(0.98 + m * 0.07 + (leaf - 0.5) * 0.03), 0.75, 0.32 + n * 0.28) * (0.9 + 0.2 * leaf);
}
