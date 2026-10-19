// An artist's wooden easel, three legs splayed, a ledge across its front legs
// to stand a canvas on, a peg at the top to hold it. The canvas leans on its
// front (+z), where the painter sits.
export const meta = {
  name: "easel",
  bounds: [0.36, 1.75, 0.5],
  tags: ["furniture", "art"],
  props: { mass: 6, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const legs = min(capsule(abs(x), y, z, 0.32, 0.0, 0.1, 0.05, 1.7, 0.0, 0.022), capsule(x, y, z, 0, 0.0, -0.45, 0, 1.6, -0.05, 0.02));
  const ledge = box(x, y - 0.72, z - 0.1, 0.36, 0.02, 0.05);
  const peg = box(x, y - 1.46, z - 0.05, 0.08, 0.02, 0.04);
  return min(min(legs, ledge), peg);
}

export function color(x, y, z, k) {
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
