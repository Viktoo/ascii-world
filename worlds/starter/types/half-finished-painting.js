// A half-finished painting on canvas of the sunset over the cove: the sky,
// the sun and its path on the water done in reds and golds, the sea and the
// pier still bare pencil lines on white. It leans back a little, as on an
// easel; its painted face is +z.
export const meta = {
  name: "half-finished painting",
  bounds: [0.38, 0.7, 0.06],
  tags: ["item", "art"],
  props: { mass: 0.6, burns: 0.9, fragile: 0.3 },
};

export function sdf(x, y, z, k) {
  return box(x, y - 0.34, z + y * 0.0588, 0.36, 0.34, 0.012);
}

export function color(x, y, z, k) {
  if (z + y * 0.0588 < 0.004) return mix(rgb(220, 210, 186), rgb(236, 228, 206), noise3(x * 30, y * 30, 1.0));
  const u = x / 0.355;
  const v = (y - 0.34) / 0.335;
  if (abs(u) > 0.97 || abs(v) > 0.97) return rgb(150, 112, 70);
  if (v > -0.1) {
    const sun = length2(u + 0.15, (v - 0.15) * 1.4);
    if (sun < 0.16) return rgb(255, 236, 170);
    return mix(rgb(250, 170, 80), rgb(200, 90, 110), clamp(v, 0, 1)) * (0.92 + 0.08 * noise3(x * 60, y * 60, 1.0));
  }
  if (v > -0.3 && u < 0.2) return abs(u + 0.15) < 0.08 ? rgb(250, 200, 110) : rgb(60, 80, 120);
  if (abs(v + 0.35) < 0.012 || (abs(v + 0.6) < 0.01 && u > 0.1) || (abs(u - 0.5) < 0.01 && v < -0.35)) return rgb(120, 120, 120);
  return rgb(246, 244, 238);
}
