// The broad steps from the Piazza del Mare up to San Pietro's door: twelve
// wide marble-edged steps rising 3 m from the piazza (+z) to the church.
export const meta = {
  name: "church steps",
  bounds: [4.1, 4.1, 1.9],
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
  const a = box(x, y - ((i + 1) * 0.25 - 1.0) * 0.5, z - 1.8 + (i + 0.5) * 0.3, 4.0, ((i + 1) * 0.25 + 1.0) * 0.5, 0.15);
  const j = clamp(i + 1, 0, 11);
  const b = box(x, y - ((j + 1) * 0.25 - 1.0) * 0.5, z - 1.8 + (j + 0.5) * 0.3, 4.0, ((j + 1) * 0.25 + 1.0) * 0.5, 0.15);
  const h = clamp(i - 1, 0, 11);
  const c = box(x, y - ((h + 1) * 0.25 - 1.0) * 0.5, z - 1.8 + (h + 0.5) * 0.3, 4.0, ((h + 1) * 0.25 + 1.0) * 0.5, 0.15);
  return min(min(a, b), c);
}

export function color(x, y, z, k) {
  const n = noise3(x * 2, y * 2, z * 2);
  if (fract((1.8 - z) / 0.3) > 0.82) return rgb(244, 240, 230);
  return mix(rgb(196, 184, 160), rgb(220, 210, 188), n);
}
