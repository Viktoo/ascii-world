// An accordion with mother-of-pearl buttons and red bellows.
export const meta = {
  name: "accordion",
  bounds: [0.25, 0.42, 0.12],
  tags: ["item", "instrument"],
  props: { mass: 6, fragile: 0.4 },
};

export function sdf(x, y, z, k) {
  return min(box(x, y - 0.2, z, 0.08 + 0.0, 0.2, 0.1), min(box(abs(x) - 0.16, y - 0.2, z, 0.06, 0.2, 0.11), box(x, y - 0.2, z, 0.12, 0.19, 0.1)));
}

export function color(x, y, z, k) {
  if (abs(x) < 0.11) return fract(x * 40.0) < 0.5 ? rgb(170, 30, 40) : rgb(240, 230, 230);
  return fract(y * 12.0) < 0.15 ? rgb(240, 236, 230) : rgb(30, 30, 34);
}
