export const meta = { name: "sea pine", bounds: [2.2, 7.5, 2.2], tags: ["tree"] };
export function sdf(x, y, z, k) {
  const trunk = cylinder(x, y - 1.6, z, 0.2, 1.6);
  const crown = cappedCone(x, y - 4.6, z, 2.8, 2.0, 0.1);
  return min(trunk, crown);
}
export function color(x, y, z, k) {
  if (y < 1.9) return rgb(90, 64, 44);
  return hsv(0.33 + hash(k.seed) * 0.04, 0.5, 0.3 + noise3(x * 3, y * 3, z * 3) * 0.15);
}