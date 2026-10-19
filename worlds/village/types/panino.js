// A panino: a crusty roll split and stuffed with mortadella, cheese and rocket.
export const meta = {
  name: "panino",
  bounds: [0.13, 0.06, 0.06],
  tags: ["item", "food"],
  props: { mass: 0.25, edible: 0.6, burns: 0.5 },
};

export function sdf(x, y, z, k) {
  return ellipsoid(x, y - 0.04, z, 0.12, 0.045, 0.055);
}

export function color(x, y, z, k) {
  if (abs(y - 0.04) < 0.008) return rgb(230, 150, 150);
  if (abs(y - 0.05) < 0.008) return rgb(70, 140, 50);
  return rgb(210, 150, 80);
}
