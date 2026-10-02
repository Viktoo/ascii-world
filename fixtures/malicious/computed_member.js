export const meta = { name: "evil", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) {
  return sphere(x, y, z, k["constructor"]);
}
export function color(x, y, z, k) { return rgb(255, 0, 0); }
