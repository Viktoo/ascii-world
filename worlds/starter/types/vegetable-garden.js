// A vegetable garden: a raised bed edged with planks, rows of beans on
// cane wigwams, cabbages, lettuces and staked tomatoes hung with red fruit.
export const meta = {
  name: "vegetable garden",
  bounds: [2.6, 1.5, 1.6],
  tags: ["prop", "garden"],
  props: { mass: 2000, alive: 1 },
};

export function sdf(x, y, z, k) {
  const bed = box(x, y - 0.15, z, 2.5, 0.15, 1.5);
  const row = z - 0.75 * floor(z / 0.75 + 0.5);
  const cx = x - 0.5 * floor(x / 0.5 + 0.5);
  const plants = max(max(sphere(cx, y - 0.4, row, 0.22) + (noise3(x * 6, y * 6, z * 6) - 0.5) * 0.1, abs(x) - 2.3), abs(z) - 1.3);
  const canes = max(cappedCone(x + 1.6, y - 0.8, z, 0.7, 0.35, 0.02), -cappedCone(x + 1.6, y - 0.8, z, 0.7, 0.31, -0.02));
  return min(min(bed, plants), canes);
}

export function color(x, y, z, k) {
  if (y < 0.29 && (abs(x) > 2.4 || abs(z) > 1.4)) return rgb(120, 90, 60);
  if (y < 0.31) return rgb(84, 60, 40);
  const n = noise3(x * 10, y * 10, z * 10);
  if (x > 0.5 && n > 0.7) return rgb(220, 40, 30);
  if (x < -0.9 && y > 0.6) return rgb(150, 120, 70);
  return mix(rgb(60, 130, 50), rgb(130, 180, 70), n);
}
