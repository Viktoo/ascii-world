// Built-in conifer.
export const meta = { name: "pine", bounds: [2.6, 9.5, 2.6], tags: ["builtin", "pine"] };

export function sdf(x, y, z, k) {
  const s = 0.85 + hash(k.seed) * 0.3;
  const trunk = cylinder(x, y - 1.5, z, 0.22, 1.5);
  let d = trunk;
  for (let i = 0; i < 4; i++) {
    const base = 1.6 + i * 1.65 * s;
    const r = (2.3 - i * 0.45) * s;
    d = min(d, cappedCone(x, y - base - 1.1, z, 1.1, r, 0.12));
  }
  return d + (noise3(x * 3, y * 3, z * 3) - 0.5) * 0.12;
}

export function color(x, y, z, k) {
  if (y < 1.7 && length2(x, z) < 0.3) {
    return rgb(76, 54, 40);
  }
  const n = noise3(x * 2.5, y * 2.5, z * 2.5);
  return hsv(0.36 + hash(k.seed, 3) * 0.05, 0.55, 0.24 + n * 0.16 + y * 0.01);
}
