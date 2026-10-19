// Three oak barrels on their sides on a rack, iron hoops round them and a
// brass spigot in each head, chalked AMBER, STOUT and CIDER: the tavern's
// ale, its dark stout and the autumn cider.
export const meta = {
  name: "beer barrels",
  bounds: [1.3, 1.2, 0.52],
  tags: ["furniture", "prop"],
  props: { mass: 300, burns: 0.5, fuel: 0.4 },
};

export function sdf(x, y, z, k) {
  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  const bulge = 0.38 + 0.05 * (1.0 - (z / 0.45) * (z / 0.45));
  const barrels = max(max(length2(bx, y - 0.62) - bulge, abs(z) - 0.45), abs(x) - 1.25);
  const heads = max(max(length2(bx, y - 0.62) - 0.34, abs(z - 0.43) - 0.02), abs(x) - 1.25);
  const spigots = max(min(capsule(bx, y, z, 0, 0.5, 0.44, 0, 0.5, 0.56, 0.025), capsule(bx, y, z, 0, 0.5, 0.56, 0, 0.44, 0.58, 0.02)), abs(x) - 1.25);
  const rack = min(box(x, y - 0.1, abs(z) - 0.3, 1.25, 0.1, 0.06), box(abs(x - 0.85 * floor(x / 0.85 + 0.5)) - 0.3, y - 0.2, z, 0.06, 0.06, 0.4));
  return min(min(barrels, heads), min(spigots, rack));
}

export function color(x, y, z, k) {
  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  const r = length2(bx, y - 0.62);
  if (z > 0.43 && r < 0.35) {
    if (abs(y - 0.5) < 0.04 && abs(bx) < 0.04) return rgb(200, 160, 60);
    if (abs(y - 0.72) < 0.05 && abs(bx) < 0.2) return rgb(236, 232, 220);
    return (fract(bx * 9.0) < 0.08 ? 0.7 : 1.0) * mix(rgb(150, 108, 66), rgb(176, 130, 80), noise3(bx * 8, y * 8, 1.0));
  }
  if (z > 0.44 && y < 0.6) return rgb(200, 160, 60);
  if (abs(abs(z) - 0.12) < 0.03 || abs(abs(z) - 0.38) < 0.025) return rgb(60, 60, 62);
  return (fract(atan2(y - 0.62, bx) * 3.2) < 0.06 ? 0.75 : 1.0) * mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
