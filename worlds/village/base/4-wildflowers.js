// A patch of Mediterranean wildflowers: a low cushion of leaves thick with
// blooms, red poppies, blue cornflowers, white daisies, yellow chamomile or
// purple vetch, a whole field in one colour with strays.
export const meta = {
  name: "wildflowers",
  bounds: [1.15, 0.35, 1.15],
  tags: ["flower", "nonsolid"],
  props: { mass: 0.2, burns: 0.8, alive: 1, fuel: 0.2 },
};

export function sdf(x, y, z, k) {
  // A low, lumpy cushion, a little taller in its middle.
  const r = length2(x, z);
  const top = 0.24 - r * 0.1 + (noise3(x * 4, 0, z * 4 + k.seed) - 0.5) * 0.12;
  return max(r - 0.95 - (noise3(x * 2, 1, z * 2 + k.seed) - 0.5) * 0.3, max(y - top, -y - 0.05)) * 0.8;
}

export function color(x, y, z, k) {
  let f = k.a;
  if (hash(k.seed, 9) < 0.15) f = hash(k.seed, 10);
  let c = rgb(226, 34, 36);
  if (f > 0.35) c = rgb(70, 110, 236);
  if (f > 0.55) c = rgb(250, 250, 244);
  if (f > 0.75) c = rgb(252, 214, 46);
  if (f > 0.9) c = rgb(196, 90, 210);
  // The cushion: blooms dotted thick over the leaves.
  const gx = floor(x * 7.0);
  const gz = floor(z * 7.0);
  const fx = fract(x * 7.0) - 0.5;
  const fz = fract(z * 7.0) - 0.5;
  if (y > 0.04 && hash(gx, gz, k.seed) < 0.66 && fx * fx + fz * fz < 0.16) return c * (0.88 + 0.12 * hash(gz, gx));
  return mix(rgb(52, 100, 38), rgb(96, 142, 56), clamp(y / 0.25, 0, 1));
}
