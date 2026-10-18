// A stone bench along the quay or in the piazza: a long slab of pale
// limestone on two blocks, worn smooth by sitting.
export const meta = {
  name: "stone bench",
  bounds: [1.1, 0.5, 0.3],
  tags: ["bench", "furniture", "prop"],
  props: { mass: 900 },
  sound: { hard: 1, dry: 0.4, ring: 0.1 },
  anchors: [
    { kind: "seat", at: [-0.5, 0.46, 0.05], face: 0 },
    { kind: "seat", at: [0.5, 0.46, 0.05], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  return min(roundBox(x, y - 0.4, z, 1.05, 0.06, 0.26, 0.03), box(abs(x) - 0.75, y - 0.18, z, 0.14, 0.18, 0.2));
}

export function color(x, y, z, k) {
  return mix(rgb(214, 204, 182), rgb(232, 224, 206), noise3(x * 3, y * 3, z * 3));
}
