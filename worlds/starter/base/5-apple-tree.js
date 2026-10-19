// An old apple tree of the orchards: a short crooked trunk forking low into a
// rounded crown of yellowing leaves hung with red apples, windfalls in the
// grass beneath.
export const meta = {
  name: "apple tree",
  bounds: [3.2, 4.8, 3.2],
  tags: ["apple"],
  props: { mass: 900, burns: 0.5, alive: 1, fuel: 0.9 },
};

export function sdf(x, y, z, k) {
  const s = 0.78 + hash(k.seed) * 0.26;
  const turn = hash(k.seed, 2) * 6.2832;
  const trunk = cappedCone(x, y - 0.65 * s, z, 0.65 * s, 0.22 * s, 0.132 * s);
  const ring = length2(x, z) - 2.38 * s;
  // Leaves on the ground: only near it is the ring's edge worth finding.
  const ground = y > 0.3 ? max(y - 0.024, ring) : max(cylinder(x, y - 0.012, z, 2.38 * s, 0.012), length2(x, z) - (1.05 + noise3(x * 0.9, 3.0, z * 0.9) * 1.32) * s);
  // Far from the crown, its bounding sphere is enough.
  const far = length3(x, y - 2.45 * s, z) - 2.88 * s;
  if (far > 0.5) return min(min(trunk, ground), far * 0.8);
  // Limbs forking from the trunk's top out to the lobes.
  let limbs = 10.0;
  for (let i = 0; i < 3; i++) {
    const a = turn + i * 2.0944;
    limbs = min(limbs, capsule(x, y, z, 0, 1.10 * s, 0, cos(a) * 1.12 * s, 2.33 * s, sin(a) * 1.12 * s, 0.099 * s));
  }
  const wood = min(trunk, limbs);
  let crown = sphere(x, y - 3.6 * s, z, 1.21 * s);
  for (let i = 0; i < 5; i++) {
    const a = turn + i * 2.39996 + hash(k.seed, i) * 0.6;
    const r = (0.90 + hash(k.seed, i, 3) * 0.75) * s;
    const ly = (2.10 + hash(k.seed, i, 4) * 1.15) * s;
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
  // The fallen leaves on the ground.
  if (y < 0.06) {
    const g = hash(floor(x * 7.0), floor(z * 7.0), k.seed);
    return mix(hsv(0.16 + (m - 0.5) * 0.06 + (n - 0.5) * 0.04, 0.62, 0.46 + n * 0.22), rgb(120, 84, 44), 0.35 * g) * (0.7 + 0.3 * g);
  }
  // Bark on the trunk and the limbs low in the crown.
  if (y < 1.37 * s && length2(x, z) < 0.35 * s) return mix(rgb(70, 58, 46), rgb(98, 82, 64), noise3(x * 6, y * 6, z * 6));
  if (y < 1.99 * s && n < 0.45 && length2(x, z) < 1.20 * s) return mix(rgb(70, 58, 46), rgb(98, 82, 64), noise3(x * 6, y * 6, z * 6));
  // Apples dotted thick over the outside of the crown.
  const fx = fract(x * 2.4) - 0.5;
  const fy = fract(y * 2.4) - 0.5;
  const fz = fract(z * 2.4) - 0.5;
  if (hash(floor(x * 2.4), floor(y * 2.4) * 7 + floor(z * 2.4), k.seed) < 0.3 && fx * fx + fy * fy + fz * fz < 0.09) return mix(rgb(176, 24, 28), rgb(220, 60, 40), hash(floor(z * 2.4), floor(x * 2.4)));
  // Deeper in the crown, darker; outside, the sun on it.
  const out = clamp(length3(x, (y - 2.45 * s) * 1.2, z) / (2.44 * s), 0, 1);
  const spec = hash(floor(x * 5.0), floor(y * 5.0) * 31 + floor(z * 5.0), k.seed);
  return hsv(0.16 + (m - 0.5) * 0.06 + (n - 0.5) * 0.04, 0.62, 0.46 + n * 0.22) * (0.62 + 0.38 * out) * (0.9 + 0.2 * spec);
}
