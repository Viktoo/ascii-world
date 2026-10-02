export const meta = { name: "evil", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) {
  const m = import("fs");
  return sphere(x, y, z, 0.5);
}
export function color(x, y, z, k) { return rgb(255, 0, 0); }
