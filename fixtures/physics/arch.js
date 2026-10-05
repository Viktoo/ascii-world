export const meta = { name: "test arch", bounds: [1.6, 1.7, 0.35], tags: ["building"] };
export function sdf(x, y, z, k) {
  const legs = min(box(x - 1.3, y + 0.8, z, 0.25, 0.8, 0.3), box(x + 1.3, y + 0.8, z, 0.25, 0.8, 0.3));
  const lintel = box(x, y + 0.2, z, 1.6, 0.2, 0.3);
  return min(legs, lintel);
}
export function color(x, y, z, k) { return rgb(150, 140, 130); }
