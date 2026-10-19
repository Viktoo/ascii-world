// A folding wooden stool with a round green-painted seat, for sitting at the
// easel.
export const meta = {
  name: "painter's stool",
  bounds: [0.2, 0.5, 0.2],
  tags: ["furniture", "seat"],
  props: { mass: 3, burns: 0.8 },
  anchors: [
    { kind: "seat", at: [0, 0.48, 0], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.46, z, 0.18, 0.025), box(abs(x) - 0.13, y - 0.22, z, 0.02, 0.22, 0.12));
}

export function color(x, y, z, k) {
  if (y > 0.43) return rgb(60, 110, 90);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
