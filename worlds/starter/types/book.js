// A cloth-bound book of stories, its corners worn, gold letters on its spine
// and a red ribbon marking the page.
export const meta = {
  name: "book",
  bounds: [0.12, 0.035, 0.09],
  tags: ["item"],
  props: { mass: 0.5, burns: 0.9 },
};

export function sdf(x, y, z, k) {
  const cover = max(box(x, y - 0.0275, z, 0.11, 0.0275, 0.085), -box(x + 0.012, y - 0.0275, z, 0.1, 0.02, 0.08));
  const pages = box(x + 0.006, y - 0.0275, z, 0.1, 0.019, 0.078);
  const spine = capsule(x, y, z, -0.108, 0.0275, -0.085, -0.108, 0.0275, 0.085, 0.026);
  const ribbon = box(x - 0.04, y - 0.01, z - 0.09, 0.006, 0.01, 0.012);
  return min(min(cover, pages), min(max(spine, x + 0.098), ribbon));
}

export function color(x, y, z, k) {
  if (abs(x - 0.04 + 0.08) < 0.008 && z > 0.07) return rgb(180, 30, 30);
  if (x > -0.1 && abs(y - 0.0275) < 0.019 && (abs(z) > 0.076 || x > 0.1)) return fract(y * 300.0) < 0.5 ? rgb(236, 228, 206) : rgb(214, 204, 180);
  if (x < -0.09 && abs(abs(z) - 0.04) < 0.008) return rgb(210, 176, 80);
  return hsv(hash(k.seed), 0.55, 0.42) * (0.9 + 0.1 * noise3(x * 50, y * 50, z * 50));
}
