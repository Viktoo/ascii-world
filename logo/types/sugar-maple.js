// A sugar maple in its autumn colour: a straight grey-brown trunk forking into
// limbs under a tall crown of separate leafy lobes gone scarlet, flame orange
// or gold, the colour wandering across it.
export const meta = {
  name: "sugar maple",
  bounds: [4.6, 10.6, 4.6],
  tags: ["base", "tree"],
  props: { mass: 3500, burns: 0.5, alive: 1, fuel: 1 },
};

export function sdf(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.26;
  const turn = hash(k.seed, 2) * 6.2832;
  const trunk = cappedCone(x, y - 1.6 * s, z, 1.6 * s, 0.42 * s, 0.252 * s);
  const ring = 10.0;
  const ground = 10.0;
  // Far from the crown, its bounding sphere is enough.
  const far = length3(x, y - 5.80 * s, z) - 4.88 * s;
  if (far > 0.5) return min(min(trunk, ground), far * 0.8);
  // Limbs forking from the trunk's top out to the lobes.
  let limbs = 10.0;
  for (let i = 0; i < 3; i++) {
    const a = turn + i * 2.0944;
    limbs = min(limbs, capsule(x, y, z, 0, 2.72 * s, 0, cos(a) * 1.72 * s, 5.54 * s, sin(a) * 1.72 * s, 0.189 * s));
  }
  const wood = min(trunk, limbs);
  let crown = sphere(x, y - 8.4 * s, z, 2.09 * s);
  for (let i = 0; i < 5; i++) {
    const a = turn + i * 2.39996 + hash(k.seed, i) * 0.6;
    const r = (1.38 + hash(k.seed, i, 3) * 1.15) * s;
    const ly = (5.02 + hash(k.seed, i, 4) * 2.60) * s;
    crown = min(crown, sphere(x - cos(a) * r, y - ly, z - sin(a) * r, (1.52 + hash(k.seed, i, 5) * 0.76) * s));
  }
  // Leafy edges and holes: a broad wobble, then clusters of leaves, then
  // the leaves themselves ruffling the surface.
  const base = crown + (noise3(x * 1.3, y * 1.3, z * 1.3) - 0.5) * 0.9;
  if (base > 0.6) return min(min(wood, ground), base * 0.7);
  const clumps = (noise3(x * 3.7, y * 3.7, z * 3.7) - 0.5) * 0.42;
  const leaves = (noise3(x * 9.0, y * 9.0, z * 9.0) - 0.5) * 0.16 + (noise3(x * 21.0, y * 21.0, z * 21.0) - 0.5) * 0.06;
  return min(min(wood, ground), (base + clumps + leaves) * 0.55);
}

export function color(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.26;
  let f = k.a;
  if (hash(k.seed, 7) < 0.25) f = hash(k.seed, 8);
  const n = noise3(x * 2.6, y * 2.6, z * 2.6);
  const m = noise3(x * 0.45 + k.seed * 0.01, y * 0.45, z * 0.45);
  // Bark on the trunk and the limbs low in the crown.
  if (y < 3.36 * s && length2(x, z) < 0.67 * s) return mix(rgb(84, 76, 70), rgb(118, 108, 98), noise3(x * 6, y * 2, z * 6));
  if (y < 4.76 * s && n < 0.45 && length2(x, z) < 1.84 * s) return mix(rgb(84, 76, 70), rgb(118, 108, 98), noise3(x * 6, y * 2, z * 6));
  // Deeper in the crown, darker; outside, the sun on it.
  const out = clamp(length3(x, (y - 5.80 * s) * 1.2, z) / (4.15 * s), 0, 1);
  // Leaf by leaf: each its own shade and a touch of its own hue, the
  // hollows between clusters in shadow.
  const leaf = hash(floor(x * 11.0), floor(y * 11.0) * 31 + floor(z * 11.0), k.seed);
  const cl = noise3(x * 3.7, y * 3.7, z * 3.7);
  const fine = noise3(x * 9.0, y * 9.0, z * 9.0);
  const hue = (f < 0.4 ? 0.005 : (f < 0.75 ? 0.045 : 0.1)) + (m - 0.5) * 0.09 + (n - 0.5) * 0.02 + (leaf - 0.5) * 0.035;
  const lit = (0.55 + 0.45 * out) * (0.8 + 0.32 * smoothstep(0.25, 0.75, cl)) * (0.85 + 0.25 * fine) * (0.86 + 0.24 * leaf);
  return hsv(hue, 0.8 + 0.15 * leaf, 0.7 + n * 0.22) * lit;
}
