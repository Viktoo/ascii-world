export const meta = {
  name: "test sword", bounds: [0.08, 0.5, 0.04], tags: ["item"], props: { mass: 1.5 },
  tool: { grip: [0, -0.42, 0], tip: [0, 0.48, 0], motions: ["swing", "thrust"] },
};
export function sdf(x, y, z, k) {
  return min(box(x, y - 0.1, z, 0.03, 0.38, 0.01), box(x, y + 0.42, z, 0.07, 0.07, 0.025));
}
export function color(x, y, z, k) { return rgb(200, 200, 210); }
