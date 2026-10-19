// A well-worn fiddle of honey-coloured maple and its bow, for the dances at the tavern.
export const meta = {
  name: "fiddle",
  bounds: [0.12, 0.06, 0.32],
  tags: ["item", "instrument"],
  props: { mass: 0.5, fragile: 0.6, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const body = max(min(ellipsoid(x, y - 0.03, z + 0.08, 0.1, 0.03, 0.09), ellipsoid(x, y - 0.03, z - 0.06, 0.085, 0.03, 0.075)), -ellipsoid(abs(x) - 0.11, y - 0.03, z, 0.035, 0.05, 0.04));
  const neck = box(x, y - 0.04, z - 0.22, 0.016, 0.012, 0.09);
  const scroll = sphere(x, y - 0.045, z - 0.31, 0.02);
  return min(body, min(neck, scroll));
}

export function color(x, y, z, k) {
  if (z < -0.12) return rgb(40, 30, 26);
  return mix(rgb(160, 84, 34), rgb(196, 120, 50), noise3(x * 30, y * 30, z * 8));
}
