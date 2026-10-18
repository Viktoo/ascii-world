// A sawhorse: two crossed-leg trestles joined by a beam, a log laid across
// it half sawn through, the bow saw resting on top.
export const meta = {
  name: "sawhorse",
  bounds: [1.4, 1.1, 0.6],
  tags: ["prop"],
  props: { mass: 60, burns: 0.7 },
  sound: { hard: 0.5, dry: 0.8, ring: 0.05 },
};

export function sdf(x, y, z, k) {
  const ax = abs(x) - 0.55;
  const legs = min(capsule(ax, y, z, 0, 0, -0.4, 0, 0.9, 0.12, 0.04), capsule(ax, y, z, 0, 0, 0.4, 0, 0.9, -0.12, 0.04));
  const beam = box(x, y - 0.6, z, 0.6, 0.04, 0.04);
  const log = max(length2(y - 0.98, z) - 0.17, abs(x) - 1.3);
  const saw = max(abs(length2(x - 0.2, y - 1.15) - 0.35) - 0.02, max(abs(z - 0.25) - 0.02, 1.12 - y));
  return min(min(legs, beam), min(log, saw));
}

export function color(x, y, z, k) {
  if (abs(z - 0.25) < 0.03 && y > 1.14) return rgb(170, 60, 40);
  if (abs(x) > 1.27) return rgb(214, 180, 126);
  if (y > 0.8) return mix(rgb(96, 72, 50), rgb(120, 90, 60), noise3(x * 6, y * 6, z * 6));
  return rgb(150, 116, 76);
}
