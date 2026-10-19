// A fishing boat half built on trestles in Matteo's workshop: ribs and fresh
// planks of chestnut, half of it painted blue, a pot of paint and brushes
// beside it.
export const meta = {
  name: "boat on trestles",
  bounds: [0.95, 1.5, 2.6],
  tags: ["furniture", "prop"],
  props: { mass: 400, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const tr = box(abs(x) - 0.0, y - 0.35, abs(z) - 1.6, 0.8, 0.35, 0.08);
  const t2 = clamp(z / 2.5, -1, 1);
  const w = 0.85 * sqrt(max(1 - t2 * t2, 0.02));
  const hull = max(max(length2(x / max(w, 0.05), (y - 1.15) / 0.5) - 1, y - 1.25), abs(z) - 2.5) * min(w, 0.4);
  const inner = max(length2(x / max(w - 0.06, 0.03), (y - 1.2) / 0.44) - 1, abs(z) - 2.4) * 0.4;
  return min(tr, max(hull, -inner));
}

export function color(x, y, z, k) {
  if (y < 0.72) return mix(rgb(84, 56, 38), rgb(104, 72, 48), noise3(x * 6, y * 18, z * 6));
  if (z > 0.0 && y > 0.75) return rgb(40, 100, 180);
  return fract(y * 8.0) < 0.12 ? rgb(110, 76, 48) : rgb(196, 150, 100);
}
