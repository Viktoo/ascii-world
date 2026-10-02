// A piece of flint.
export const meta = { name: "flint", bounds: [0.06, 0.04, 0.06], tags: ["item", "stone"], props: { mass: 0.2 } };

export function sdf(x, y, z, k) {
  return roundBox(x, y, z, 0.05, 0.03, 0.04, 0.01);
}

export function color(x, y, z, k) {
  return rgb(60, 62, 70);
}
