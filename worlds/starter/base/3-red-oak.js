// A red oak: a thick dark trunk forking low into heavy limbs under a broad,
// spreading crown of lobes gone russet, burgundy and copper, holding its
// leaves late, brown leaves and acorns under it.
export const meta = {
  name: "red oak",
  bounds: [5.4, 9.6, 5.4],
  tags: ["oak"],
  props: { mass: 5000, burns: 0.45, alive: 1, fuel: 1 },
};

export function sdf(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.26;
  const turn = hash(k.seed, 2) * 6.2832;
  const trunk = cappedCone(x, y - 1.3 * s, z, 1.3 * s, 0.6 * s, 0.360 * s);
  const ring = length2(x, z) - 4.60 * s;
  // Leaves on the ground: only near it is the ring's edge worth finding.
  const ground = y > 0.3 ? max(y - 0.024, ring) : max(cylinder(x, y - 0.012, z, 4.60 * s, 0.012), length2(x, z) - (2.10 + noise3(x * 0.9, 3.0, z * 0.9) * 2.40) * s);
  // Far from the crown, its bounding sphere is enough.
  const far = length3(x, y - 5.20 * s, z) - 5.50 * s;
  if (far > 0.5) return min(min(trunk, ground), far * 0.8);
  // Limbs forking from the trunk's top out to the lobes.
  let limbs = 10.0;
  for (let i = 0; i < 3; i++) {
    const a = turn + i * 2.0944;
    limbs = min(limbs, capsule(x, y, z, 0, 2.21 * s, 0, cos(a) * 2.25 * s, 4.94 * s, sin(a) * 2.25 * s, 0.270 * s));
  }
  const wood = min(trunk, limbs);
  let crown = sphere(x, y - 7.8 * s, z, 2.20 * s);
  for (let i = 0; i < 6; i++) {
    const a = turn + i * 2.39996 + hash(k.seed, i) * 0.6;
    const r = (1.80 + hash(k.seed, i, 3) * 1.50) * s;
    const ly = (4.42 + hash(k.seed, i, 4) * 2.60) * s;
    crown = min(crown, sphere(x - cos(a) * r, y - ly, z - sin(a) * r, (1.60 + hash(k.seed, i, 5) * 0.80) * s));
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
  // The fallen leaves on the ground.
  if (y < 0.06) {
    const g = hash(floor(x * 7.0), floor(z * 7.0), k.seed);
    return mix(hsv(0.025 + hash(k.seed, 3) * 0.035 + (m - 0.5) * 0.05 + (n - 0.5) * 0.02, 0.78, 0.5 + n * 0.26), rgb(120, 84, 44), 0.35 * g) * (0.7 + 0.3 * g);
  }
  // Bark on the trunk and the limbs low in the crown.
  if (y < 2.73 * s && length2(x, z) < 0.96 * s) return mix(rgb(56, 48, 42), rgb(86, 74, 62), noise3(x * 5, y * 5, z * 5));
  if (y < 4.16 * s && n < 0.45 && length2(x, z) < 2.40 * s) return mix(rgb(56, 48, 42), rgb(86, 74, 62), noise3(x * 5, y * 5, z * 5));
  // Deeper in the crown, darker; outside, the sun on it.
  const out = clamp(length3(x, (y - 5.20 * s) * 1.2, z) / (4.67 * s), 0, 1);
  const spec = hash(floor(x * 5.0), floor(y * 5.0) * 31 + floor(z * 5.0), k.seed);
  return hsv(0.025 + hash(k.seed, 3) * 0.035 + (m - 0.5) * 0.05 + (n - 0.5) * 0.02, 0.78, 0.5 + n * 0.26) * (0.62 + 0.38 * out) * (0.9 + 0.2 * spec);
}
