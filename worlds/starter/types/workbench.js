// A heavy workbench with a vice, planes, chisels and saws hung on a board
// above it, wood shavings on the floor.
export const meta = {
  name: "workbench",
  bounds: [1.0, 1.7, 0.42],
  tags: ["furniture", "prop"],
  props: { mass: 80, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const top = box(x, y - 0.85, z, 0.95, 0.05, 0.38);
  const legs = box(abs(x) - 0.85, y - 0.4, abs(z) - 0.3, 0.05, 0.4, 0.05);
  const vice = box(x + 0.85, y - 0.95, z + 0.36, 0.08, 0.06, 0.06);
  const board = box(x, y - 1.35, z - 0.4, 0.9, 0.35, 0.02);
  const tools = max(box(x - 0.25 * floor(x / 0.25 + 0.5), y - 1.35, z - 0.37, 0.03, 0.25, 0.01), abs(x) - 0.8);
  return min(min(top, legs), min(vice, min(board, tools)));
}

export function color(x, y, z, k) {
  if (z < -0.36 && y > 1.0) return abs(x - 0.25 * floor(x / 0.25 + 0.5)) < 0.035 ? rgb(150, 150, 156) : rgb(196, 170, 120);
  if (abs(x - 0.85) < 0.1 && y > 0.88) return rgb(60, 60, 64);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
