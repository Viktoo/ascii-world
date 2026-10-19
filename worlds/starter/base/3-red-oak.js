// A red oak: a thick dark trunk and a broad, spreading, lumpy crown of
// leaves gone russet, burgundy and copper, holding on late into autumn.
export const meta = {
  name: "red oak",
  bounds: [5.0, 9.5, 5.0],
  tags: ["oak"],
  props: { mass: 5000, burns: 0.45, alive: 1, fuel: 1 },
};

export function sdf(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.25;
  const trunk = cappedCone(x, y - 2.2 * s, z, 2.2 * s, 0.6 * s, 0.38 * s);
  const far = length3(x, y - 5.8 * s, z) - 5.2 * s - 0.3;
  if (far > 0.4) return min(trunk, far * 0.82);
  const c1 = ellipsoid(x, y - 6.0 * s, z, 4.6 * s, 2.8 * s, 4.3 * s);
  const c2 = sphere(x - 2.4 * s, y - 5.0 * s, z - 1.0 * s, 2.3 * s);
  const c3 = sphere(x + 2.2 * s, y - 5.4 * s, z + 1.4 * s, 2.2 * s);
  const c4 = sphere(x + 0.4 * s, y - 7.6 * s, z - 0.4 * s, 2.2 * s);
  const crown = smoothUnion(c1, min(min(c2, c3), c4), 0.9);
  const bumpy = crown + (noise3(x * 1.1, y * 1.1, z * 1.1) - 0.5) * 0.8;
  return min(trunk, bumpy * 0.82);
}

export function color(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.25;
  if (y < 3.4 * s && length2(x, z) < 0.9 * s) return mix(rgb(56, 48, 42), rgb(84, 72, 60), noise3(x * 5, y * 5, z * 5));
  const n = noise3(x * 2.2, y * 2.2, z * 2.2);
  const hue = 0.025 + hash(k.seed, 3) * 0.04 + (n - 0.5) * 0.03;
  return hsv(hue, 0.78, 0.46 + n * 0.28);
}
