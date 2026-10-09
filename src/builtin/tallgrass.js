// Built-in tall grass: a sheaf of long leaning blades with seed heads
// (walk-through). A meadow shares its ripeness (k.a): lush green to straw.
export const meta = { name: "tall grass", bounds: [0.62, 1.3, 0.62], tags: ["builtin", "tallgrass", "nonsolid"], props: { mass: 0.5, burns: 0.95, alive: 1, fuel: 0.5 } };

export function sdf(x, y, z, k) {
  let d = 10;
  for (let i = 0; i < 9; i++) {
    const a = i * 2.39996 + k.seed;
    const r = 0.04 + hash(k.seed, i) * 0.16;
    const lean = 0.1 + hash(k.seed, i, 2) * 0.25;
    const h = 0.7 + hash(k.seed, i, 3) * 0.45;
    const tx = cos(a) * (r + lean);
    const tz = sin(a) * (r + lean);
    d = min(d, capsule(x, y, z, cos(a) * r, 0, sin(a) * r, tx, h, tz, 0.022));
    d = min(d, ellipsoid(x - tx, y - h, z - tz, 0.03, 0.07, 0.03));
  }
  return d;
}

export function color(x, y, z, k) {
  const ripe = clamp(k.a * 1.3 - 0.2 + (hash(k.seed, 12) - 0.5) * 0.2, 0, 1);
  const base = mix(rgb(56, 98, 42), rgb(120, 112, 60), ripe);
  const tip = mix(rgb(132, 168, 72), rgb(222, 196, 120), ripe);
  return mix(base, tip, clamp(y / 1.0, 0, 1));
}
