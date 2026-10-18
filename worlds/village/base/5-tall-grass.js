// Tall meadow grass, knee to hip high, its seed heads gone gold, swaying.
export const meta = {
  name: "tall grass",
  bounds: [0.5, 1.0, 0.5],
  tags: ["tallgrass", "nonsolid"],
  props: { mass: 0.3, burns: 0.9, alive: 1, fuel: 0.3 },
};

export function sdf(x, y, z, k) {
  let d = 10;
  for (let i = 0; i < 6; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 0.8;
    const r = 0.04 + hash(k.seed, i, 2) * 0.32;
    const h = 0.55 + hash(k.seed, i, 3) * 0.4;
    const lean = 0.12 + hash(k.seed, i, 4) * 0.12;
    const tx = cos(a) * (r + lean);
    const tz = sin(a) * (r + lean);
    d = min(d, capsule(x, y, z, cos(a) * r * 0.3, 0, sin(a) * r * 0.3, tx, h, tz, 0.012));
    d = min(d, ellipsoid(x - tx, y - h - 0.06, z - tz, 0.025, 0.08, 0.025));
  }
  return d;
}

export function color(x, y, z, k) {
  const top = clamp((y - 0.45) / 0.45, 0, 1);
  return mix(mix(rgb(74, 120, 44), rgb(120, 150, 60), k.b), mix(rgb(214, 188, 104), rgb(184, 150, 80), k.a), top);
}
