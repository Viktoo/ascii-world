// A bunch of garden flowers wrapped in brown paper and tied with string: rust
// and gold mums, purple asters, a few sprigs of goldenrod.
export const meta = {
  name: "bunch of flowers",
  bounds: [0.12, 0.32, 0.12],
  tags: ["item"],
  props: { mass: 0.3, alive: 1, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const paper = max(max(cappedCone(x, y - 0.11, z, 0.11, 0.025, 0.08), -cappedCone(x, y - 0.13, z, 0.1, 0.015, 0.07)), y - 0.22);
  let d = paper;
  for (let i = 0; i < 7; i++) {
    const a = i * 2.39996;
    const r = 0.015 + (i % 3) * 0.025;
    const hx = cos(a) * r;
    const hz = sin(a) * r;
    const hy = 0.24 + hash(i, 2) * 0.05;
    d = min(d, capsule(x, y, z, 0, 0.04, 0, hx, hy, hz, 0.005));
    d = min(d, ellipsoid(x - hx, y - hy, z - hz, 0.032, 0.022, 0.032));
  }
  return d;
}

export function color(x, y, z, k) {
  if (y < 0.215) return abs(y - 0.12) < 0.006 ? rgb(230, 220, 200) : mix(rgb(176, 140, 96), rgb(196, 160, 112), noise3(x * 40, y * 40, z * 40));
  const a = atan2(z, x);
  const i = floor(fract(a / 6.2832 * 2.0 + y * 3.0) * 4.0);
  if (y < 0.235 && length2(x, z) < 0.05) return rgb(70, 110, 50);
  const petal = 0.85 + 0.15 * step(0.5, fract(atan2(z, x) * 4.0));
  if (i < 1) return rgb(196, 90, 30) * petal;
  if (i < 2) return rgb(236, 180, 40) * petal;
  if (i < 3) return rgb(140, 80, 190) * petal;
  return rgb(170, 40, 50) * petal;
}
