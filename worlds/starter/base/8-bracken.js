// Bracken on the wood floor, its arching fronds turned gold and rust.
export const meta = {
  name: "bracken",
  bounds: [0.8, 0.8, 0.8],
  tags: ["fern", "nonsolid"],
  props: { mass: 0.5, burns: 0.8, alive: 1, fuel: 0.3 },
};

export function sdf(x, y, z, k) {
  // Six fronds round the crown: fold the point into the nearest one's
  // sector and draw that frond alone.
  const n = 6.0;
  const a = atan2(z, x) + hash(k.seed) * 1.0;
  const sec = floor(a * n / 6.2832 + 0.5);
  const b = sec * 6.2832 / n - hash(k.seed) * 1.0;
  const c = cos(b);
  const s = sin(b);
  const qx = x * c + z * s;
  const qz = z * c - x * s;
  const fr = ellipsoid(qx - 0.35, y - 0.42 + qx * qx * 0.9, qz, 0.4, 0.025, 0.13 * clamp(1.15 - qx * 1.3, 0.2, 1.0));
  // Cut into leaflets along the frond, a midrib left down its middle.
  const slit = abs(fract(qx * 14.0) - 0.5) * 0.071 - 0.012;
  const leaflets = max(fr, min(-slit, abs(qz) - 0.008));
  const stalk = capsule(qx, y, qz, 0, 0, 0, 0.1, 0.4, 0, 0.008);
  return min(leaflets * 0.7, stalk);
}

export function color(x, y, z, k) {
  const r = length2(x, z);
  const n = noise3(x * 8, y * 8, z * 8);
  const autumn = mix(rgb(176, 96, 40), rgb(222, 170, 70), k.a);
  return mix(rgb(96, 120, 50), autumn, smoothstep(0.15, 0.45, r + (n - 0.5) * 0.2)) * (0.85 + 0.15 * n);
}
