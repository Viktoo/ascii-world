export const meta = {
  name: "test sapling", bounds: [0.5, 1.6, 0.5], tags: ["tree"], props: { alive: 1, growth: 1, mass: 60, burns: 0.5 },
};
export function sdf(x, y, z, k) {
  return min(cylinder(x, y + 0.6, z, 0.08, 1.0), sphere(x, y - 1.0, z, 0.5));
}
export function color(x, y, z, k) { return y > 0.5 ? rgb(60, 140, 60) : rgb(100, 70, 50); }
