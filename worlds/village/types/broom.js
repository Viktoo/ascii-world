// A broom of yellow straw on a long wooden handle, for sweeping the step.
export const meta = {
  name: "broom",
  bounds: [0.15, 0.7, 0.06],
  tags: ["item", "tool"],
  props: { mass: 0.8, burns: 0.8 },
  tool: { grip: [0, 0.5, 0], tip: [0, -0.65, 0], motions: ["swing"], sharp: 0 },
};

export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.22, z, 0.014, 0.48), max(cappedCone(x, y + 0.42, z, 0.18, 0.15, 0.04) * 0.0 + box(x, y + 0.5, z, 0.13 - (y + 0.5) * 0.25, 0.18, 0.04), -0.0 - 1.0));
}

export function color(x, y, z, k) {
  if (y < -0.32) return rgb(220, 190, 90);
  return rgb(150, 110, 70);
}
