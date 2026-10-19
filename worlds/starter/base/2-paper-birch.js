// A paper birch: a slender white trunk marked with black forking into thin
// limbs, a narrow, airy crown of small lobes of leaves gone bright gold.
export const meta = {
  name: "paper birch",
  bounds: [2.8, 9.2, 2.8],
  tags: ["birch"],
  props: { mass: 900, burns: 0.6, alive: 1, fuel: 0.9 },
};

export function sdf(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.26;
  const turn = hash(k.seed, 2) * 6.2832;
  const trunk = cappedCone(x, y - 2.1 * s, z, 2.1 * s, 0.17 * s, 0.102 * s);
  const ring = 10.0;
  const ground = 10.0;
  // Far from the crown, its bounding sphere is enough.
  const far = length3(x, y - 6.10 * s, z) - 3.22 * s;
  if (far > 0.5) return min(min(trunk, ground), far * 0.8);
  // Limbs forking from the trunk's top out to the lobes.
  let limbs = 10.0;
  for (let i = 0; i < 2; i++) {
    const a = turn + i * 3.1416;
    limbs = min(limbs, capsule(x, y, z, 0, 3.57 * s, 0, cos(a) * 0.90 * s, 5.91 * s, sin(a) * 0.90 * s, 0.077 * s));
  }
  const wood = min(trunk, limbs);
  let crown = sphere(x, y - 8.0 * s, z, 1.21 * s);
  for (let i = 0; i < 4; i++) {
    const a = turn + i * 2.39996 + hash(k.seed, i) * 0.6;
    const r = (0.72 + hash(k.seed, i, 3) * 0.60) * s;
    const ly = (5.53 + hash(k.seed, i, 4) * 1.90) * s;
    crown = min(crown, sphere(x - cos(a) * r, y - ly, z - sin(a) * r, (0.88 + hash(k.seed, i, 5) * 0.44) * s));
  }
  // Leafy edges and holes: a broad wobble and a fine one.
  const leafy = crown + (noise3(x * 1.3, y * 1.3, z * 1.3) - 0.5) * 0.9 + (noise3(x * 3.7, y * 3.7, z * 3.7) - 0.5) * 0.35;
  return min(min(wood, ground), leafy * 0.7);
}

export function color(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.26;
  let f = k.a;
  if (hash(k.seed, 7) < 0.25) f = hash(k.seed, 8);
  const n = noise3(x * 2.6, y * 2.6, z * 2.6);
  const m = noise3(x * 0.45 + k.seed * 0.01, y * 0.45, z * 0.45);
  // Bark on the trunk and the limbs low in the crown.
  if (y < 4.41 * s && length2(x, z) < 0.27 * s) return (noise3(x * 4, y * 9, z * 4) > 0.7 ? rgb(46, 42, 40) : mix(rgb(222, 218, 206), rgb(244, 240, 230), noise3(x * 8, y * 3, z * 8)));
  if (y < 5.34 * s && n < 0.45 && length2(x, z) < 0.96 * s) return (noise3(x * 4, y * 9, z * 4) > 0.7 ? rgb(46, 42, 40) : mix(rgb(222, 218, 206), rgb(244, 240, 230), noise3(x * 8, y * 3, z * 8)));
  // Deeper in the crown, darker; outside, the sun on it.
  const out = clamp(length3(x, (y - 6.10 * s) * 1.2, z) / (2.74 * s), 0, 1);
  const spec = hash(floor(x * 5.0), floor(y * 5.0) * 31 + floor(z * 5.0), k.seed);
  return hsv(0.115 + (m - 0.5) * 0.05 + (n - 0.5) * 0.02, 0.8, 0.66 + n * 0.28) * (0.62 + 0.38 * out) * (0.9 + 0.2 * spec);
}
