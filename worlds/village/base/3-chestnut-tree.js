// A sweet chestnut of the woods: a stout ridged trunk and a big, rounded,
// glossy green crown, a few gold leaves.
export const meta = {
  name: "chestnut tree",
  bounds: [4.5, 10.0, 4.5],
  tags: ["tree"],
  props: { mass: 4000, burns: 0.5, alive: 1, fuel: 1 },
};

export function sdf(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.2;
  const trunk = cappedCone(x, y - 2.4 * s, z, 2.4 * s, 0.6 * s, 0.35 * s);
  // Far from the crown, its bounding sphere is enough.
  const far = length3(x, y - 6.4 * s, z) - 5.4 * s - 0.3;
  if (far > 0.4) return min(trunk, far * 0.82);
  const c1 = ellipsoid(x, y - 6.6 * s, z, 4.2 * s, 3.4 * s, 4.0 * s);
  const c2 = sphere(x - 2.0 * s, y - 5.4 * s, z + 1.0 * s, 2.4 * s);
  const c3 = sphere(x + 1.7 * s, y - 7.8 * s, z - 1.2 * s, 2.3 * s);
  const crown = smoothUnion(c1, min(c2, c3), 0.8);
  const bumpy = crown + (noise3(x * 1.2, y * 1.2, z * 1.2) - 0.5) * 0.6;
  return min(trunk, bumpy * 0.82);
}

export function color(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.2;
  const n = noise3(x * 2.4, y * 2.4, z * 2.4);
  if (y < 4.0 * s && length2(x, z) < 0.9 * s) return mix(rgb(70, 56, 44), rgb(104, 86, 66), noise3(x * 6, y, z * 6));
  if (n > 0.86) return rgb(196, 160, 60);
  return hsv(0.26 + k.a * 0.05, 0.6, 0.24 + n * 0.2);
}
