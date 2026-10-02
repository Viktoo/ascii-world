export const meta = { name: "tide rock", bounds: [1.5, 1.1, 1.5], tags: ["rock"] };
export function sdf(x, y, z, k) {
  return ellipsoid(x, y - 0.2, z, 1.3, 0.7, 1.1) + (noise3(x * 2, y * 2, z * 2) - 0.5) * 0.2;
}
export function color(x, y, z, k) { return mix(rgb(90, 95, 100), rgb(140, 140, 135), noise3(x * 4, y * 4, z * 4)); }