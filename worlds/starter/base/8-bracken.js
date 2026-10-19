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
  const fr = ellipsoid(qx - 0.35, y - 0.42 + qx * qx * 0.9, qz, 0.4, 0.04, 0.12);
  return fr * 0.7;
}

export function color(x, y, z, k) {
  return mix(mix(rgb(176, 110, 46), rgb(214, 170, 70), k.a), rgb(140, 120, 56), noise3(x * 8, y * 8, z * 8) * 0.6);
}
