// A tuft of tall meadow grass gone to seed in autumn: a fountain of long,
// narrow blades arching out from its crown, knee to hip high, green at the
// foot and tawny gold to rust at the tips, a few stems standing up out of it
// with feathery seed heads, swaying.
export const meta = {
  name: "tall grass",
  bounds: [0.6, 1.15, 0.6],
  tags: ["tallgrass", "nonsolid"],
  props: { mass: 0.3, burns: 0.9, alive: 1, fuel: 0.3 },
};

export function sdf(x, y, z, k) {
  // Fold round the tuft into eleven sectors; in each, one blade arches out
  // from the crown, a curve rising and bending over, thin and flat.
  const n = 11.0;
  const turn = hash(k.seed) * 6.2832;
  const a = atan2(z, x) - turn;
  const sec = floor(a * n / 6.2832 + 0.5);
  const b = sec * 6.2832 / n + turn;
  const u = x * cos(b) + z * sin(b);
  const v = z * cos(b) - x * sin(b);
  const h = 0.55 + hash(k.seed, sec) * 0.4;
  const t = clamp(y / h, 0, 1);
  const bend = (0.2 + hash(k.seed, sec, 2) * 0.35) * t * t * h;
  const blade = max(max(abs(u - bend - 0.02) - 0.012, abs(v) - 0.02 * (1.0 - t * 0.85)), max(y - h, -y));
  // An inner ring, more upright, between the outer blades.
  const a2 = a + 3.14159 / n;
  const sec2 = floor(a2 * n / 6.2832 + 0.5);
  const b2 = sec2 * 6.2832 / n + turn - 3.14159 / n;
  const u2 = x * cos(b2) + z * sin(b2);
  const v2 = z * cos(b2) - x * sin(b2);
  const h2 = 0.7 + hash(k.seed, sec2, 5) * 0.35;
  const t2 = clamp(y / h2, 0, 1);
  const blade2 = max(max(abs(u2 - 0.02 - 0.16 * t2 * t2) - 0.01, abs(v2) - 0.014 * (1.0 - t2 * 0.85)), max(y - h2, -y));
  // Three stems with feathery seed heads.
  let seeds = 10.0;
  for (let i = 0; i < 3; i++) {
    const sa = i * 2.1 + hash(k.seed, i, 7) * 1.5;
    const sx = cos(sa) * 0.08;
    const sz = sin(sa) * 0.08;
    const sh = 0.9 + hash(k.seed, i, 8) * 0.2;
    seeds = min(seeds, capsule(x, y, z, 0, 0, 0, sx, sh, sz, 0.006));
    seeds = min(seeds, ellipsoid(x - sx * 1.04, y - sh - 0.05, z - sz * 1.04, 0.025, 0.09, 0.025));
  }
  return min(min(blade, blade2) * 0.6, seeds);
}

export function color(x, y, z, k) {
  if (y > 0.92) return mix(rgb(214, 176, 110), rgb(236, 210, 150), noise3(x * 40, y * 40, z * 40));
  const t = clamp(y / 0.9, 0, 1);
  const tip = mix(rgb(214, 168, 90), rgb(176, 96, 56), k.a);
  return mix(mix(rgb(84, 112, 46), rgb(120, 136, 58), k.b), tip, smoothstep(0.25, 0.85, t)) * (0.85 + 0.15 * noise3(x * 30, y * 10, z * 30));
}
