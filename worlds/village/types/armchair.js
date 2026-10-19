// An old armchair with a lace cloth on its back and a cushion, a reading
// lamp's spot beside it. Faces +z.
export const meta = {
  name: "armchair",
  bounds: [0.46, 0.9, 0.46],
  tags: ["furniture", "seat"],
  props: { mass: 25, burns: 0.8 },
  anchors: [
    { kind: "seat", at: [0, 0.4, 0.02], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  return min(min(roundBox(x, y - 0.25, z, 0.38, 0.15, 0.38, 0.07), roundBox(x, y - 0.55, z + 0.32, 0.38, 0.3, 0.08, 0.07)), roundBox(abs(x) - 0.36, y - 0.35, z, 0.08, 0.15, 0.38, 0.06));
}

export function color(x, y, z, k) {
  if (y > 0.75 && abs(x) < 0.2) return rgb(246, 242, 232);
  return hsv(fract(hash(k.seed) * 0.3 + 0.02), 0.45, 0.6) * (0.9 + 0.1 * noise3(x * 9, y * 9, z * 9));
}
