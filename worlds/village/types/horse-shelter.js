// A horse shelter: a roof of chestnut shingles on six stout posts, open on
// three sides, a hay rack and a stone water trough along its back wall of
// planks.
export const meta = {
  name: "horse shelter",
  bounds: [4.2, 3.4, 2.7],
  tags: ["building", "stable"],
  props: { mass: 20000, burns: 0.6 },
  sound: { hard: 0.5, dry: 0.8, ring: 0.05 },
};

export function sdf(x, y, z, k) {
  const posts = box(abs(x - 4.0 * floor(x / 4.0 + 0.5)) - 0.0, y - 1.4, abs(z) - 2.2, 0.12, 1.4, 0.12);
  const p2 = max(posts, abs(x) - 4.1);
  const back = box(x, y - 1.2, z + 2.3, 4.0, 1.2, 0.08);
  const roof = box(x, y - 3.0 + z * 0.22, z, 4.2, 0.08, 2.65);
  const rack = box(x, y - 1.3, z + 2.0, 2.4, 0.4, 0.25);
  const trough = max(box(x - 2.5, y - 0.35, z + 1.8, 1.0, 0.35, 0.35), -box(x - 2.5, y - 0.5, z + 1.8, 0.9, 0.3, 0.27));
  return min(min(p2, back), min(roof, min(rack, trough)));
}

export function color(x, y, z, k) {
  if (y > 2.4) return mix(rgb(90, 72, 54), rgb(84, 110, 56), smoothstep(0.5, 0.8, noise3(x * 2, y * 2, z * 2)));
  if (abs(z + 2.0) < 0.3 && abs(y - 1.3) < 0.45 && abs(x) < 2.45) return rgb(214, 190, 100);
  if (abs(x - 2.5) < 1.05 && y < 0.75 && z > -2.2) return y > 0.6 ? rgb(90, 140, 170) : rgb(150, 146, 136);
  return mix(rgb(110, 80, 54), rgb(138, 102, 68), noise3(x * 5, y * 5, z * 5));
}
