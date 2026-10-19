// An eastern white pine: a tall straight trunk and soft, uneven tiers of
// long dark blue-green needles, the evergreen among the turning woods.
export const meta = {
  name: "white pine",
  bounds: [3.0, 11.6, 3.0],
  tags: ["pine"],
  props: { mass: 1500, burns: 0.75, alive: 1, fuel: 1 },
};

export function sdf(x, y, z, k) {
  const s = 0.8 + hash(k.seed) * 0.25;
  const trunk = cylinder(x, y - 5.0 * s, z, 0.24 * s, 5.0 * s);
  const far = length3(x, y - 7.4 * s, z * 1.0) - 4.4 * s;
  if (far > 0.4) return min(trunk, far * 0.85);
  // Tiers: fold y into the nearest of five, each a flat, lumpy layer.
  const t = clamp(floor((y - 3.4 * s) / (1.7 * s) + 0.5), 0, 4);
  const ty = 3.4 * s + t * 1.7 * s;
  const r = (2.7 - t * 0.42) * s;
  const ox = (hash(k.seed, t) - 0.5) * 0.8;
  const tier = ellipsoid(x - ox, y - ty, z + ox * 0.6, r, 0.55 * s, r * 0.9);
  const top = cappedCone(x, y - 10.2 * s, z, 0.9 * s, 0.7 * s, 0.05);
  const d = min(tier, top) + (noise3(x * 2.5, y * 2.5, z * 2.5) - 0.5) * 0.35;
  return min(trunk, d * 0.8);
}

export function color(x, y, z, k) {
  const s = 0.8 + hash(k.seed) * 0.25;
  if (length2(x, z) < 0.32 * s) return rgb(80, 62, 50);
  const n = noise3(x * 3.0, y * 3.0, z * 3.0);
  return hsv(0.4 + hash(k.seed, 3) * 0.04, 0.48, 0.2 + n * 0.14 + y * 0.008);
}
