// A woven basket of the morning's catch: silver mackerel and a cod or two
// laid head to tail, a handful of seaweed over them.
export const meta = {
  name: "basket of fish",
  bounds: [0.28, 0.24, 0.28],
  tags: ["item", "food"],
  props: { mass: 6, edible: 0.7, wet: 0.6, burns: 0.4 },
};

export function sdf(x, y, z, k) {
  const basket = max(cappedCone(x, y - 0.1, z, 0.1, 0.2, 0.26), -cappedCone(x, y - 0.14, z, 0.1, 0.17, 0.23));
  const rim = max(abs(length2(x, z) - 0.255) - 0.018, abs(y - 0.2) - 0.018);
  const fcx = x - 0.07 * floor(x / 0.07 + 0.5);
  const fcz = z - 0.22 * floor(z / 0.22 + 0.5) + (fract(floor(x / 0.07 + 0.5) * 0.5) - 0.25) * 0.06;
  const fbody = ellipsoid(fcx, y - 0.18, fcz, 0.034, 0.024, 0.100);
  const ftail = triangle(fcx, y - 0.18, fcz, 0, 0, -0.085, -0.032, 0, -0.125, 0.032, 0, -0.125, 0.005);
  const fish = min(fbody, ftail);
  const fishes = max(fish, length2(x, z) - 0.22);
  const weed = max(ellipsoid(x + 0.08, y - 0.2, z - 0.05, 0.08, 0.02, 0.06), -y + 0.18);
  return min(min(basket, rim), min(fishes, weed));
}

export function color(x, y, z, k) {
  {
    const fcx = x - 0.07 * floor(x / 0.07 + 0.5);
    const fcz = z - 0.22 * floor(z / 0.22 + 0.5) + (fract(floor(x / 0.07 + 0.5) * 0.5) - 0.25) * 0.06;
    if (abs(y - 0.18) < 0.027 && abs(fcx) < 0.040) {
      if (fcz > 0.070 && abs(y - 0.18 - 0.006) < 0.008 && abs(fcx) > 0.020) return rgb(20, 20, 24);
      if (y - 0.18 > 0.010) return mix(rgb(60, 80, 100), rgb(90, 110, 120), hash(floor(x / 0.07 + 0.5), 3));
      if (y - 0.18 > -0.008) return mix(rgb(196, 204, 210), rgb(230, 234, 236), noise3(x * 60, y * 60, z * 60));
      return rgb(240, 240, 236);
    }
  }
  if (y > 0.19 && length2(x + 0.08, z - 0.05) < 0.08) return rgb(60, 90, 40);
  if (abs(length2(x, z) - 0.255) < 0.025 && y > 0.18) return rgb(120, 86, 50);
  return step(0.4, fract(y * 30.0)) + step(0.5, fract(atan2(z, x) * 6.0)) == 1 ? rgb(130, 96, 56) : rgb(176, 136, 80);
}
