// Built-in shrub, sometimes flowering.
export const meta = { name: "shrub", bounds: [1.5, 1.4, 1.5], tags: ["builtin", "bush"] };

export function sdf(x, y, z, k) {
  const a = sphere(x, y - 0.45, z, 0.75);
  const b = sphere(x - 0.45, y - 0.35, z + 0.2, 0.55);
  const c = sphere(x + 0.35, y - 0.4, z - 0.35, 0.55);
  const d = smoothUnion(smoothUnion(a, b, 0.3), c, 0.3);
  return (d + (noise3(x * 3, y * 3, z * 3) - 0.5) * 0.25) * 0.8;
}

export function color(x, y, z, k) {
  const n = noise3(x * 4, y * 4, z * 4);
  if (hash(k.seed) > 0.55 && n > 0.72) {
    return hsv(hash(k.seed, 9), 0.6, 0.9);
  }
  return hsv(0.27 + hash(k.seed, 2) * 0.05, 0.6, 0.28 + n * 0.2);
}
