// A well-worn fiddle of honey-coloured maple: a waisted body with two f-holes,
// a dark fingerboard and four strings over its bridge, a scroll at the head,
// its bow beside it. For the dances at the tavern.
export const meta = {
  name: "fiddle",
  bounds: [0.13, 0.07, 0.34],
  tags: ["item", "instrument"],
  props: { mass: 0.5, fragile: 0.6, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const lower = ellipsoid(x, y - 0.035, z + 0.1, 0.105, 0.032, 0.095);
  const upper = ellipsoid(x, y - 0.035, z - 0.05, 0.085, 0.032, 0.08);
  const waist = ellipsoid(abs(x) - 0.105, y - 0.035, z - 0.025, 0.035, 0.05, 0.04);
  const body = smoothSubtract(smoothUnion(lower, upper, 0.03), waist, 0.01);
  const neck = box(x, y - 0.055, z + 0.19, 0.016, 0.012, 0.08);
  const scroll = max(abs(length2(x, z + 0.3) - 0.018) - 0.008, abs(y - 0.06) - 0.015);
  const pegs = box(x, y - 0.06, z + 0.265, 0.03, 0.006, 0.006);
  const bridge = box(x, y - 0.075, z - 0.06 + 0.08, 0.04, 0.008, 0.004);
  const bow = capsule(x, y, z, 0.12, 0.012, -0.32, 0.12, 0.012, 0.3, 0.006);
  return min(min(min(body, neck), min(scroll, pegs)), min(bridge, bow));
}

export function color(x, y, z, k) {
  if (x > 0.108) return abs(z) > 0.28 ? rgb(40, 30, 26) : rgb(236, 230, 210);
  if (z < -0.11 || (y > 0.064 && abs(x) < 0.018 && z < 0.07)) return z < -0.26 ? rgb(150, 70, 30) : rgb(30, 26, 24);
  if (y > 0.06 && abs(x) < 0.02 && abs(fract(x * 80.0) - 0.5) < 0.12) return rgb(220, 220, 210);
  if (y > 0.06 && abs(abs(x) - 0.045) < 0.006 && abs(z - 0.03) < 0.035) return rgb(30, 20, 16);
  if (abs(z - 0.02) < 0.006 && y > 0.065) return rgb(236, 214, 170);
  return mix(rgb(150, 70, 26), rgb(206, 120, 44), clamp(1.0 - length2(x * 1.4, z - 0.03) * 6.0, 0, 1)) * (0.92 + 0.08 * noise3(x * 30, y * 30, z * 8));
}
