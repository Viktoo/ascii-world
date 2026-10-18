// Washing hung out to dry: two thin iron poles 4 m apart with a line
// between them, shirts, sheets and towels in bright colours pegged along
// it. The line runs along x; its middle hangs lowest.
export const meta = {
  name: "laundry line",
  bounds: [2.1, 2.4, 0.12],
  tags: ["prop"],
  props: { mass: 20, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const poles = cylinder(abs(x) - 2.0, y - 1.15, z, 0.035, 1.15);
  const sag = 0.15 * (1 - (x / 2.0) * (x / 2.0));
  const line = max(length2(y - 2.2 + sag, z) - 0.025, abs(x) - 2.0);
  const cx = x - 0.5 * floor(x / 0.5 + 0.5);
  const len = 0.45 + hash(floor(x / 0.5 + 0.5), k.seed) * 0.5;
  const cloth = max(box(cx, y - 2.2 + sag + len * 0.5, z, 0.2, len * 0.5, 0.03), abs(x) - 1.75);
  return min(poles, min(line, cloth));
}

export function color(x, y, z, k) {
  if (abs(abs(x) - 2.0) < 0.06) return rgb(40, 40, 42);
  const i = floor(x / 0.5 + 0.5);
  const h = hash(i, k.seed);
  if (h < 0.3) return rgb(248, 248, 244);
  return hsv(fract(h * 3.7), 0.6, 0.92);
}
