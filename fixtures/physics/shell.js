export const meta = {
  name: "test shell", bounds: [0.08, 0.08, 0.08], tags: ["item"], props: { mass: 3 },
  burst: { radius: 4, hurt: 0.7, crater: 0.5, impact: true },
};
export function sdf(x, y, z, k) { return sphere(x, y, z, 0.075); }
export function color(x, y, z, k) { return rgb(50, 50, 55); }
