// A stack of wooden fish crates on the landing, slatted and weathered, the
// boat's name stencilled on their ends, the top one full of silver fish on ice.
export const meta = {
  name: "fish crates",
  bounds: [0.75, 0.9, 0.52],
  tags: ["prop"],
  props: { mass: 40, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const s = hash(k.seed) * 0.2;
  const a = max(box(x + 0.36, y - 0.15, z, 0.34, 0.15, 0.24), -box(x + 0.36, y - 0.2, z, 0.31, 0.12, 0.21));
  const b = max(box(x + 0.34, y - 0.45, z + 0.03, 0.34, 0.15, 0.24), -box(x + 0.34, y - 0.5, z + 0.03, 0.31, 0.12, 0.21));
  const c = max(box(x - 0.36, y - 0.15, z - 0.06 + s, 0.34, 0.15, 0.24), -box(x - 0.36, y - 0.2, z - 0.06 + s, 0.31, 0.12, 0.21));
  const ice = box(x + 0.34, y - 0.52, z + 0.03, 0.31, 0.04, 0.21);
  const iceb = box(x + 0.36, y - 0.22, z, 0.31, 0.04, 0.21);
  const fcx = x - 0.1 * floor(x / 0.1 + 0.5);
  const fcz = z - 0.4 * floor(z / 0.4 + 0.5) + (fract(floor(x / 0.1 + 0.5) * 0.5) - 0.25) * 0.06;
  const fbody = ellipsoid(fcx, y - 0.58, fcz, 0.029, 0.021, 0.100);
  const ftail = triangle(fcx, y - 0.58, fcz, 0, 0, -0.085, -0.028, 0, -0.125, 0.028, 0, -0.125, 0.004);
  const fish = min(fbody, ftail);
  const fishes = max(fish, box(x + 0.34, y - 0.58, z + 0.03, 0.3, 0.06, 0.2));
  return min(min(min(a, b), min(c, ice)), min(iceb, fishes));
}

export function color(x, y, z, k) {
  {
    const fcx = x - 0.1 * floor(x / 0.1 + 0.5);
    const fcz = z - 0.4 * floor(z / 0.4 + 0.5) + (fract(floor(x / 0.1 + 0.5) * 0.5) - 0.25) * 0.06;
    if (abs(y - 0.58) < 0.024 && abs(fcx) < 0.035) {
      if (fcz > 0.063 && abs(y - 0.58 - 0.006) < 0.007 && abs(fcx) > 0.018) return rgb(20, 20, 24);
      if (y - 0.58 > 0.008) return mix(rgb(60, 80, 100), rgb(90, 110, 120), hash(floor(x / 0.1 + 0.5), 3));
      if (y - 0.58 > -0.007) return mix(rgb(196, 204, 210), rgb(230, 234, 236), noise3(x * 60, y * 60, z * 60));
      return rgb(240, 240, 236);
    }
  }
  if (y > 0.47 && y < 0.57 && x > 0.02 && abs(z - 0.03) < 0.215) return mix(rgb(220, 232, 236), rgb(250, 252, 252), noise3(x * 40, y * 40, z * 40));
  if (abs(z) > 0.23 && abs(y - 0.15 - 0.3 * floor(y / 0.3)) < 0.04 && fract(x * 12.0) < 0.6) return rgb(60, 70, 100);
  if (fract(y / 0.3 * 3.0) < 0.12) return rgb(90, 70, 50);
  return mix(rgb(150, 132, 104), rgb(180, 160, 128), noise3(x * 6, y * 20, z * 6));
}
