// A lidded wicker picnic basket with a bent-wood handle over it, a checked
// cloth peeking out under the lid.
export const meta = {
  name: "picnic basket",
  bounds: [0.24, 0.32, 0.17],
  tags: ["item", "container"],
  props: { mass: 2, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const body = roundBox(x, y - 0.1, z, 0.21, 0.1, 0.14, 0.02);
  const lid = roundBox(x, y - 0.215, z, 0.22, 0.015, 0.15, 0.01);
  // The handle: an arch over the lid, its feet on the basket's sides.
  const handle = max(abs(length2(x, y - 0.2) - 0.1) - 0.012, max(abs(z) - 0.012, 0.2 - y));
  return min(min(body, lid), handle);
}

export function color(x, y, z, k) {
  if (y > 0.235) return rgb(150, 110, 70);
  if (abs(y - 0.198) < 0.006) return step(0.5, fract(x * 30.0)) > 0.5 ? rgb(200, 40, 40) : rgb(246, 242, 236);
  return step(0.4, fract(y * 30.0)) + step(0.5, fract(x * 14.0 + z * 14.0)) == 1 ? rgb(150, 110, 64) : rgb(196, 156, 100);
}
