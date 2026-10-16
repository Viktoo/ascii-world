export const meta = {
  name: "test grenade", bounds: [0.06, 0.08, 0.06], tags: ["item"], props: { mass: 0.6, bounce: 0.2 },
  burst: { radius: 6, hurt: 0.9, push: 18, crater: 0.7, fire: 0.2, fuse: 2 },
};
export function sdf(x, y, z, k) { return ellipsoid(x, y, z, 0.055, 0.075, 0.055); }
export function color(x, y, z, k) { return rgb(70, 90, 50); }
