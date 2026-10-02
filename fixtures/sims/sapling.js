// A young tree.
export const meta = { name: "sapling", bounds: [0.5, 1.2, 0.5], tags: ["plant"], props: { mass: 4, alive: 1, growth: 0.2, burns: 0.6 } };

export function sdf(x, y, z, k) {
  const stem = capsule(x, y, z, 1.0, 0.03);
  const leaves = sphere(x, y - 0.95, z, 0.3);
  return min(stem, leaves);
}

export function color(x, y, z, k) {
  return y > 0.7 ? rgb(70, 140, 60) : rgb(100, 80, 50);
}
