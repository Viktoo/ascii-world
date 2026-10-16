// Built-in slug: what a fast launcher (a gun) sends off until the world has
// written its own bullets. A small pointed slug, point along +z.
export const meta = {
  name: "slug",
  bounds: [0.012, 0.012, 0.03],
  tags: ["builtin", "shot", "small", "nonsolid"],
  props: { mass: 0.01, bounce: 0.05, conducts: 0.6 },
  tool: { grip: [0, 0, -0.02], tip: [0, 0, 0.028], motions: ["thrust"], sharp: 0.6 },
};

export function sdf(x, y, z, k) {
  return min(capsule(x, y, z, 0, 0, -0.018, 0, 0, 0.012, 0.0085), ellipsoid(x, y, z - 0.014, 0.0085, 0.0085, 0.014));
}

export function color(x, y, z, k) {
  return rgb(180, 140, 70);
}
