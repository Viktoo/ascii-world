// A nasone: a cast-iron drinking fountain shaped like a little column, its
// curved spout always running with cool water into a grate.
export const meta = {
  name: "drinking fountain",
  bounds: [0.3, 1.0, 0.45],
  tags: ["prop", "fountain"],
  props: { mass: 200, wet: 1 },
  sound: { hard: 1, dry: 0.2, ring: 0.5 },
};

export function sdf(x, y, z, k) {
  const col = cappedCone(x, y - 0.45, z, 0.45, 0.17, 0.14);
  const top = sphere(x, y - 0.92, z, 0.12);
  const spout = cone(x, y, z, 0, 0.6, 0.12, 0, 0.55, 0.3, 0.03, 0.022);
  const water = cylinder(x, y - 0.27, z - 0.31, 0.012, 0.27);
  const grate = box(x, y - 0.01, z - 0.25, 0.22, 0.01, 0.18);
  return min(min(col, top), min(spout, min(water, grate)));
}

export function color(x, y, z, k) {
  if (abs(z - 0.31) < 0.03 && y < 0.54 && y > 0.03) return rgb(200, 230, 246);
  return rgb(44, 56, 50);
}
