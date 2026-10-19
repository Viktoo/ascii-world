// The gazebo on the green: an eight-sided bandstand of white posts and
// railings on a raised floor, two steps up, under a pointed red roof with a
// weathervane; benches round the inside where people sit and the fiddler
// plays. Its steps face +z.
export const meta = {
  name: "gazebo",
  bounds: [3.6, 5.6, 3.6],
  tags: ["building", "landmark", "shelter"],
  props: { mass: 8000, burns: 0.6 },
  anchors: [
    { kind: "seat", at: [-2.4, 1.0, 0], face: 90 },
    { kind: "seat", at: [2.4, 1.0, 0], face: 270 },
    { kind: "seat", at: [0, 1.0, -2.4], face: 0 },
    { kind: "seat", at: [-1.7, 1.0, -1.7], face: 45 },
    { kind: "seat", at: [1.7, 1.0, -1.7], face: 315 },
    { kind: "light", at: [0, 2.9, 0] },
  ],
};

export function sdf(x, y, z, k) {
  // Fold round the eight sides.
  const a = atan2(x, z);
  const sec = floor(a / 0.7854 + 0.5) * 0.7854;
  const u = x * sin(sec) + z * cos(sec);
  const v = x * cos(sec) - z * sin(sec);
  const r = max(abs(x), abs(z)) * 0.0 + u;
  const floor0 = max(u - 3.0, abs(y - 0.25) - 0.25);
  const step1 = max(max(abs(x) - 0.8, z - 3.6), max(-z + 2.8, abs(y - 0.12) - 0.12));
  const posts = box(u - 2.85, y - 1.75, abs(v) - 1.17, 0.08, 1.25, 0.08);
  const rail = max(max(box(u - 2.85, y - 1.35, v, 0.04, 0.04, 1.18), -box(x, y - 1.3, z - 2.9, 0.8, 1.0, 0.6)), 0.0 - 1.0);
  const roof = max(max(u - 3.35 + (y - 3.0) * 1.3, 3.0 - y), y - 5.2);
  const vane = capsule(x, y, z, 0, 5.0, 0, 0, 5.55, 0, 0.025);
  const benches = max(box(u - 2.55, y - 0.95, v, 0.18, 0.04, 1.0), z - 2.4);
  return min(min(min(floor0, step1), min(posts, rail)), min(min(roof, vane), benches));
}

export function color(x, y, z, k) {
  if (y > 2.95 && y < 5.0) return mix(rgb(176, 44, 38), rgb(196, 60, 46), noise3(x * 4, y * 4, z * 4)) * (fract(y * 4.0) < 0.15 ? 0.85 : 1.0);
  if (y > 5.0) return rgb(40, 40, 40);
  if (y < 0.51) return fract(x * 4.0) < 0.06 ? rgb(130, 110, 90) : rgb(176, 156, 126);
  return rgb(246, 244, 238);
}
