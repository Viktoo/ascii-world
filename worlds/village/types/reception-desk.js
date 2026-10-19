// The hotel's reception desk: dark wood with a brass bell and a guest book on
// it, a board of room keys on hooks behind. The clerk stands behind it (-z).
export const meta = {
  name: "reception desk",
  bounds: [1.25, 2.0, 0.6],
  tags: ["furniture", "counter"],
  props: { mass: 150, burns: 0.4 },
};

export function sdf(x, y, z, k) {
  const desk = box(x, y - 0.55, z, 1.2, 0.55, 0.3);
  const top = box(x, y - 1.11, z, 1.25, 0.025, 0.35);
  const bell = min(sphere(x - 0.6, y - 1.18, z, 0.06), cylinder(x - 0.6, y - 1.14, z, 0.07, 0.01));
  const book = box(x + 0.2, y - 1.15, z, 0.2, 0.02, 0.14);
  const keys = box(x, y - 1.6, z + 0.58, 0.6, 0.35, 0.02);
  return min(min(desk, top), min(min(bell, book), keys));
}

export function color(x, y, z, k) {
  if (z < -0.5) return fract(x * 6.0) < 0.15 && fract(y * 5.0) < 0.3 ? rgb(200, 160, 60) : rgb(90, 60, 40);
  if (y > 1.12 && x < -0.4) return rgb(210, 170, 70);
  if (y > 1.12) return rgb(140, 30, 40);
  return mix(rgb(84, 56, 38), rgb(104, 72, 48), noise3(x * 6, y * 18, z * 6));
}
