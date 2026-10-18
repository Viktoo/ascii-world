// A wicker basket of Rosa's bread: golden round loaves and a long
// focaccia, still warm, under a checked cloth folded back.
export const meta = {
  name: "basket of bread",
  bounds: [0.28, 0.26, 0.28],
  tags: ["item", "food"],
  props: { mass: 3, edible: 0.6, burns: 0.5 },
};

export function sdf(x, y, z, k) {
  const basket = max(cappedCone(x, y - 0.1, z, 0.1, 0.2, 0.26), -cappedCone(x, y - 0.14, z, 0.1, 0.17, 0.23));
  const loaves = min(ellipsoid(x - 0.08, y - 0.19, z, 0.12, 0.07, 0.12), ellipsoid(x + 0.1, y - 0.18, z - 0.05, 0.11, 0.06, 0.11));
  return min(basket, loaves);
}

export function color(x, y, z, k) {
  if (y > 0.15 && length2(x, z) < 0.22) return mix(rgb(196, 130, 60), rgb(232, 180, 100), noise3(x * 12, y * 12, z * 12));
  if (y > 0.17) return step(0.5, fract(x * 10.0)) + step(0.5, fract(z * 10.0)) == 1 ? rgb(200, 40, 40) : rgb(244, 240, 230);
  return fract(y * 30.0) < 0.4 ? rgb(130, 96, 56) : rgb(176, 136, 80);
}
