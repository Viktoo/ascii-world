// An artist's easel set up on the sand facing the cove, a half-finished
// painting on it: the sunset over the water, the sky and sun done in reds and
// golds, the sea and the pier still bare pencil on white canvas. A folding
// stool before it with a palette of paint blobs and a jar of brushes. The
// painting faces +z, where the painter sits; beyond it, the view.
export const meta = {
  name: "easel",
  bounds: [0.55, 1.75, 0.95],
  tags: ["furniture", "art", "seat"],
  props: { mass: 8, burns: 0.8 },
  anchors: [
    { kind: "seat", at: [0, 0.48, 0.8], face: 180 },
  ],
};

export function sdf(x, y, z, k) {
  const legs = min(capsule(abs(x), y, z, 0.32, 0.0, 0.1, 0.05, 1.7, 0.0, 0.022), capsule(x, y, z, 0, 0.0, -0.45, 0, 1.6, -0.05, 0.02));
  const ledge = box(x, y - 0.72, z - 0.06, 0.38, 0.02, 0.05);
  const canvas = box(x, y - 1.08, z - 0.02 + (y - 1.08) * 0.08, 0.36, 0.34, 0.012);
  const stool = min(cylinder(x, y - 0.46, z - 0.8, 0.18, 0.025), box(abs(x) - 0.13, y - 0.22, abs(z - 0.8) - 0.0, 0.02, 0.22, 0.12));
  const palette = cylinder(x - 0.18, y - 0.495, z - 0.76, 0.1, 0.008);
  const jar = cappedCone(x + 0.4, y - 0.08, z - 0.5, 0.08, 0.05, 0.045);
  const brushes = capsule(x, y, z, -0.4, 0.1, 0.5, -0.38, 0.32, 0.52, 0.006);
  return min(min(min(legs, ledge), min(canvas, stool)), min(palette, min(jar, max(brushes, -1.0))));
}

export function color(x, y, z, k) {
  // The painting: the upper part done (sky, sun, its path on the water),
  // the lower still pencil lines on white.
  if (abs(x) < 0.355 && abs(y - 1.08) < 0.335 && z > 0.0 && abs(z - 0.02 - (y - 1.08) * 0.08) < 0.02) {
    const u = x / 0.355;
    const v = (y - 1.08) / 0.335;
    if (v > -0.1) {
      const sun = length2(u + 0.15, (v - 0.15) * 1.4);
      if (sun < 0.16) return rgb(255, 236, 170);
      return mix(rgb(250, 170, 80), rgb(200, 90, 110), clamp(v, 0, 1)) * (0.92 + 0.08 * noise3(x * 60, y * 60, 1.0));
    }
    if (v > -0.3 && u < 0.2) return abs(u + 0.15) < 0.08 ? rgb(250, 200, 110) : rgb(60, 80, 120);
    if (abs(v + 0.35) < 0.012 || (abs(v + 0.6) < 0.01 && u > 0.1) || (abs(u - 0.5) < 0.01 && v < -0.35)) return rgb(120, 120, 120);
    return rgb(246, 244, 238);
  }
  if (length2(x - 0.18, z - 0.76) < 0.1 && y > 0.49) return hsv(floor(atan2(z - 0.76, x - 0.18) * 1.6) * 0.17, 0.7, 0.9);
  if (abs(z - 0.8) < 0.2 && y > 0.43 && y < 0.49) return rgb(60, 110, 90);
  if (length2(x + 0.4, z - 0.5) < 0.09 && y < 0.17) return rgb(200, 220, 230);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
