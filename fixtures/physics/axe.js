export const meta = {
  name: "test axe", bounds: [0.14, 0.45, 0.04], tags: ["item"], props: { mass: 2.5 },
  tool: { grip: [0, -0.38, 0], tip: [0.1, 0.38, 0], motions: ["chop", "swing"] },
};
export function sdf(x, y, z, k) {
  return min(cylinder(x, y, z, 0.02, 0.42), box(x - 0.07, y - 0.36, z, 0.07, 0.07, 0.015));
}
export function color(x, y, z, k) { return rgb(120, 100, 80); }
