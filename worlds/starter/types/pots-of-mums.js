// Clay pots of autumn mums by a doorway or on porch steps, two or three
// together: domes thick with small flowers in rust, gold and wine red over
// dark green leaves.
export const meta = {
  name: "pots of mums",
  bounds: [0.62, 0.66, 0.36],
  tags: ["flower", "prop"],
  props: { mass: 12, alive: 1, fragile: 0.5 },
};

export function sdf(x, y, z, k) {
  const px = x - 0.42 * floor(x / 0.42 + 0.5);
  const pots = max(max(cappedCone(px, y - 0.15, z, 0.15, 0.1, 0.15), -cappedCone(px, y - 0.2, z, 0.12, 0.09, 0.13)), abs(x) - 0.58);
  const rim = max(max(abs(length2(px, z) - 0.15) - 0.02, abs(y - 0.29) - 0.025), abs(x) - 0.6);
  // The dome of blooms: small bumps, one flower to each cell.
  const fx = px - 0.05 * floor(px / 0.05 + 0.5);
  const fz = z - 0.05 * floor(z / 0.05 + 0.5);
  const dome = ellipsoid(px, y - 0.36, z, 0.2, 0.16, 0.2);
  const blooms = max(dome - 0.012 + length2(fx, fz) * 0.35, -dome - 0.04);
  return min(min(pots, rim), max(min(dome, blooms), abs(x) - 0.6));
}

export function color(x, y, z, k) {
  if (y < 0.31) return abs(y - 0.29) < 0.03 ? rgb(170, 84, 50) : mix(rgb(166, 82, 50), rgb(196, 104, 62), noise3(x * 8, y * 8, z * 8));
  const px = x - 0.42 * floor(x / 0.42 + 0.5);
  const fx = px - 0.05 * floor(px / 0.05 + 0.5);
  const fz = z - 0.05 * floor(z / 0.05 + 0.5);
  const pot = floor(x / 0.42 + 0.5 + k.seed);
  let c = rgb(200, 90, 30);
  if (fract(pot / 3.0) > 0.3) c = rgb(236, 176, 40);
  if (fract(pot / 3.0) > 0.6) c = rgb(150, 30, 50);
  if (y < 0.4 && length2(px, z) > 0.17) return rgb(46, 80, 40);
  const r = length2(fx, fz);
  if (r < 0.006) return c * 0.6;
  return c * (0.75 + 0.25 * step(0.5, fract(atan2(fz, fx) * 1.9))) * (0.85 + 0.15 * (1.0 - r / 0.035));
}
