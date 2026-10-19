// A green-painted wooden bench of slats on iron legs, for sitting on the
// green or the landing and watching the sun go down.
export const meta = {
  name: "park bench",
  bounds: [0.9, 0.85, 0.35],
  tags: ["bench", "furniture", "prop"],
  props: { mass: 40, burns: 0.5 },
  anchors: [
    { kind: "seat", at: [-0.45, 0.46, -0.05], face: 0 },
    { kind: "seat", at: [0.45, 0.46, -0.05], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const sz = z - 0.1 * floor(z / 0.1 + 0.5);
  const seat = max(box(x, y - 0.45, z, 0.85, 0.025, 0.22), abs(sz) - 0.035);
  const back = box(x, y - 0.68 - 0.0, z + 0.25 - (y - 0.45) * 0.2, 0.85, 0.2, 0.02);
  const legs = box(abs(x) - 0.75, y - 0.23, z, 0.03, 0.23, 0.22);
  return min(min(seat, back), legs);
}

export function color(x, y, z, k) {
  if (y < 0.42) return rgb(40, 40, 40);
  return rgb(56, 100, 66);
}
