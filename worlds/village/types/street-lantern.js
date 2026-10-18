// An iron street lantern of Marinella: a slim black post on a stone foot,
// a curled bracket, and a glass lantern with a warm light.
export const meta = {
  name: "street lantern",
  bounds: [0.5, 3.7, 0.3],
  tags: ["lamppost", "prop"],
  props: { mass: 120, light: 1 },
  sound: { hard: 0.9, dry: 0.5, ring: 0.5 },
  anchors: [{ kind: "light", at: [0.32, 3.1, 0] }],
};

export function sdf(x, y, z, k) {
  const foot = cappedCone(x, y - 0.2, z, 0.2, 0.2, 0.12);
  const post = cylinder(x, y - 1.7, z, 0.055, 1.6);
  const curl = max(abs(length2(x - 0.18, y - 3.3) - 0.18) - 0.03, max(-x, abs(z) - 0.03));
  const lamp = cappedCone(x - 0.36, y - 3.0, z, 0.24, 0.12, 0.18);
  const cap = cappedCone(x - 0.36, y - 3.32, z, 0.1, 0.22, 0.04);
  return min(min(foot, post), min(curl, min(lamp, cap)));
}

export function color(x, y, z, k) {
  if (y < 0.4) return rgb(196, 186, 166);
  if (x > 0.2 && y > 2.78 && y < 3.22) return glow(rgb(255, 214, 140));
  return rgb(32, 32, 34);
}
