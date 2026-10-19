// A painted wooden rocking chair on a porch, a folded quilt over its arm.
export const meta = {
  name: "rocking chair",
  bounds: [0.35, 1.1, 0.45],
  tags: ["furniture", "seat"],
  props: { mass: 12, burns: 0.6 },
  anchors: [
    { kind: "seat", at: [0, 0.47, -0.05], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const seat = box(x, y - 0.45, z, 0.25, 0.03, 0.22);
  const back = box(x, y - 0.8, z + 0.24 - (y - 0.45) * 0.2, 0.25, 0.35, 0.02);
  const rock = max(abs(length2(z, y - 1.4) - 1.38) - 0.02, max(abs(abs(x) - 0.23) - 0.02, abs(z) - 0.4));
  const legs = box(abs(x) - 0.23, y - 0.25, abs(z) - 0.18, 0.02, 0.22, 0.02);
  return min(min(seat, back), min(rock, legs));
}

export function color(x, y, z, k) {
  if (y > 0.48 && y < 0.55 && x > 0.15) return rgb(170, 50, 40);
  return rgb(40, 70, 56);
}
