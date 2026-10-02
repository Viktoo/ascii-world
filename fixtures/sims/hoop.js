// A hoop on a pole, for throwing things through. Nothing in the engine knows
// what a hoop is: it is a ring the shape leaves open at the top.
export const meta = { name: "hoop", bounds: [0.6, 3.4, 0.6], tags: ["landmark", "hoop"], props: { mass: 3000 } };

export function sdf(x, y, z, k) {
  const pole = cylinder(x, y - 1.5, z + 0.42, 0.05, 1.5);
  const arm = box(x, y - 3.0, z + 0.32, 0.03, 0.03, 0.1);
  const ring = torus(x, y - 3.0, z, 0.24, 0.025);
  return min(pole, arm, ring);
}

export function color(x, y, z, k) {
  if (y > 2.9) {
    return rgb(220, 90, 40);
  }
  return rgb(120, 120, 128);
}
