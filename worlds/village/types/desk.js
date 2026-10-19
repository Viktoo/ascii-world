// A writing desk with a drawer, a chair pushed under, a green-shaded lamp, an
// open ledger and a pen. Someone sits at its front (+z).
export const meta = {
  name: "desk",
  bounds: [0.7, 1.2, 0.72],
  tags: ["furniture", "table"],
  props: { mass: 40, burns: 0.6, light: 1 },
  anchors: [
    { kind: "seat", at: [0, 0.47, -0.5], face: 0 },
    { kind: "light", at: [-0.45, 1.0, 0.1] },
  ],
};

export function sdf(x, y, z, k) {
  const top = box(x, y - 0.74, z, 0.65, 0.03, 0.32);
  const sides = box(abs(x) - 0.6, y - 0.36, z, 0.04, 0.36, 0.3);
  const lamp = min(cylinder(x + 0.45, y - 0.9, z + 0.1, 0.02, 0.15), cappedCone(x + 0.45, y - 1.08, z + 0.1, 0.06, 0.12, 0.04));
  const book = box(x, y - 0.78, z - 0.05, 0.2, 0.012, 0.14);
  const seat = box(x, y - 0.45, z - 0.5, 0.2, 0.025, 0.2);
  const back = box(x, y - 0.7, z - 0.68, 0.2, 0.22, 0.02);
  return min(min(top, sides), min(min(lamp, book), min(seat, back)));
}

export function color(x, y, z, k) {
  if (y > 1.0 && abs(x + 0.45) < 0.13) return glow(rgb(60, 140, 80), 0.4);
  if (y > 0.77 && abs(x) < 0.2) return rgb(244, 238, 220);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
