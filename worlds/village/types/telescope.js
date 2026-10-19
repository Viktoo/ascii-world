// A brass telescope on a wooden tripod, pointed out to sea.
export const meta = {
  name: "telescope",
  bounds: [0.45, 1.6, 0.55],
  tags: ["furniture", "prop"],
  props: { mass: 8 },
};

export function sdf(x, y, z, k) {
  const legs = min(capsule(x, y, z, -0.35, 0, -0.2, 0, 1.2, 0, 0.02), min(capsule(x, y, z, 0.35, 0, -0.2, 0, 1.2, 0, 0.02), capsule(x, y, z, 0, 0, 0.35, 0, 1.2, 0, 0.02)));
  const tube = cone(x, y, z, 0, 1.2, -0.3, 0, 1.45, 0.5, 0.06, 0.04);
  return min(legs, tube);
}

export function color(x, y, z, k) {
  if (y > 1.18) return rgb(200, 160, 70);
  return rgb(110, 76, 48);
}
