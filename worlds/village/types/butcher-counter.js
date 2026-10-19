// The butcher's counter: a white-tiled counter with a glass front full of
// steaks, sausages and chops, a wooden block and cleaver on top, hams and
// salami hanging from a rail above.
export const meta = {
  name: "butcher counter",
  bounds: [1.15, 2.1, 0.4],
  tags: ["furniture", "counter", "shop"],
  props: { mass: 160 },
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 0.45, z, 1.1, 0.45, 0.36);
  const block = box(x + 0.6, y - 1.0, z, 0.3, 0.08, 0.25);
  const meats = max(box(x - 0.2 * floor(x / 0.2 + 0.5), y - 0.94, z, 0.08, 0.04, 0.24), abs(x) - 0.4);
  const rail = capsule(x, y, z, -1.0, 2.0, -0.2, 1.0, 2.0, -0.2, 0.02);
  const hams = max(ellipsoid(x - 0.4 * floor(x / 0.4 + 0.5), y - 1.65, z + 0.2, 0.1, 0.22, 0.1), abs(x) - 1.0);
  return min(min(body, block), min(meats, min(rail, hams)));
}

export function color(x, y, z, k) {
  if (y > 1.4) return y > 1.95 ? rgb(120, 120, 120) : mix(rgb(150, 60, 40), rgb(190, 110, 80), noise3(x * 10, y * 10, z * 10));
  if (y > 0.98 && x > 0.3) return rgb(160, 120, 80);
  if (y > 0.89) return mix(rgb(170, 40, 40), rgb(220, 120, 110), noise3(x * 20, y * 20, z * 20));
  return (fract(x * 6.0) < 0.05 || fract(y * 6.0) < 0.05) ? rgb(200, 200, 200) : rgb(246, 246, 242);
}
