// Bunk beds for the children of the house: two beds one over the other in a
// wooden frame, a ladder at the foot, blankets in bright colours.
export const meta = {
  name: "bunk beds",
  bounds: [0.53, 1.75, 1.08],
  tags: ["furniture", "bed"],
  props: { mass: 60, burns: 0.6 },
  anchors: [
    { kind: "bed", at: [0, 0.48, 0] },
    { kind: "bed", at: [0, 1.42, 0] },
  ],
};

export function sdf(x, y, z, k) {
  const posts = box(abs(x) - 0.47, y - 0.85, abs(z) - 0.97, 0.04, 0.85, 0.04);
  const low = roundBox(x, y - 0.38, z, 0.44, 0.09, 0.96, 0.04);
  const high = roundBox(x, y - 1.32, z, 0.44, 0.09, 0.96, 0.04);
  const rails = min(box(x, y - 0.27, z, 0.48, 0.04, 1.0), box(x, y - 1.2, z, 0.48, 0.04, 1.0));
  const guard = box(x - 0.46, y - 1.52, z + 0.25, 0.02, 0.1, 0.7);
  const rungs = max(box(x + 0.46, y - 0.3 * floor(y / 0.3) - 0.15, z - 0.95, 0.02, 0.02, 0.2), max(y - 1.3, -y));
  return min(min(posts, min(low, high)), min(rails, min(guard, rungs)));
}

export function color(x, y, z, k) {
  if (abs(abs(x) - 0.47) < 0.06 || abs(y - 0.27) < 0.05 || abs(y - 1.2) < 0.05 || abs(z - 0.95) < 0.05) return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
  return y > 1.0 ? rgb(60, 120, 200) : rgb(230, 80, 70);
}
