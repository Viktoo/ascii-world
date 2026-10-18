// An Italian stone pine: a tall bare trunk leaning a little, and a broad,
// flat umbrella of dark green needles high on top.
export const meta = {
  name: "stone pine",
  bounds: [5.6, 14.0, 5.6],
  tags: ["pine"],
  props: { mass: 3000, burns: 0.6, alive: 1, fuel: 1 },
};

export function sdf(x, y, z, k) {
  const h = 9.5 + hash(k.seed) * 3.5;
  const lx = (hash(k.seed, 2) - 0.5) * 2.2;
  const lz = (hash(k.seed, 3) - 0.5) * 2.2;
  const trunk = cone(x, y, z, 0, 0, 0, lx, h, lz, 0.38, 0.22);
  const crown = ellipsoid(x - lx, y - h - 0.6, z - lz, 4.8, 1.25, 4.6);
  const bumps = crown + (noise3(x * 0.9, y * 2.0, z * 0.9) - 0.5) * 1.0;
  if (bumps > 1.5) return min(trunk, bumps);
  return min(trunk, bumps * 0.7);
}

export function color(x, y, z, k) {
  const h = 9.5 + hash(k.seed) * 3.5;
  const n = noise3(x * 2, y * 2, z * 2);
  if (y < h - 0.4) return mix(rgb(110, 76, 52), rgb(150, 104, 70), n);
  return mix(rgb(42, 74, 40), rgb(70, 104, 54), n);
}
