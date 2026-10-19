// A shop counter of varnished wood with a worn top, an old brass till and a
// set of scales on it. The shopkeeper stands behind it (-z); customers at its
// front (+z).
export const meta = {
  name: "shop counter",
  bounds: [1.2, 1.2, 0.35],
  tags: ["furniture", "counter", "shop"],
  props: { mass: 120, burns: 0.4 },
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 0.48, z, 1.15, 0.48, 0.3);
  const top = box(x, y - 0.98, z, 1.2, 0.025, 0.34);
  const till = box(x + 0.75, y - 1.12, z + 0.05, 0.18, 0.12, 0.15);
  const scales = min(cylinder(x - 0.6, y - 1.06, z, 0.12, 0.05), cylinder(x - 0.6, y - 1.18, z, 0.03, 0.08));
  return min(min(body, top), min(till, scales));
}

export function color(x, y, z, k) {
  if (y > 1.0 && abs(x - 0.75) < 0.2) return rgb(196, 160, 60);
  if (y > 1.0) return rgb(190, 196, 200);
  if (y > 0.95) return rgb(236, 232, 226);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
