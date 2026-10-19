// A short length of stone balustrade (1.0 m), to close a gap: the same
// pale limestone, a plinth, round-bellied balusters and a broad top rail to
// lean on and look out over the roofs to the sea.
export const meta = {
  name: "short balustrade",
  bounds: [0.52, 1.0, 0.24],
  tags: ["structure", "wall"],
  props: { mass: 4000 },
  sound: { hard: 1, dry: 0.4, ring: 0.1 },
};

export function sdf(x, y, z, k) {
  const plinth = box(x, y - 0.08, z, 0.5, 0.08, 0.2);
  const rail = roundBox(x, y - 0.9, z, 0.5, 0.07, 0.2, 0.03);
  const bx = x - 0.32 * floor(x / 0.32 + 0.5);
  const bal = max(cappedCone(bx, y - 0.5, z, 0.33, 0.05, 0.09) * 0.9, abs(x) - 0.45);
  const posts = box(abs(x) - 0.42, y - 0.5, z, 0.08, 0.4, 0.16);
  return min(min(plinth, rail), min(bal, posts));
}

export function color(x, y, z, k) {
  return mix(rgb(222, 212, 192), rgb(240, 234, 220), noise3(x * 3, y * 3, z * 3));
}
