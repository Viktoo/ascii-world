// A market stall on the piazza: a wooden counter heaped with crates of
// lemons, tomatoes, oranges and greens, posts holding up a striped awning.
// The seller stands behind it (-z); buyers come to the front (+z).
export const meta = {
  name: "market stall",
  bounds: [1.7, 2.55, 1.15],
  tags: ["furniture", "prop", "shop"],
  props: { mass: 150, burns: 0.5 },
  sound: { hard: 0.5, dry: 0.8, ring: 0.05 },
};

export function sdf(x, y, z, k) {
  const counter = box(x, y - 0.45, z - 0.2, 1.5, 0.45, 0.5);
  const crates = box(x - 1.0 * floor(x / 1.0) - 0.5, y - 0.98, z - 0.25, 0.44, 0.1, 0.4);
  const heap = max(ellipsoid(x - 1.0 * floor(x / 1.0) - 0.5, y - 1.06, z - 0.25, 0.42, 0.14, 0.36), abs(x) - 1.45);
  const posts = box(abs(x) - 1.55, y - 1.2, abs(z) - 0.95, 0.05, 1.2, 0.05);
  const awning = box(x, y - 2.3 - z * 0.15, z, 1.7, 0.03, 1.12);
  return min(min(counter, max(crates, abs(x) - 1.45)), min(heap, min(posts, awning)));
}

export function color(x, y, z, k) {
  if (y > 2.0) return fract(x * 1.6) < 0.5 ? rgb(232, 70, 54) : rgb(246, 238, 220);
  if (y > 1.0 && abs(x) < 1.45) {
    const c = floor(x + 2) + floor(hash(k.seed) * 3);
    const t = c - 4 * floor(c / 4);
    if (t < 1) return rgb(246, 214, 50);
    if (t < 2) return rgb(214, 40, 30);
    if (t < 3) return rgb(242, 140, 30);
    return rgb(90, 156, 60);
  }
  return mix(rgb(132, 96, 60), rgb(160, 120, 78), noise3(x * 6, y * 20, z * 6));
}
