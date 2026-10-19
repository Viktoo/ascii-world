// A wooden Adirondack chair, wide-armed and low, its slatted back leaning far
// back, painted barn red, sea green or white, for sitting on the sand and
// watching the sun go down. It faces +z.
export const meta = {
  name: "adirondack chair",
  bounds: [0.42, 1.0, 0.55],
  tags: ["furniture", "seat"],
  props: { mass: 15, burns: 0.6 },
  anchors: [
    { kind: "seat", at: [0, 0.38, -0.05], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const seat = max(box(x, y - 0.36 + z * 0.15, z, 0.3, 0.025, 0.28), 0.0 - 1.0);
  const sx = x - 0.12 * floor(x / 0.12 + 0.5);
  const back = max(max(box(sx, y - 0.62, z + 0.3 + (y - 0.35) * 0.5, 0.045, 0.32, 0.02), abs(x) - 0.3), (y - 0.95) + abs(x) * 0.4);
  const arms = box(abs(x) - 0.36, y - 0.55, z - 0.02, 0.06, 0.02, 0.32);
  const legs = box(abs(x) - 0.33, y - 0.27, abs(z) - 0.26, 0.025, 0.27, 0.025);
  return min(min(seat, back), min(arms, legs));
}

export function color(x, y, z, k) {
  const c = floor(hash(k.seed) * 3.0);
  const n = 0.92 + 0.08 * noise3(x * 8, y * 8, z * 8);
  if (c < 1) return rgb(160, 50, 40) * n;
  if (c < 2) return rgb(80, 150, 130) * n;
  return rgb(240, 238, 230) * n;
}
