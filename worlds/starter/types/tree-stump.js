// An old stump left from a felled tree, its top sawn flat and grey, moss
// round its roots; a handy seat.
export const meta = {
  name: "tree stump",
  bounds: [0.55, 0.55, 0.55],
  tags: ["prop", "seat", "wood"],
  props: { mass: 300, burns: 0.5 },
  anchors: [
    { kind: "seat", at: [0, 0.5, 0], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  return min(cappedCone(x, y - 0.25, z, 0.25, 0.48, 0.38), max(cylinder(x, y - 0.06, z, 0.55, 0.06), length2(x, z) - 0.52 + noise3(x * 6, 0, z * 6) * 0.2));
}

export function color(x, y, z, k) {
  if (y > 0.48) return fract(length2(x, z) * 14.0) < 0.2 ? rgb(150, 130, 100) : rgb(186, 170, 140);
  if (y < 0.14) return rgb(80, 110, 50);
  return mix(rgb(70, 58, 46), rgb(98, 82, 64), noise3(x * 8, y * 8, z * 8));
}
