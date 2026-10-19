// A scarecrow on a pole over the hay: a straw-stuffed plaid shirt and patched
// overalls, a burlap face with a stitched smile, a battered straw hat.
export const meta = {
  name: "scarecrow",
  bounds: [0.8, 2.1, 0.25],
  tags: ["prop", "decoration"],
  props: { mass: 10, burns: 0.95 },
};

export function sdf(x, y, z, k) {
  const pole = box(x, y - 0.9, z, 0.04, 0.9, 0.04);
  const arms = capsule(x, y, z, -0.7, 1.45, 0, 0.7, 1.45, 0, 0.07);
  const body = roundBox(x, y - 1.2, z, 0.22, 0.32, 0.12, 0.08);
  const head = sphere(x, y - 1.7, z, 0.15);
  const hat = min(cylinder(x, y - 1.82, z, 0.3, 0.015), cylinder(x, y - 1.9, z, 0.14, 0.08));
  return min(min(pole, arms), min(min(body, head), hat));
}

export function color(x, y, z, k) {
  if (y > 1.8) return rgb(214, 186, 110);
  if (y > 1.55) return abs(y - 1.66) < 0.012 && z > 0.12 ? rgb(60, 40, 30) : rgb(196, 170, 120);
  if (y > 1.33) return step(0.5, fract(x * 6.0)) + step(0.5, fract(y * 6.0)) == 1 ? rgb(170, 40, 34) : rgb(60, 70, 120);
  if (y > 0.88) return rgb(70, 96, 140);
  return rgb(120, 100, 70);
}
