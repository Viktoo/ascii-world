// Eelgrass on the cove's floor: long ribbons swaying in the cold water.
export const meta = {
  name: "eelgrass",
  bounds: [0.6, 0.9, 0.6],
  tags: ["seagrass", "nonsolid", "water"],
  props: { mass: 1, burns: 0, alive: 1, wet: 1 },
  grows: { depth: [1.0, 14] },
};

export function sdf(x, y, z, k) {
  let d = 10;
  for (let i = 0; i < 5; i++) {
    const a = i * 2.39996 + hash(k.seed, i);
    const r = hash(k.seed, i, 2) * 0.35;
    const h = 0.5 + hash(k.seed, i, 3) * 0.35;
    d = min(d, capsule(x, y, z, cos(a) * r, 0, sin(a) * r, cos(a) * r + 0.12, h, sin(a) * r, 0.02));
  }
  return d;
}

export function color(x, y, z, k) {
  return mix(rgb(40, 86, 50), rgb(90, 120, 60), clamp(y, 0, 1));
}
