// A long trestle table for the harvest supper on the green, laid with a
// white cloth, apple pies, jugs of
// cider and loaves of bread, benches along both sides. Long along x.
export const meta = {
  name: "harvest table",
  bounds: [2.6, 1.0, 0.95],
  tags: ["furniture", "table"],
  props: { mass: 120, burns: 0.6 },
  anchors: [
    { kind: "seat", at: [-1.6, 0.46, 0.75], face: 180 },
    { kind: "seat", at: [0, 0.46, 0.75], face: 180 },
    { kind: "seat", at: [1.6, 0.46, 0.75], face: 180 },
    { kind: "seat", at: [-1.6, 0.46, -0.75], face: 0 },
    { kind: "seat", at: [0, 0.46, -0.75], face: 0 },
    { kind: "seat", at: [1.6, 0.46, -0.75], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const top = box(x, y - 0.76, z, 2.5, 0.03, 0.45);
  const legs = box(abs(x) - 2.2, y - 0.38, z, 0.06, 0.38, 0.38);
  const benches = box(x, y - 0.44, abs(z) - 0.75, 2.4, 0.03, 0.15);
  const blegs = box(abs(x) - 2.1, y - 0.22, abs(z) - 0.75, 0.05, 0.22, 0.12);
  const px = x - 1.1 * floor(x / 1.1 + 0.5);
  const pies = max(cylinder(px - 0.45, y - 0.8, z - 0.24, 0.13, 0.02), abs(x) - 2.3);
  const jugs = max(cappedCone(px + 0.45, y - 0.88, z + 0.22, 0.09, 0.06, 0.04), abs(x) - 2.3);
  return min(min(min(top, legs), min(benches, blegs)), min(pies, jugs));
}

export function color(x, y, z, k) {
  if (y > 0.79 && z > 0.1) return rgb(200, 140, 70);
  if (y > 0.79 && z < -0.1) return rgb(196, 150, 90);
  if (abs(y - 0.76) < 0.035 && abs(z) < 0.46) return rgb(246, 244, 236);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
