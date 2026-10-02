// A leather ball: it bounces.
export const meta = { name: "leather ball", bounds: [0.13, 0.13, 0.13], tags: ["item", "ball"], props: { mass: 0.5, bounce: 0.8, burns: 0.3 } };

export function sdf(x, y, z, k) {
  return sphere(x, y, z, 0.12);
}

export function color(x, y, z, k) {
  return abs(y) < 0.012 ? rgb(60, 40, 30) : rgb(170, 100, 50);
}
