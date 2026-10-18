// Bougainvillea trained up a house front: a twisted woody stem rising
// against the wall and a flat spread of leaves thick with magenta (or
// purple, or coral) bracts fanning over the plaster beside the door. It
// hugs the wall behind it (-z); its face is +z.
export const meta = {
  name: "bougainvillea",
  bounds: [1.3, 4.3, 0.35],
  tags: ["flower", "bush", "nonsolid"],
  props: { mass: 40, alive: 1, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const stem = capsule(x, y, z, 0.1, 0, -0.12, -0.1, 2.6, -0.15, 0.06);
  const n = noise3(x * 2.6, y * 2.6, k.seed);
  // A fan of growth spread flat on the wall, fuller at the top.
  const fan = ellipsoid(x, y - 3.1, z + 0.12, 1.2, 1.1, 0.2) + (n - 0.5) * 0.25;
  const tail = ellipsoid(x - 0.5, y - 2.1, z + 0.12, 0.45, 0.6, 0.16) + (n - 0.5) * 0.15;
  return min(stem, min(fan, tail) * 0.8);
}

export function color(x, y, z, k) {
  if (y < 1.9) return rgb(110, 82, 56);
  const c = floor(hash(k.seed) * 3);
  const bloom = c < 1 ? rgb(222, 34, 142) : c < 2 ? rgb(176, 56, 190) : rgb(244, 96, 96);
  // Bracts in clusters over the leaves.
  const gx = floor(x * 6.0);
  const gy = floor(y * 6.0);
  const fx = fract(x * 6.0) - 0.5;
  const fy = fract(y * 6.0) - 0.5;
  if (hash(gx, gy, k.seed) < 0.7 && fx * fx + fy * fy < 0.18) return bloom * (0.85 + 0.15 * hash(gy, gx));
  return mix(rgb(46, 98, 40), rgb(80, 132, 54), noise3(x * 5, y * 5, z * 5));
}
