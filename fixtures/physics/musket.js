export const meta = {
  name: "test musket", bounds: [0.05, 0.08, 0.75], tags: ["item"], props: { mass: 4.5 },
  tool: { grip: [0, 0, -0.3], tip: [0, 0.03, 0.72], motions: ["swing", "shoot"], launch: { ammo: "lead ball", speed: 400, spread: 1, holds: 3, ready: 0.5 } },
};
export function sdf(x, y, z, k) {
  return min(capsule(x, y, z, 0, 0.03, -0.1, 0, 0.03, 0.72, 0.015), box(x, y + 0.02, z + 0.45, 0.025, 0.06, 0.28));
}
export function color(x, y, z, k) { return rgb(90, 70, 50); }
