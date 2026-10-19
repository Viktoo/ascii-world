// An old apple tree of the orchards: a short crooked trunk and a low,
// rounded crown of yellowing leaves hung with red apples, a few fallen in
// the grass beneath.
export const meta = {
  name: "apple tree",
  bounds: [3.0, 4.6, 3.0],
  tags: ["apple"],
  props: { mass: 900, burns: 0.5, alive: 1, fuel: 0.9 },
};

export function sdf(x, y, z, k) {
  const s = 0.8 + hash(k.seed) * 0.3;
  const trunk = capsule(x, y, z, 0, 0, 0, 0.3 * s, 1.6 * s, -0.2 * s, 0.2 * s);
  const crown = ellipsoid(x, y - 2.8 * s, z, 2.5 * s, 1.5 * s, 2.4 * s) + (noise3(x * 1.8, y * 1.8, z * 1.8) - 0.5) * 0.5;
  // Windfalls in the grass.
  const gx = x - 1.3 * floor(x / 1.3 + 0.5);
  const gz = z - 1.1 * floor(z / 1.1 + 0.5);
  const fallen = max(sphere(gx, y - 0.05, gz, 0.07), length2(x, z) - 2.2 * s);
  return min(min(trunk, crown * 0.82), fallen);
}

export function color(x, y, z, k) {
  const s = 0.8 + hash(k.seed) * 0.3;
  if (y < 0.15) return rgb(186, 34, 30);
  if (y < 1.8 * s && length2(x, z) < 0.6 * s) return mix(rgb(70, 58, 46), rgb(98, 82, 64), noise3(x * 6, y * 6, z * 6));
  // Apples dotted thick over the outside of the crown.
  const ax = floor(x * 2.4);
  const ay = floor(y * 2.4);
  const az = floor(z * 2.4);
  const fx = fract(x * 2.4) - 0.5;
  const fy = fract(y * 2.4) - 0.5;
  const fz = fract(z * 2.4) - 0.5;
  if (hash(ax, ay * 7 + az, k.seed) < 0.3 && fx * fx + fy * fy + fz * fz < 0.09) return mix(rgb(176, 24, 28), rgb(220, 60, 40), hash(az, ax));
  const n = noise3(x * 3, y * 3, z * 3);
  return hsv(0.17 + (n - 0.5) * 0.05, 0.6, 0.42 + n * 0.22);
}
