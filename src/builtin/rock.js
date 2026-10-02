// Built-in boulder.
export const meta = { name: "boulder", bounds: [1.8, 1.4, 1.8], tags: ["builtin", "rock"], props: { mass: 3500, bounce: 0.1 } };

export function sdf(x, y, z, k) {
  const q = rotY(x, y, z, hash(k.seed) * 6.28);
  const sx = 1.0 + hash(k.seed, 1) * 0.35;
  const base = ellipsoid(q.x, q.y - 0.2, q.z, 1.2 * sx, 0.75, 1.0);
  const n = noise3(x * 1.7 + k.seed * 13, y * 1.7, z * 1.7) - 0.5;
  return (base + n * 0.32) * 0.8;
}

export function color(x, y, z, k) {
  const n = noise3(x * 3, y * 3, z * 3);
  const g = 104 + n * 60 + hash(k.seed, 4) * 20;
  const moss = clamp((y - 0.3) * 1.5, 0, 1) * clamp((noise3(x * 1.3, y, z * 1.3) - 0.45) * 4, 0, 1);
  return mix(rgb(g, g * 0.97, g * 0.92), rgb(84, 110, 60), moss);
}
