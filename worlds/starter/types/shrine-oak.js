// The great oak of Pine Hollow, the woodcutters' shrine: an ancient oak
// with a huge gnarled trunk and a wide crown, never to be cut. From its low
// branches hang paper lanterns that glow at night and coloured ribbons;
// at its foot a little stone niche holds candles and offerings of
// acorns and flowers.
export const meta = {
  name: "shrine oak",
  bounds: [9.0, 13.0, 9.0],
  tags: ["tree", "landmark", "shrine"],
  props: { mass: 100000, alive: 1, burns: 0.3, light: 1 },
  anchors: [{ kind: "light", at: [0, 3.0, 3.2] }, { kind: "light", at: [0, 0.6, 1.8] }],
};

export function sdf(x, y, z, k) {
  const trunk = cone(x, y, z, 0, -0.3, 0, 0.3, 5.0, -0.2, 1.4, 0.8) + (noise3(x * 1.5, y * 1.5, z * 1.5) - 0.5) * 0.3;
  const roots = cone(abs(x), y, abs(z), 0, 0.8, 0, 1.6, -0.2, 1.4, 0.45, 0.15);
  const b1 = cone(x, y, z, 0, 4.0, 0, 4.6, 6.6, 1.0, 0.6, 0.2);
  const b2 = cone(x, y, z, 0, 4.2, 0, -4.2, 7.0, 1.8, 0.6, 0.2);
  const b3 = cone(x, y, z, 0, 4.4, 0, 0.6, 7.2, -4.4, 0.55, 0.2);
  const b4 = cone(x, y, z, 0, 3.4, 0, 1.0, 4.6, 4.2, 0.45, 0.16);
  let crown = 100.0;
  if (y > 5.0) {
    crown = min(min(ellipsoid(x, y - 9.0, z, 8.0, 3.2, 7.6), ellipsoid(x - 4.0, y - 8.0, z - 1.0, 4.4, 2.6, 4.4)), ellipsoid(x + 3.6, y - 8.4, z + 2.0, 4.8, 2.8, 4.6));
    crown = (crown + (noise3(x * 0.8, y * 0.8, z * 0.8) - 0.5) * 1.2) * 0.6;
  }
  // Lanterns hanging under the crown.
  const lx = x - 2.4 * floor(x / 2.4 + 0.5);
  const lz = z - 2.4 * floor(z / 2.4 + 0.5);
  const lanterns = max(ellipsoid(lx, y - 4.7, lz, 0.18, 0.24, 0.18), max(length2(x, z) - 5.5, 1.6 - length2(x, z)));
  const niche = max(box(x, y - 0.55, z - 1.55, 0.5, 0.55, 0.4), -box(x, y - 0.6, z - 1.75, 0.32, 0.35, 0.3));
  return min(min(min(trunk * 0.8, roots), min(b1, b2)), min(min(b3, b4), min(crown, min(lanterns, niche))));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.2, y * 1.2, z * 1.2);
  if (y > 4.45 && y < 4.95 && length2(x, z) > 1.6 && length2(x, z) < 5.6) {
    const c = floor(x / 2.4 + 0.5) + floor(z / 2.4 + 0.5) * 3;
    return glow(hsv(fract(c * 0.17), 0.7, 1.0), 0.9);
  }
  if (y > 6.0 && length2(x, z) > 1.0) return mix(rgb(132, 52, 30), rgb(196, 110, 44), noise3(x * 2, y * 2, z * 2));
  if (z > 1.1 && abs(x) < 0.55 && y < 1.15) {
    if (y > 0.25 && y < 0.5 && abs(x) < 0.25) return glow(rgb(255, 210, 120));
    return rgb(170, 162, 146);
  }
  if (y > 2.2 && y < 3.8 && abs(sin(atan2(z, x) * 3.0 + y * 1.5)) < 0.12) return hsv(fract(floor(y * 4.0) * 0.23), 0.8, 0.95);
  return mix(rgb(84, 70, 56), rgb(112, 98, 80), n);
}
