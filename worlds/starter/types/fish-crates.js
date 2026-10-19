// A stack of wooden fish crates on the quay, some painted blue, the top one
// full of silver fish on ice.
export const meta = {
  name: "fish crates",
  bounds: [0.7, 0.9, 0.5],
  tags: ["prop"],
  props: { mass: 40, burns: 0.6 },
  sound: { hard: 0.5, dry: 0.8, ring: 0.05 },
};

export function sdf(x, y, z, k) {
  const s = hash(k.seed) * 0.3;
  const a = box(x + 0.3, y - 0.15, z, 0.34, 0.15, 0.24);
  const b = box(x + 0.28, y - 0.45, z + 0.04, 0.34, 0.15, 0.24);
  const c = box(x - 0.36, y - 0.15, z - 0.1 + s, 0.34, 0.15, 0.24);
  const d = max(box(x - 0.34, y - 0.45, z - 0.08 + s, 0.34, 0.15, 0.24), -box(x - 0.34, y - 0.5, z - 0.08 + s, 0.3, 0.12, 0.2));
  const fish = box(x - 0.34, y - 0.52, z - 0.08 + s, 0.3, 0.05, 0.2);
  return min(min(a, b), min(min(c, d), fish));
}

export function color(x, y, z, k) {
  if (y > 0.5 && y < 0.58 && x < 0.0) return mix(rgb(190, 200, 210), rgb(230, 236, 240), noise3(x * 30, 0, z * 30));
  if (fract(y / 0.3 + 0.1) < 0.12) return rgb(80, 60, 40);
  if (x > 0) return rgb(50, 110, 170);
  return mix(rgb(160, 122, 80), rgb(186, 150, 100), noise3(x * 6, y * 20, z * 6));
}
