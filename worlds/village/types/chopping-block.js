// A chopping block: a broad chestnut stump with an axe bitten into its
// top, split kindling scattered round it.
export const meta = {
  name: "chopping block",
  bounds: [0.7, 0.95, 0.7],
  tags: ["prop"],
  props: { mass: 80, burns: 0.6 },
  sound: { hard: 0.6, dry: 0.8, ring: 0.05 },
};

export function sdf(x, y, z, k) {
  const stump = cappedCone(x, y - 0.3, z, 0.3, 0.38, 0.33);
  const handle = capsule(x, y, z, 0.05, 0.62, 0, 0.35, 0.9, 0.2, 0.025);
  const head = box(x - 0.04, y - 0.66, z + 0.02, 0.05, 0.08, 0.13);
  const chips = box(abs(x) - 0.5, y - 0.04, abs(z) - 0.3, 0.12, 0.04, 0.05);
  return min(min(stump, handle), min(head, chips));
}

export function color(x, y, z, k) {
  if (y > 0.6 && y < 0.75 && abs(x - 0.04) < 0.08) return rgb(150, 156, 160);
  if (y > 0.58) return rgb(150, 110, 70);
  if (y > 0.57 - 0.0 && length2(x, z) < 0.33) return rgb(210, 178, 128);
  return mix(rgb(92, 70, 50), rgb(116, 88, 60), noise3(x * 8, y * 8, z * 8));
}
