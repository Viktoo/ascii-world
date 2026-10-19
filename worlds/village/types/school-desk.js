// A double school desk with an inkwell and a slanted lid, a bench for two
// children. They sit at its back (-z), facing +z.
export const meta = {
  name: "school desk",
  bounds: [0.6, 0.75, 0.6],
  tags: ["furniture", "table"],
  props: { mass: 25, burns: 0.6 },
  anchors: [
    { kind: "seat", at: [-0.25, 0.42, -0.45], face: 0 },
    { kind: "seat", at: [0.25, 0.42, -0.45], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const lid = box(x, y - 0.68 - z * 0.15, z, 0.55, 0.02, 0.2);
  const sides = box(abs(x) - 0.52, y - 0.35, z, 0.03, 0.35, 0.22);
  const bench = box(x, y - 0.4, z + 0.45, 0.55, 0.025, 0.12);
  return min(min(lid, sides), bench);
}

export function color(x, y, z, k) {
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
