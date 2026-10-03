// An old skull an orc camp tosses about: barely bounces, but it is for play.
export const meta = { name: "old skull", bounds: [0.11, 0.12, 0.13], tags: ["item", "bone"], props: { mass: 1.5, bounce: 0.1, toy: 1 } };

export function sdf(x, y, z, k) {
  return sphere(x, y, z, 0.11);
}

export function color(x, y, z, k) {
  return rgb(220, 210, 180);
}
