// A small bathroom against a wall: a white toilet, a washbasin under a round
// mirror and a towel on a ring, blue tiles behind.
export const meta = {
  name: "bathroom",
  bounds: [0.85, 1.6, 0.4],
  tags: ["furniture", "bathroom"],
  props: { mass: 120, wet: 0.2 },
};

export function sdf(x, y, z, k) {
  const bowl = roundBox(x + 0.4, y - 0.22, z - 0.05, 0.18, 0.22, 0.24, 0.08);
  const tank = box(x + 0.4, y - 0.62, z + 0.28, 0.2, 0.2, 0.08);
  const basin = roundBox(x - 0.35, y - 0.8, z - 0.05, 0.26, 0.07, 0.2, 0.05);
  const pedestal = cylinder(x - 0.35, y - 0.4, z + 0.05, 0.08, 0.4);
  const mirror = cylinder(x + 0.35, z - 0.36, y - 1.35, 0.25, 0.015);
  const tiles = box(x, y - 0.75, z + 0.38, 0.85, 0.75, 0.02);
  return min(min(bowl, tank), min(min(basin, pedestal), min(box(x + 0.35, y - 1.35, z - 0.36, 0.25, 0.25, 0.015) * 0.0 + length2(x + 0.35, y - 1.35) * 0.0 + max(length2(x + 0.35, y - 1.35) - 0.25, abs(z - 0.355) - 0.015), tiles)));
}

export function color(x, y, z, k) {
  if (z > 0.34 && length2(x + 0.35, y - 1.35) < 0.25) return glow(rgb(200, 220, 230), 0.15);
  if (z > 0.35) return (fract(x * 6.0) < 0.06 || fract(y * 6.0) < 0.06) ? rgb(220, 230, 236) : rgb(80, 140, 190);
  return rgb(246, 246, 244);
}
