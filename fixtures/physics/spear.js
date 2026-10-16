export const meta = {
  name: "test spear", bounds: [0.04, 0.04, 1.0], tags: ["item"], props: { mass: 1.6 },
  tool: { grip: [0, 0, -0.2], tip: [0, 0, 0.98], motions: ["thrust"], sharp: 0.9 },
};
export function sdf(x, y, z, k) {
  return min(capsule(x, y, z, 0, 0, -0.95, 0, 0, 0.8, 0.018), ellipsoid(x, y, z - 0.88, 0.03, 0.012, 0.1));
}
export function color(x, y, z, k) {
  if (z > 0.78) return rgb(190, 190, 200);
  return rgb(120, 90, 60);
}
