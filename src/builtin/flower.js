// Built-in wildflowers: a clump of stems and blossoms (walk-through). A field
// shares its colour (k.a, the same across a stand), with a few strays.
export const meta = { name: "wildflowers", bounds: [0.5, 0.5, 0.5], tags: ["builtin", "flower", "nonsolid"], props: { mass: 0.1, burns: 0.8, alive: 1, fuel: 0.2 } };

export function sdf(x, y, z, k) {
  let d = 10;
  for (let i = 0; i < 7; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 0.6;
    const r = 0.06 + hash(k.seed, i, 2) * 0.3;
    const h = 0.2 + hash(k.seed, i, 3) * 0.2;
    const bx = cos(a) * r;
    const bz = sin(a) * r;
    d = min(d, capsule(x, y, z, bx * 0.4, 0, bz * 0.4, bx, h, bz, 0.016));
    d = min(d, sphere(x - bx, y - h, z - bz, 0.04 + hash(k.seed, i, 4) * 0.025));
  }
  return d;
}

export function color(x, y, z, k) {
  let bloom = 10;
  for (let i = 0; i < 7; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 0.6;
    const r = 0.06 + hash(k.seed, i, 2) * 0.3;
    const h = 0.2 + hash(k.seed, i, 3) * 0.2;
    bloom = min(bloom, length3(x - cos(a) * r, y - h, z - sin(a) * r));
  }
  if (bloom > 0.075) return mix(rgb(52, 92, 40), rgb(96, 140, 60), clamp(y / 0.35, 0, 1));
  let f = k.a;
  if (hash(k.seed, 9) < 0.12) f = hash(k.seed, 10);
  let c = rgb(240, 196, 36);
  if (f > 0.2) c = rgb(246, 244, 232);
  if (f > 0.4) c = rgb(150, 86, 204);
  if (f > 0.6) c = rgb(214, 40, 36);
  if (f > 0.8) c = rgb(86, 120, 232);
  if (bloom < 0.022) return rgb(250, 210, 60);
  return c * (0.9 + hash(k.seed, 11) * 0.2);
}
