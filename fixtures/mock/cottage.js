export const meta = { name: "keeper's cottage", bounds: [3.2, 3.6, 2.7], tags: ["building"] };
export function sdf(x, y, z, k) {
  const walls = box(x, y - 1.2, z, 2.6, 1.2, 2.1);
  const q = rotZ(x, y - 2.4, z, 0.0);
  const roof = intersect(box(x, y - 2.9, z, 3.0, 0.7, 2.5), plane(abs(x), y - 2.4, z, 0.6, 1.0, 0, -1.8));
  const door = box(x, y - 0.8, z - 2.1, 0.45, 0.8, 0.2);
  return subtract(union(walls, roof), door);
}
export function color(x, y, z, k) {
  if (y > 2.35) return rgb(150, 60, 45);
  if (abs(x) < 0.5 && y < 1.6 && z > 2.0) return rgb(70, 50, 35);
  return mix(rgb(225, 215, 190), rgb(200, 190, 165), noise3(x * 2, y * 2, z * 2));
}