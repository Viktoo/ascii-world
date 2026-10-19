// A sugar maple in its autumn colour: a straight grey-brown trunk under a
// tall, rounded crown of leaves gone scarlet, flame orange or gold, a whole
// stand turning alike with a few strays, some still edged in green.
export const meta = {
  name: "sugar maple",
  bounds: [4.0, 10.5, 4.0],
  tags: ["tree"],
  props: { mass: 3500, burns: 0.5, alive: 1, fuel: 1 },
};

export function sdf(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.25;
  const trunk = cappedCone(x, y - 2.5 * s, z, 2.5 * s, 0.42 * s, 0.24 * s);
  // Far from the crown, its bounding sphere is enough.
  const far = length3(x, y - 6.6 * s, z) - 4.6 * s - 0.3;
  if (far > 0.4) return min(trunk, far * 0.82);
  const c1 = ellipsoid(x, y - 6.6 * s, z, 3.5 * s, 3.8 * s, 3.4 * s);
  const c2 = sphere(x - 1.6 * s, y - 5.4 * s, z + 0.9 * s, 2.1 * s);
  const c3 = sphere(x + 1.3 * s, y - 8.0 * s, z - 0.8 * s, 1.9 * s);
  const crown = smoothUnion(c1, min(c2, c3), 0.8);
  const bumpy = crown + (noise3(x * 1.4, y * 1.4, z * 1.4) - 0.5) * 0.65;
  return min(trunk, bumpy * 0.82);
}

export function color(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.25;
  if (y < 3.6 * s && length2(x, z) < 0.6 * s) return mix(rgb(84, 76, 70), rgb(116, 106, 96), noise3(x * 6, y * 2, z * 6));
  let f = k.a;
  if (hash(k.seed, 7) < 0.25) f = hash(k.seed, 8);
  // Scarlet, flame orange, or gold.
  let hue = 0.005;
  if (f > 0.4) hue = 0.045;
  if (f > 0.75) hue = 0.1;
  const n = noise3(x * 2.6, y * 2.6, z * 2.6);
  return hsv(hue + (n - 0.5) * 0.03, 0.85, 0.58 + n * 0.32 + (y - 6.6 * s) * 0.03);
}
