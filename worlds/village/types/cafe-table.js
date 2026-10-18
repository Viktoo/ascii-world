// A café table on the piazza: a round marble top on an iron foot, two
// little iron chairs facing each other, and a big striped umbrella over
// them (red and white, or blue and white, or green and cream).
export const meta = {
  name: "cafe table",
  bounds: [1.4, 2.6, 1.4],
  tags: ["furniture", "prop"],
  props: { mass: 40 },
  sound: { hard: 0.8, dry: 0.6, ring: 0.4 },
  anchors: [
    { kind: "seat", at: [-0.72, 0.46, 0], face: 90 },
    { kind: "seat", at: [0.72, 0.46, 0], face: 270 },
  ],
};

export function sdf(x, y, z, k) {
  const top = cylinder(x, y - 0.74, z, 0.38, 0.02);
  const post = cylinder(x, y - 1.25, z, 0.03, 1.25);
  const foot = cylinder(x, y - 0.02, z, 0.24, 0.02);
  const r = length2(x, z);
  const shade = max(abs(y - 2.25 + r * 0.3) - 0.025, r - 1.35);
  const ax = abs(x) - 0.72;
  const seat = cylinder(ax, y - 0.45, z, 0.2, 0.02);
  const legs = cylinder(ax, y - 0.22, z, 0.025, 0.22);
  const back = box(abs(x) - 0.92, y - 0.7, z, 0.02, 0.22, 0.18);
  return min(min(top, post), min(min(foot, shade), min(seat, min(legs, back))));
}

export function color(x, y, z, k) {
  if (y > 1.8) {
    const seg = fract(atan2(z, x) * 1.2732 * 0.5) < 0.5;
    const c = floor(hash(k.seed) * 3);
    const a = c < 1 ? rgb(214, 48, 44) : c < 2 ? rgb(40, 96, 176) : rgb(54, 128, 84);
    return seg ? a : rgb(244, 238, 222);
  }
  if (abs(y - 0.74) < 0.03 && length2(x, z) < 0.4) return rgb(238, 236, 230);
  return rgb(36, 36, 38);
}
