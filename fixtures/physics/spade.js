export const meta = {
  name: "test spade", bounds: [0.14, 0.6, 0.04], tags: ["item"], props: { mass: 2 },
  tool: { grip: [0, 0.5, 0], tip: [0, -0.55, 0], motions: ["dig", "swing"] },
};
export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.15, z, 0.02, 0.42), box(x, y + 0.42, z, 0.12, 0.15, 0.015));
}
export function color(x, y, z, k) { return rgb(120, 100, 80); }
