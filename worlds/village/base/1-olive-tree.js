// An old olive tree: a gnarled, twisting silver-brown trunk, sometimes split
// in two, under a loose, rounded crown of grey-green leaves.
export const meta = {
  name: "olive tree",
  bounds: [3.4, 5.2, 3.4],
  tags: ["olive"],
  props: { mass: 1500, burns: 0.5, alive: 1, fuel: 0.9 },
};

export function sdf(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.45;
  const a = hash(k.seed, 2) * 6.28;
  const lx = cos(a) * 0.6 * s;
  const lz = sin(a) * 0.6 * s;
  // A gnarled trunk, split in two, twisting up.
  const t1 = cone(x, y, z, 0, 0, 0, lx, 1.8 * s, lz, 0.34 * s, 0.14 * s);
  const t2 = cone(x, y, z, 0.12, 0, -0.1, -lx * 0.9, 1.6 * s, -lz * 0.7 + 0.25, 0.26 * s, 0.12 * s);
  const trunk = smoothUnion(t1, t2, 0.18) + (noise3(x * 4, y * 2, z * 4) - 0.5) * 0.14;
  // A loose, open crown: several thin, wispy clouds of leaves.
  // Each cloud on a branch reaching out from the top of one of the trunks.
  let crown = 10;
  let limbs = 10;
  for (let i = 0; i < 5; i++) {
    const b = i * 1.2566 + a;
    const r = (0.6 + hash(k.seed, i) * 0.9) * s;
    const cx = lx * 0.5 + cos(b) * r;
    const cy = (2.4 + hash(k.seed, i, 3) * 0.8) * s;
    const cz = lz * 0.5 + sin(b) * r;
    crown = min(crown, ellipsoid(x - cx, y - cy, z - cz, 1.3 * s, 0.65 * s, 1.2 * s));
    const fx = i < 3 ? lx : -lx * 0.9;
    const fz = i < 3 ? lz : -lz * 0.7 + 0.25;
    const fy = (i < 3 ? 1.7 : 1.5) * s;
    limbs = min(limbs, cone(x, y, z, fx, fy, fz, cx, cy - 0.2 * s, cz, 0.13 * s, 0.05 * s));
  }
  crown = crown + (noise3(x * 2.6, y * 2.6, z * 2.6) - 0.5) * 0.55;
  return min(min(trunk * 0.9, limbs), crown * 0.7);
}

export function color(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.45;
  const n = noise3(x * 4.5, y * 4.5, z * 4.5);
  // Bark wherever the wood is, under the leaves too.
  const bark = mix(rgb(96, 88, 72), rgb(150, 142, 124), n);
  if (y < 1.6 * s) return bark;
  if (noise3(x * 2.6, y * 2.6, z * 2.6) < 0.25 && y < 2.4 * s) return bark;
  return mix(rgb(104, 124, 86), rgb(168, 180, 142), n * n);
}
