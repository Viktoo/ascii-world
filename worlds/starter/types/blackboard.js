// A blackboard on an easel stand, chalked with the alphabet, a sum and a
// drawing of a fish; a box of chalk on its ledge.
export const meta = {
  name: "blackboard",
  bounds: [0.85, 1.9, 0.4],
  tags: ["furniture", "prop"],
  props: { mass: 20, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const board = box(x, y - 1.3, z, 0.8, 0.5, 0.025);
  const legs = capsule(abs(x) - 0.7, y, z, 0, 0, -0.35, 0, 1.85, 0, 0.025);
  return min(board, legs);
}

export function color(x, y, z, k) {
  if (abs(z) < 0.03 && abs(x) < 0.76 && abs(y - 1.3) < 0.46) return noise3(x * 30, y * 30, 1.0) > 0.78 ? rgb(230, 230, 220) : rgb(40, 60, 50);
  return rgb(120, 90, 60);
}
