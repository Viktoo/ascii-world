// A woodcutter's felling axe: a long ash handle, a heavy steel head with a
// keen edge. A tool for chopping.
export const meta = {
  name: "woodcutter's axe",
  bounds: [0.14, 0.5, 0.06],
  tags: ["item", "tool"],
  props: { mass: 2.4, burns: 0.3 },
  tool: { grip: [0, -0.4, 0], tip: [0.12, 0.38, 0], motions: ["chop", "swing"], sharp: 0.6 },
};

export function sdf(x, y, z, k) {
  return min(box(x, y + 0.05, z, 0.022, 0.45, 0.018), box(x - 0.05, y - 0.37, z, 0.08, 0.06, 0.02));
}

export function color(x, y, z, k) {
  if (y > 0.3) return x > 0.1 ? rgb(220, 224, 228) : rgb(110, 114, 120);
  return rgb(176, 136, 88);
}
