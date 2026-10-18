// Rosa's wood-fired bread oven outside the bakery: a whitewashed dome on
// a stone base, a dark arched mouth glowing with embers, a little chimney,
// a long wooden peel leaning against it and a stack of charcoal sacks.
export const meta = {
  name: "bread oven",
  bounds: [1.1, 2.3, 1.1],
  tags: ["prop", "oven", "fire"],
  props: { mass: 6000, heat: 400, light: 1 },
  sound: { hard: 0.9, dry: 0.5, ring: 0.1 },
  anchors: [{ kind: "light", at: [0, 1.1, 1.0] }],
};

export function sdf(x, y, z, k) {
  const base = box(x, y - 0.45, z, 1.0, 0.45, 1.0);
  const dome = max(length3(x, y - 0.9, z) - 0.95, 0.9 - y);
  const mouth = min(box(x, y - 1.1, z - 0.9, 0.3, 0.2, 0.4), max(length2(x, y - 1.3) - 0.3, abs(z - 0.9) - 0.4));
  const chim = cylinder(x + 0.4, y - 1.9, z + 0.3, 0.12, 0.35);
  const peel = capsule(x, y, z, 0.95, 0.0, 0.7, 1.05, 2.0, 0.85, 0.03);
  return min(max(min(base, dome), -mouth), min(chim, peel));
}

export function color(x, y, z, k) {
  if (z > 0.55 && abs(x) < 0.32 && y > 0.9 && y < 1.62) return glow(rgb(255, 120, 40), 0.8);
  if (x > 0.9) return rgb(150, 112, 70);
  if (y < 0.9) return mix(rgb(150, 140, 126), rgb(178, 168, 150), noise3(x * 3, y * 3, z * 3));
  if (y > 1.8) return rgb(60, 54, 50);
  return mix(rgb(236, 230, 218), rgb(248, 244, 236), noise3(x * 3, y * 3, z * 3));
}
