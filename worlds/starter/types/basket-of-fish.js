// A wicker basket of the morning's catch: silver sardines and a few red
// mullet heaped in it.
export const meta = {
  name: "basket of fish",
  bounds: [0.28, 0.24, 0.28],
  tags: ["item", "food"],
  props: { mass: 5, edible: 0.5, burns: 0.4 },
};

export function sdf(x, y, z, k) {
  const basket = max(cappedCone(x, y - 0.1, z, 0.1, 0.2, 0.26), -cappedCone(x, y - 0.14, z, 0.1, 0.17, 0.23));
  const fish = max(ellipsoid(x, y - 0.18, z, 0.23, 0.06, 0.23), -y + 0.1);
  return min(basket, fish);
}

export function color(x, y, z, k) {
  if (y > 0.18 && length2(x, z) < 0.23) return noise3(x * 20, 0, z * 20) > 0.7 ? rgb(214, 80, 60) : rgb(196, 206, 214);
  return fract(y * 30.0) < 0.4 ? rgb(130, 96, 56) : rgb(176, 136, 80);
}
