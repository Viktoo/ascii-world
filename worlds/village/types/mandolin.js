// A mandolin: a round-backed body of striped maple and rosewood, a spruce
// top with a round sound hole, a short neck and its head. Its strings run
// along y.
export const meta = {
  name: "mandolin",
  bounds: [0.16, 0.34, 0.1],
  tags: ["item", "instrument"],
  props: { mass: 0.7, burns: 0.6, fragile: 0.6 },
};

export function sdf(x, y, z, k) {
  const body = max(ellipsoid(x, y + 0.15, z, 0.15, 0.18, 0.09), -z - 0.0 - 0.09 + 0.09);
  const neck = box(x, y - 0.12, z - 0.02, 0.022, 0.14, 0.012);
  const head = box(x, y - 0.29, z - 0.02, 0.035, 0.04, 0.01);
  return min(body, min(neck, head));
}

export function color(x, y, z, k) {
  if (z > 0.0 && y < 0.0 && length2(x, y + 0.08) < 0.035) return rgb(30, 22, 16);
  if (z > 0.0 && y < 0.0) return rgb(226, 196, 140);
  if (y > 0.0) return rgb(60, 40, 28);
  return fract(x * 25.0) < 0.5 ? rgb(130, 70, 40) : rgb(190, 130, 80);
}
