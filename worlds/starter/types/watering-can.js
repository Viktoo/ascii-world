// A green tin watering can with a long spout, for the flower boxes.
export const meta = {
  name: "watering can",
  bounds: [0.12, 0.38, 0.32],
  tags: ["item", "tool"],
  props: { mass: 3, wet: 0.8 },
  tool: { grip: [0, 0.28, -0.02], tip: [0, 0.22, 0.28], motions: ["pour"], sharp: 0 },
};

export function sdf(x, y, z, k) {
  return min(min(cylinder(x, y - 0.11, z, 0.1, 0.11), cone(x, y, z, 0, 0.05, 0.08, 0, 0.22, 0.28, 0.025, 0.015)), max(abs(length2(z + 0.02, y - 0.2) - 0.08) - 0.01, abs(x) - 0.01));
}

export function color(x, y, z, k) {
  return rgb(60, 130, 80);
}
