// The fish counter in the fish shed: a sloping bed of crushed ice on a stand
// of weathered boards, laid with rows of silver mackerel and cod, two red
// lobsters and a heap of clams at one end, a slate with the day's prices.
export const meta = {
  name: "fish counter",
  bounds: [1.15, 1.25, 0.45],
  tags: ["furniture", "counter", "shop"],
  props: { mass: 140, wet: 0.5 },
};

export function sdf(x, y, z, k) {
  const stand = box(x, y - 0.4, z, 1.1, 0.4, 0.4);
  const tray = max(box(x, y - 0.84, z, 1.12, 0.06, 0.42), -box(x, y - 0.9, z, 1.06, 0.06, 0.36));
  const ice = box(x, y - 0.83, z, 1.06, 0.04, 0.36) + (noise3(x * 30, y * 30, z * 30) - 0.5) * 0.01;
  const fcx = x - 0.14 * floor(x / 0.14 + 0.5);
  const fcz = z - 0.32 * floor(z / 0.32 + 0.5) + (fract(floor(x / 0.14 + 0.5) * 0.5) - 0.25) * 0.06;
  const fbody = ellipsoid(fcx, y - 0.9, fcz, 0.032, 0.022, 0.100);
  const ftail = triangle(fcx, y - 0.9, fcz, 0, 0, -0.085, -0.030, 0, -0.125, 0.030, 0, -0.125, 0.004);
  const fish = min(fbody, ftail);
  const fishes = max(fish, max(max(x - 0.55, -x - 1.0), abs(z) - 0.34));
  const lob = ellipsoid(x - 0.68, y - 0.9, abs(z) - 0.16, 0.06, 0.04, 0.14);
  const clams = max(ellipsoid(x - 0.95, y - 0.88, z, 0.12, 0.06, 0.3) + (noise3(x * 30, y * 30, z * 30) - 0.5) * 0.03, -y + 0.86);
  const slate = box(x + 0.9, y - 1.05, z + 0.38, 0.18, 0.13, 0.01);
  return min(min(min(stand, tray), min(ice, fishes)), min(min(lob, clams), slate));
}

export function color(x, y, z, k) {
  {
    const fcx = x - 0.14 * floor(x / 0.14 + 0.5);
    const fcz = z - 0.32 * floor(z / 0.32 + 0.5) + (fract(floor(x / 0.14 + 0.5) * 0.5) - 0.25) * 0.06;
    if (abs(y - 0.9) < 0.026 && abs(fcx) < 0.037) {
      if (fcz > 0.070 && abs(y - 0.9 - 0.006) < 0.007 && abs(fcx) > 0.019) return rgb(20, 20, 24);
      if (y - 0.9 > 0.009) return mix(rgb(60, 80, 100), rgb(90, 110, 120), hash(floor(x / 0.14 + 0.5), 3));
      if (y - 0.9 > -0.007) return mix(rgb(196, 204, 210), rgb(230, 234, 236), noise3(x * 60, y * 60, z * 60));
      return rgb(240, 240, 236);
    }
  }
  if (abs(x + 0.9) < 0.18 && y > 0.92) return abs(y - 1.05) < 0.08 && fract(x * 30.0) < 0.5 && fract(y * 40.0) < 0.4 ? rgb(230, 230, 220) : rgb(50, 54, 58);
  if (abs(x - 0.68) < 0.09 && y > 0.86) return rgb(200, 50, 36);
  if (x > 0.83 && y > 0.86) return noise3(x * 40, y * 40, z * 40) > 0.5 ? rgb(170, 160, 150) : rgb(110, 104, 100);
  if (y > 0.8 && abs(x) < 1.07 && abs(z) < 0.37) return mix(rgb(220, 232, 236), rgb(250, 252, 252), noise3(x * 40, y * 40, z * 40));
  if (y > 0.78) return rgb(120, 116, 110);
  return (fract(x * 4.0) < 0.05 ? 0.75 : 1.0) * mix(rgb(140, 132, 120), rgb(168, 160, 146), noise3(x * 3, y * 20, z * 3));
}
