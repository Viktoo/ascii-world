// Built-in mushroom: edible, grows where it is damp.
export const meta = { name: "mushroom", bounds: [0.12, 0.14, 0.12], tags: ["builtin", "mushroom", "small", "nonsolid", "food"], props: { mass: 0.05, edible: 0.35, alive: 1, burns: 0.4 } };

export function sdf(x, y, z, k) {
  const stem = cylinder(x, y + 0.07, z, 0.025, 0.06);
  const cap = max(sphere(x, y + 0.02, z, 0.1), -(y - 0.02));
  return min(stem, cap);
}

export function color(x, y, z, k) {
  if (y > 0.015) {
    return mix(rgb(176, 72, 52), rgb(214, 170, 120), hash(k.seed));
  }
  return rgb(230, 222, 200);
}
