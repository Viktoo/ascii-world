export const meta = { name: "test stairs", bounds: [1.05, 1.05, 1.55], tags: ["building"] };
export function sdf(x, y, z, k) {
  let d = box(x, y - 0.5, z - 0.75, 1.0, 0.5, 0.75);
  for (let i = 0; i < 5; i++) {
    d = min(d, box(x, y - 0.1 * (i + 1), z + 1.35 - 0.3 * i, 1.0, 0.1 * (i + 1), 0.15));
  }
  return d;
}
export function color(x, y, z, k) { return rgb(150, 150, 150); }
