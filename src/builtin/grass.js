// Built-in grass tuft (walk-through).
export const meta = { name: "grass tuft", bounds: [0.45, 0.6, 0.45], tags: ["builtin", "grass", "nonsolid"], props: { mass: 0.2, burns: 0.9, alive: 1, fuel: 0.4 } };

export function sdf(x, y, z, k) {
  let d = 10;
  for (let i = 0; i < 5; i++) {
    const a = i * 1.257 + k.seed * 3;
    const r = 0.12 + hash(k.seed, i) * 0.14;
    const bx = cos(a) * r;
    const bz = sin(a) * r;
    d = min(d, capsule(x, y, z, bx * 0.3, 0, bz * 0.3, bx, 0.45, bz, 0.03));
  }
  return d;
}

export function color(x, y, z, k) {
  return mix(rgb(70, 110, 46), rgb(150, 176, 82), clamp(y / 0.5, 0, 1));
}
