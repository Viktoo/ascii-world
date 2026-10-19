// A heavy tavern table of scrubbed pine, four chairs round it, laid with
// a pot of clam chowder, foaming mugs of beer and a basket of bread.
export const meta = {
  name: "tavern table",
  bounds: [0.95, 1.0, 0.95],
  tags: ["furniture", "table"],
  props: { mass: 40, burns: 0.6 },
  anchors: [
    { kind: "seat", at: [0.68, 0.47, 0], face: 270 },
    { kind: "seat", at: [-0.68, 0.47, 0], face: 90 },
    { kind: "seat", at: [0, 0.47, 0.68], face: 180 },
    { kind: "seat", at: [0, 0.47, -0.68], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const top = box(x, y - 0.74, z, 0.45, 0.025, 0.45);
  const leg = cylinder(x, y - 0.37, z, 0.05, 0.37);
  const pot = cylinder(x, y - 0.775, z, 0.25, 0.012);
  const carafe = cappedCone(x + 0.3, y - 0.86, z - 0.3, 0.1, 0.07, 0.03);
  const ax = abs(x) - 0.68;
  const az = abs(z) - 0.68;
  const seats = min(box(ax, y - 0.45, z, 0.18, 0.025, 0.18), box(x, y - 0.45, az, 0.18, 0.025, 0.18));
  const backs = min(box(abs(x) - 0.86, y - 0.7, z, 0.02, 0.24, 0.18), box(x, y - 0.7, abs(z) - 0.86, 0.18, 0.24, 0.02));
  return min(min(top, leg), min(min(pot, carafe), min(seats, backs)));
}

export function color(x, y, z, k) {
  if (y > 0.76 && length2(x, z) < 0.25) return noise3(x * 25, y * 25, z * 25) > 0.7 ? rgb(200, 180, 140) : rgb(236, 226, 196);
  if (y > 0.78 && length2(x - 0.3, z + 0.3) < 0.1) return y > 0.93 ? rgb(250, 246, 236) : rgb(214, 150, 40);
    return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
