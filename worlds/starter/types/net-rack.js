// Fishing nets hung to dry on the quay: two poles and a crossbar with a
// great green-brown net draped over it, cork floats along its edge, more
// of it heaped on the stones below.
export const meta = {
  name: "net rack",
  bounds: [2.1, 2.2, 0.8],
  tags: ["prop"],
  props: { mass: 60, burns: 0.7 },
  sound: { hard: 0.2, dry: 0.7, ring: 0.0 },
};

export function sdf(x, y, z, k) {
  const poles = cylinder(abs(x) - 1.9, y - 1.05, z, 0.06, 1.05);
  const bar = capsule(x, y, z, -1.95, 2.05, 0, 1.95, 2.05, 0, 0.05);
  const sag = 0.25 * (1 - (x / 1.8) * (x / 1.8));
  const net = max(max(abs(z) - 0.06 - (2.0 - y) * 0.12, abs(x) - 1.8), max(y - 2.0, 0.55 + sag - y));
  const heap = ellipsoid(x + 0.6, y, z - 0.35, 0.8, 0.3, 0.4);
  return min(min(poles, bar), min(net * 0.8, heap));
}

export function color(x, y, z, k) {
  if (abs(abs(x) - 1.9) < 0.08 || y > 2.0) return rgb(120, 92, 62);
  const sag = 0.25 * (1 - (x / 1.8) * (x / 1.8));
  if (y < 0.62 + sag && y > 0.4 && fract(x * 2.5) < 0.3) return rgb(236, 170, 60);
  const mesh = fract(x * 7 + y * 7) < 0.3 || fract(x * 7 - y * 7) < 0.3;
  const g = floor(hash(k.seed) * 2) < 1 ? rgb(70, 110, 80) : rgb(150, 90, 60);
  return mesh ? g * 0.6 : g;
}
