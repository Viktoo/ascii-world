export const meta = { name: "test cellar house", bounds: [3, 3, 3], tags: ["building"], hollow: [2.5, 2.5, -2.4] };
export function sdf(x, y, z, k) {
  const walls = subtract(box(x, y, z, 2.8, 2.8, 2.8), box(x, y - 0.2, z, 2.5, 2.8, 2.5));
  return walls;
}
export function color(x, y, z, k) { return rgb(150, 140, 130); }
