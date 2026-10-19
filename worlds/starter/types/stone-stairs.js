// Stone stairs down the granite face of the landing: twelve worn steps rising
// 3 m from the lane below (+z) to the one above, low walls either side
// topped with flat coping.
export const meta = {
  name: "stone stairs",
  bounds: [1.55, 4.1, 1.9],
  tags: ["structure", "stairs"],
  props: { mass: 60000 },
  sound: { hard: 1, dry: 0.5, ring: 0.1 },
  anchors: [{ kind: "stairs", at: [0, 0, 2.2], to: [0, 3.0, -2.3], face: 180 }],
};

export function sdf(x, y, z, k) {
  // Twelve steps of 0.25 m, 0.3 m deep, from the foot (+z) up to the top
  // (-z), each a block from a deep footing.
  const u = (1.8 - z) / 0.3;
  const i = clamp(floor(u), 0, 11);
  const a = box(x, y - ((i + 1) * 0.25 - 1.0) * 0.5, z - 1.8 + (i + 0.5) * 0.3, 1.2, ((i + 1) * 0.25 + 1.0) * 0.5, 0.15);
  const j = clamp(i + 1, 0, 11);
  const b = box(x, y - ((j + 1) * 0.25 - 1.0) * 0.5, z - 1.8 + (j + 0.5) * 0.3, 1.2, ((j + 1) * 0.25 + 1.0) * 0.5, 0.15);
  const h = clamp(i - 1, 0, 11);
  const c = box(x, y - ((h + 1) * 0.25 - 1.0) * 0.5, z - 1.8 + (h + 0.5) * 0.3, 1.2, ((h + 1) * 0.25 + 1.0) * 0.5, 0.15);
  const wall = max(box(abs(x) - 1.38, y - 1.5, z, 0.16, 2.5, 1.8), (y - 1.0 - (1.8 - z) / 3.6 * 3.0) * 0.76);
  return min(min(a, b), min(c, wall));
}

export function color(x, y, z, k) {
  const n = noise3(x * 2, y * 2, z * 2);
  if (abs(x) > 1.21 && y > (1.8 - z) / 3.6 * 3.0 + 0.85) return rgb(236, 230, 216);
  return mix(rgb(196, 184, 160), rgb(220, 210, 188), n);
}
