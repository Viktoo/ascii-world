// A wooden ladder fixed to the granite face of the landing, its rungs green
// with weed below the tide line, its rails rising over the edge for a hand
// to hold: the way up out of the water. It faces +z, out to sea.
export const meta = {
  name: "quay ladder",
  bounds: [0.32, 4.0, 0.12],
  tags: ["ladder", "prop"],
  props: { mass: 40, burns: 0.5 },
  anchors: [
    { kind: "stairs", at: [0, 0, 0.5], to: [0, 3.0, -0.5], face: 180 },
  ],
};

export function sdf(x, y, z, k) {
  const rails = box(abs(x) - 0.25, y - 1.95, z, 0.04, 1.95, 0.04);
  const ry = y - 0.3 * floor(y / 0.3) - 0.15;
  const rungs = max(max(box(x, ry, z, 0.25, 0.022, 0.025), y - 3.0), 0.05 - y);
  return min(rails, rungs);
}

export function color(x, y, z, k) {
  if (y < 1.3) return mix(rgb(54, 76, 44), rgb(80, 96, 56), noise3(x * 8, y * 8, z * 8));
  return mix(rgb(120, 100, 76), rgb(146, 124, 96), noise3(x * 8, y * 20, z * 8));
}
