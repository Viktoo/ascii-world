export const meta = {
  name: "test arrow", bounds: [0.03, 0.03, 0.4], tags: ["item"], props: { mass: 0.05 },
  tool: { grip: [0, 0, -0.1], tip: [0, 0, 0.39], motions: ["thrust"], sharp: 0.9 },
};
export function sdf(x, y, z, k) {
  return min(capsule(x, y, z, 0, 0, -0.38, 0, 0, 0.33, 0.007), ellipsoid(x, y, z - 0.35, 0.016, 0.016, 0.045));
}
export function color(x, y, z, k) { return rgb(140, 110, 70); }
