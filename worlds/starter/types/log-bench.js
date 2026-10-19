// A log bench: half a pine log, flat side up, on two short stumps;
// three can sit on it.
export const meta = {
  name: "log bench",
  bounds: [1.35, 0.5, 0.3],
  tags: ["bench", "furniture", "prop"],
  props: { mass: 120, burns: 0.6 },
  sound: { hard: 0.5, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "seat", at: [-0.8, 0.44, 0], face: 0 },
    { kind: "seat", at: [0, 0.44, 0], face: 0 },
    { kind: "seat", at: [0.8, 0.44, 0], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const log = max(max(length2(y - 0.25, z) - 0.24, abs(x) - 1.3), y - 0.44);
  const stumps = cylinder(abs(x) - 0.9, y - 0.12, z, 0.16, 0.12);
  return min(log, stumps);
}

export function color(x, y, z, k) {
  if (y > 0.42) return rgb(196, 160, 112);
  return mix(rgb(96, 72, 50), rgb(120, 90, 60), noise3(x * 6, y * 6, z * 6));
}
