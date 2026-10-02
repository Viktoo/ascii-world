// A small wooden hut.
export const meta = { name: "wooden hut", bounds: [2.6, 3.0, 2.6], tags: ["building"], props: { burns: 0.5 } };

export function sdf(x, y, z, k) {
  const walls = box(x, y - 1.2, z, 2.0, 1.2, 2.0);
  const roof = cappedCone(x, y - 2.8, z, 0.4, 2.5, 0.2);
  return min(walls, roof);
}

export function color(x, y, z, k) {
  return y > 2.4 ? rgb(120, 70, 50) : rgb(150, 110, 70);
}
