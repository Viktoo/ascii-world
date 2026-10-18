// Bougainvillea climbing a house front: a twisted woody stem up the wall
// and a great spill of magenta (or purple, or coral) bracts and green
// leaves spreading over the plaster above the door.
export const meta = {
  name: "bougainvillea",
  bounds: [1.7, 4.4, 0.6],
  tags: ["flower", "bush", "nonsolid"],
  props: { mass: 40, alive: 1, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const stem = cone(x, y, z, 0, 0, 0, 0.3, 2.6, -0.2, 0.08, 0.04);
  const n = noise3(x * 2.2, y * 2.2, z * 2.2 + k.seed);
  const mass = ellipsoid(x - 0.2, y - 3.2, z + 0.2, 1.6, 1.0, 0.36);
  const spill = ellipsoid(x + 0.7, y - 2.4, z + 0.25, 0.8, 0.9, 0.3);
  return min(stem, (min(mass, spill) + (n - 0.5) * 0.4) * 0.75);
}

export function color(x, y, z, k) {
  if (y < 2.0) return rgb(96, 70, 48);
  const n = noise3(x * 5, y * 5, z * 5 + k.seed);
  const c = floor(hash(k.seed) * 3);
  const flower = c < 1 ? rgb(214, 36, 140) : c < 2 ? rgb(170, 50, 170) : rgb(240, 90, 90);
  return n > 0.42 ? flower : rgb(70, 120, 50);
}
