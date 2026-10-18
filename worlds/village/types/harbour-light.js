// The harbour light at the end of the breakwater: a squat round tower of
// white stone banded in green, a gallery, and a green lantern that shines
// at night to mark the harbour mouth.
export const meta = {
  name: "harbour light",
  bounds: [1.5, 6.4, 1.5],
  tags: ["landmark", "lighthouse"],
  props: { mass: 60000, light: 1 },
  sound: { hard: 1, dry: 0.5, ring: 0.3 },
  anchors: [{ kind: "light", at: [0, 5.6, 0] }],
};

export function sdf(x, y, z, k) {
  const tower = cappedCone(x, y - 2.3, z, 2.3, 1.3, 0.95);
  const gallery = cylinder(x, y - 4.65, z, 1.3, 0.08);
  const rail = max(abs(length2(x, z) - 1.25) - 0.03, abs(y - 5.05) - 0.35);
  const lantern = cylinder(x, y - 5.25, z, 0.6, 0.55);
  const cap = cappedCone(x, y - 6.0, z, 0.22, 0.7, 0.1);
  return min(min(tower, gallery), min(rail, min(lantern, cap)));
}

export function color(x, y, z, k) {
  if (y > 4.75 && y < 5.75 && length2(x, z) < 0.62) return glow(rgb(90, 255, 140));
  if (y > 5.75) return rgb(36, 90, 60);
  if (y > 4.5) return rgb(36, 90, 60);
  if (abs(y - 3.0) < 0.3 || abs(y - 1.2) < 0.3) return rgb(46, 120, 80);
  if (y < 1.7 && y > 0.1 && length2(x, z) > 1.0 && abs(x) < 0.4 && z > 0) return rgb(60, 50, 40);
  return mix(rgb(236, 232, 222), rgb(248, 246, 240), noise3(x * 3, y * 3, z * 3));
}
