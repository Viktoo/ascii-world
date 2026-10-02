// Built-in flame: drawn over anything that burns. s0 = time (flicker).
export const meta = { name: "flame", bounds: [0.6, 1.0, 0.6], tags: ["builtin", "flame", "nonsolid", "effect"] };

export function sdf(x, y, z, k) {
  const t = k.s0;
  const n = noise3(x * 3.1 + t * 0.7, y * 2.6 - t * 3.6, z * 3.1 + hash(k.seed) * 9);
  const tongue = cappedCone(x + sin(t * 2.3 + y * 2) * 0.06, y - 0.05, z, 0.8, 0.46, 0.03);
  const inner = cappedCone(x, y + 0.35, z, 0.45, 0.3, 0.05);
  return (min(tongue, inner) + (n - 0.5) * 0.22) * 0.75;
}

export function color(x, y, z, k) {
  const t = clamp((y + 0.75) / 1.6, 0, 1);
  const c = clamp(1 - length2(x, z) * 2.2, 0, 1);
  const hot = mix(rgb(255, 120, 30), rgb(255, 236, 150), c * (1 - t));
  return mix(hot, rgb(220, 60, 20), t * t);
}
