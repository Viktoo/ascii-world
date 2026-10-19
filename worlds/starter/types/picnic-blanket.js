// A red-and-white checked picnic blanket spread flat on the ground, its edges
// sunk a little into the sand, for sitting on.
export const meta = {
  name: "picnic blanket",
  bounds: [1.0, 0.25, 0.8],
  tags: ["prop", "seat", "nonsolid"],
  props: { mass: 2, burns: 0.8 },
  anchors: [
    { kind: "seat", at: [0.3, 0.02, 0.2], face: 0 },
    { kind: "seat", at: [-0.3, 0.02, -0.2], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  // A cloth on the ground, its underside sunk so a slope doesn't lift it.
  return box(x, y + 0.1, z, 0.95, 0.112, 0.75);
}

export function color(x, y, z, k) {
  return step(0.5, fract(x * 5.0)) + step(0.5, fract(z * 5.0)) == 1 ? rgb(200, 40, 40) : rgb(246, 242, 236);
}
