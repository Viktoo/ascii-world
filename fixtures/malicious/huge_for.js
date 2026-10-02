export const meta = { name: "evil", bounds: [1, 1, 1], tags: [] };
export function sdf(x, y, z, k) {
  let d = 0;
  for (let i = 0; i < 1000000; i++) { d = d + 1; }
  return sphere(x, y, z, 0.5);
}
export function color(x, y, z, k) { return rgb(255, 0, 0); }
