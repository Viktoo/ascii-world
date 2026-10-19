// A soft three-seat sofa in faded velvet (green, rose, mustard or blue) with
// round arms and cushions, a crocheted cover over its back. Faces +z.
export const meta = {
  name: "sofa",
  bounds: [1.05, 0.85, 0.46],
  tags: ["furniture", "seat"],
  props: { mass: 60, burns: 0.8 },
  anchors: [
    { kind: "seat", at: [-0.6, 0.4, 0.05], face: 0 },
    { kind: "seat", at: [0, 0.4, 0.05], face: 0 },
    { kind: "seat", at: [0.6, 0.4, 0.05], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const seat = roundBox(x, y - 0.25, z + 0.02, 0.95, 0.15, 0.4, 0.08);
  const back = roundBox(x, y - 0.5, z + 0.34, 0.95, 0.28, 0.1, 0.08);
  const arms = roundBox(abs(x) - 0.92, y - 0.32, z + 0.02, 0.1, 0.18, 0.42, 0.08);
  return min(min(seat, back), arms);
}

export function color(x, y, z, k) {
  const c = floor(hash(k.seed) * 4);
  const cloth = c < 1 ? rgb(70, 120, 80) : c < 2 ? rgb(190, 110, 120) : c < 3 ? rgb(200, 160, 60) : rgb(60, 90, 150);
  if (y > 0.72 && abs(x) < 0.5) return rgb(244, 238, 222);
  return cloth * (0.9 + 0.1 * noise3(x * 10, y * 10, z * 10));
}
