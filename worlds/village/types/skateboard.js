// A skateboard painted with a flame, red wheels. Ridden standing on it,
// pushing with one foot; its front is +z.
export const meta = {
  name: "skateboard",
  bounds: [0.12, 0.08, 0.42],
  tags: ["vehicle", "toy"],
  props: { mass: 2.5, toy: 1 },
  anchors: [
    { kind: "seat", at: [0, 0.09, 0], face: 0 },
  ],
  drive: { speed: 4, on: "land", by: "legs" },
};

export function sdf(x, y, z, k) {
  const deck = roundBox(x, y - 0.07, z, 0.1, 0.012, 0.4, 0.05);
  const wheels = cylinder(abs(z) - 0.28 + 0.0 * x, abs(x) - 0.08, y - 0.03, 0.025, 0.02);
  return min(deck, wheels);
}

export function color(x, y, z, k) {
  if (y < 0.055) return rgb(220, 40, 40);
  return fract(z * 3.0) < 0.5 ? rgb(250, 160, 40) : rgb(30, 30, 30);
}
