// A scatter of seashells on the sand: blue mussel shells, ribbed white and
// pink scallops, a whelk, a moon snail, washed up along the tide line.
export const meta = {
  name: "seashells",
  bounds: [0.6, 0.06, 0.6],
  tags: ["prop", "shell", "nonsolid"],
  props: { mass: 0.3, fragile: 0.5 },
};

export function sdf(x, y, z, k) {
  let d = 10.0;
  for (let i = 0; i < 6; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 1.3;
    const r = 0.1 + hash(k.seed, i, 2) * 0.45;
    d = min(d, ellipsoid(x - cos(a) * r, y - 0.01, z - sin(a) * r, 0.045, 0.02, 0.035));
  }
  return d;
}

export function color(x, y, z, k) {
  const i = floor(hash(floor(x * 6.0), floor(z * 6.0), k.seed) * 4.0);
  if (i < 1) return rgb(40, 50, 80);
  if (i < 2) return fract(atan2(z, x) * 6.0) < 0.5 ? rgb(244, 230, 220) : rgb(226, 176, 170);
  if (i < 3) return rgb(230, 214, 190);
  return rgb(246, 244, 238);
}
