// An old stump left from a felled maple, its top sawn flat, grey and ringed
// with the years, its bark rough, roots spreading into the ground with moss
// between them; a handy seat.
export const meta = {
  name: "tree stump",
  bounds: [0.6, 0.55, 0.6],
  tags: ["prop", "seat", "wood"],
  props: { mass: 300, burns: 0.5 },
  anchors: [
    { kind: "seat", at: [0, 0.5, 0], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const a = atan2(z, x);
  const rough = 0.3 + 0.02 * sin(a * 9.0) + noise3(x * 8, y * 4, z * 8) * 0.03;
  const trunk = max(length2(x, z) - rough - max(0.12 - y, 0.0) * 0.8, max(y - 0.5 + noise3(x * 5, 0, z * 5) * 0.02, -y));
  let roots = 10.0;
  for (let i = 0; i < 5; i++) {
    const ra = i * 1.2566 + 0.3;
    roots = min(roots, capsule(x, y, z, cos(ra) * 0.2, 0.12, sin(ra) * 0.2, cos(ra) * 0.55, -0.02, sin(ra) * 0.55, 0.06));
  }
  return min(trunk * 0.8, roots);
}

export function color(x, y, z, k) {
  if (y > 0.47) return (fract(length2(x, z) * 28.0 + noise3(x * 6, 0, z * 6) * 0.4) < 0.25 ? rgb(140, 120, 92) : rgb(190, 172, 140)) * (length2(x, z) > 0.28 ? 0.6 : 1.0);
  if (y < 0.05 && noise3(x * 10, y * 10, z * 10) > 0.62) return mix(rgb(70, 100, 46), rgb(100, 130, 60), noise3(x * 30, y * 30, z * 30));
  return (fract(atan2(z, x) * 4.0 + noise3(x * 3, y * 6, z * 3)) < 0.2 ? 0.7 : 1.0) * mix(rgb(70, 58, 46), rgb(100, 84, 66), noise3(x * 8, y * 8, z * 8));
}
