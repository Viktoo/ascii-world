export const meta = {
  name: "test bow", bounds: [0.06, 0.65, 0.12], tags: ["item"], props: { mass: 0.9 },
  tool: { grip: [0, 0, 0], tip: [0, 0, 0.1], motions: ["swing"], launch: { ammo: "test arrow", speed: 50, spread: 0.5, holds: 2, ready: 0.6 } },
};
export function sdf(x, y, z, k) {
  return max(abs(length2(y, z + 0.5) - 0.55) - 0.015, max(abs(x) - 0.012, -z - 0.5 + 0.05));
}
export function color(x, y, z, k) { return rgb(130, 90, 50); }
