// A cast-iron mooring bollard on the quay, black, worn bright on top.
export const meta = {
  name: "bollard",
  bounds: [0.3, 0.75, 0.3],
  tags: ["prop"],
  props: { mass: 300 },
  sound: { hard: 1, dry: 0.4, ring: 0.6 },
  anchors: [{ kind: "seat", at: [0, 0.6, 0], face: 0 }],
};

export function sdf(x, y, z, k) {
  return min(cappedCone(x, y - 0.3, z, 0.3, 0.22, 0.16), min(cylinder(x, y - 0.62, z, 0.25, 0.06), cylinder(x, y - 0.03, z, 0.28, 0.03)));
}

export function color(x, y, z, k) {
  if (y > 0.65) return rgb(110, 104, 96);
  return rgb(30, 30, 32);
}
