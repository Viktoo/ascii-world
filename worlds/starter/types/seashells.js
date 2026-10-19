// Seashells strewn on the sand: blue-black mussel shells, ribbed scallops
// in white, pink and apricot, a spiralled whelk, a round moon snail, a
// bleached sand dollar, washed up along the tide line.
export const meta = {
  name: "seashells",
  bounds: [0.62, 0.07, 0.62],
  tags: ["prop", "shell", "nonsolid"],
  props: { mass: 0.3, fragile: 0.5 },
};

export function sdf(x, y, z, k) {
  let d = 10.0;
  for (let i = 0; i < 6; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 1.3;
    const r = 0.08 + hash(k.seed, i, 2) * 0.48;
    const q = rotY(x - cos(a) * r, y, z - sin(a) * r, a * 3.0);
    const kind = i % 3;
    let sh = 10.0;
    if (kind == 0) sh = max(ellipsoid(q.x, q.y - 0.006, q.z, 0.05, 0.022, 0.048), max(-q.y, q.z * 0.6 - 0.03));
    if (kind == 1) sh = ellipsoid(q.x, q.y - 0.008, q.z, 0.022, 0.014, 0.045);
    if (kind == 2) sh = min(cappedCone(q.x, q.y - 0.018, q.z, 0.018, 0.024, 0.004), sphere(q.x, q.y - 0.012, q.z, 0.02));
    d = min(d, sh);
  }
  return d;
}

export function color(x, y, z, k) {
  const cell = floor(hash(floor(x * 5.0 + 50.0), floor(z * 5.0 + 50.0), k.seed) * 4.0);
  const ribs = fract(atan2(z, x) * 5.0 + length2(x, z) * 30.0) < 0.3 ? 0.82 : 1.0;
  if (y < 0.004) return rgb(214, 200, 166);
  if (cell < 1) return mix(rgb(30, 34, 60), rgb(70, 90, 130), noise3(x * 60, y * 60, z * 60));
  if (cell < 2) return rgb(244, 214, 200) * ribs;
  if (cell < 3) return mix(rgb(236, 190, 140), rgb(250, 236, 220), fract(length2(x, z) * 60.0)) * ribs;
  return rgb(244, 240, 228) * ribs;
}
