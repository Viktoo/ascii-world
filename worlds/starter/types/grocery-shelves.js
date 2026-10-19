// The general store's shelves: jars of maple syrup and jam, sacks of flour and
// oats, tins of coffee, candles, lamp oil, rope and twine.
export const meta = {
  name: "grocery shelves",
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
    if (h < 0.25) return rgb(200, 40, 30);
    if (h < 0.5) return rgb(236, 200, 110);
    if (h < 0.75) return rgb(110, 130, 40);
    return rgb(40, 60, 120);
  }
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
