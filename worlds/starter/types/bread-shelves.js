// The baker's shelves behind the counter, stacked with round loaves, brown
// bread, apple pies and cinnamon buns.
export const meta = {
  name: "bread shelves",
  bounds: [1.0, 2.15, 0.25],
  tags: ["furniture", "shop"],
  props: { mass: 80, burns: 0.5 },
};

export function sdf(x, y, z, k) {
  const frame = max(box(x, y - 1.05, z, 0.95, 1.05, 0.22), -box(x, y - 1.12, z - 0.04, 0.91, 0.98, 0.22));
  const sy = y - 0.5 * floor(y / 0.5) - 0.25;
  const boards = max(box(x, sy + 0.23, z, 0.92, 0.02, 0.2), max(y - 2.05, 0.1 - y));
  const gx = x - 0.2 * floor(x / 0.2 + 0.5);
  const goods = max(max(box(gx, sy + 0.08, z, 0.08, 0.13, 0.15), abs(x) - 0.88), max(y - 2.0, 0.2 - y));
  return min(frame, min(boards, goods));
}

export function color(x, y, z, k) {
  const sy = y - 0.5 * floor(y / 0.5);
  if (abs(z) < 0.19 && abs(x) < 0.9 && sy > 0.05 && y > 0.2 && y < 2.0) {
    const i = floor(x / 0.2 + 0.5) + floor(y / 0.5) * 7;
    const h = hash(i, k.seed);
    return mix(rgb(196, 130, 60), rgb(236, 190, 110), h) * (0.85 + 0.15 * noise3(x * 20, y * 20, z * 20));
  }
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
