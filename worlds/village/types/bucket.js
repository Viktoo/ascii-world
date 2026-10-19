// A zinc bucket of water with a wire handle.
export const meta = {
  name: "bucket",
  bounds: [0.15, 0.42, 0.16],
  tags: ["item", "tool"],
  props: { mass: 6, wet: 1 },
  tool: { grip: [0, 0.38, 0], tip: [0, 0.2, 0.15], motions: ["pour"], sharp: 0 },
};

export function sdf(x, y, z, k) {
  return min(max(cappedCone(x, y - 0.13, z, 0.13, 0.11, 0.14), -cappedCone(x, y - 0.15, z, 0.12, 0.1, 0.13)), max(abs(length2(x, y - 0.26) - 0.13) - 0.006, abs(z) - 0.006));
}

export function color(x, y, z, k) {
  if (y > 0.2 && length2(x, z) < 0.13) return rgb(90, 140, 170);
  return rgb(170, 176, 180);
}
