// Built-in shot: what a launcher sends flying until the world has written
// its own ammunition (an arrow, a bolt, a ball). Point first along +z.
export const meta = {
  name: "shot",
  bounds: [0.035, 0.035, 0.4],
  tags: ["builtin", "shot", "small", "nonsolid"],
  props: { mass: 0.05, bounce: 0.1, burns: 0.4 },
  tool: { grip: [0, 0, -0.15], tip: [0, 0, 0.39], motions: ["thrust"], sharp: 0.85 },
};

export function sdf(x, y, z, k) {
  const shaft = capsule(x, y, z, 0, 0, -0.36, 0, 0, 0.3, 0.008);
  const head = ellipsoid(x, y, z - 0.33, 0.02, 0.02, 0.06);
  const vane = min(box(x, y, z + 0.31, 0.002, 0.028, 0.045), box(x, y, z + 0.31, 0.028, 0.002, 0.045));
  return min(min(shaft, head), vane);
}

export function color(x, y, z, k) {
  if (z > 0.27) return rgb(170, 170, 176);
  if (z < -0.26) return rgb(200, 60, 50);
  return rgb(120, 92, 60);
}
