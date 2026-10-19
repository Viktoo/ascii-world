// A wooden painter's palette with a thumb hole, dabbed with blobs of oil paint:
// crimson, cadmium orange, gold, cobalt, white, and a muddle where they mixed.
export const meta = {
  name: "paint palette",
  bounds: [0.13, 0.03, 0.1],
  tags: ["item", "art", "tool"],
  props: { mass: 0.3, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const board = max(max(cylinder(x, y - 0.006, z, 0.12, 0.006), -cylinder(x + 0.07, y - 0.006, z - 0.03, 0.018, 0.01)), -cylinder(x + 0.12, y - 0.006, z + 0.06, 0.05, 0.01));
  const ax = atan2(z, x);
  const blobs = max(sphere(x - cos(floor(ax * 1.6 + 0.5) / 1.6) * 0.085, y - 0.012, z - sin(floor(ax * 1.6 + 0.5) / 1.6) * 0.085, 0.017), y - 0.025);
  return min(board, blobs);
}

export function color(x, y, z, k) {
  if (y > 0.0135) return hsv(fract(floor(atan2(z, x) * 1.6 + 0.5) * 0.17), 0.75, 0.9);
  if (length2(x - 0.02, z) < 0.04) return mix(rgb(150, 100, 90), rgb(120, 110, 100), noise3(x * 60, 1.0, z * 60));
  return mix(rgb(176, 130, 84), rgb(200, 156, 104), noise3(x * 30, 1.0, z * 8));
}
