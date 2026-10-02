// An oil lantern: warm and bright, and full of oil if it breaks.
export const meta = { name: "oil lantern", bounds: [0.12, 0.2, 0.12], tags: ["item", "light"], props: { mass: 1.2, light: 1, heat: 150, fragile: 0.7, burns: 0.6, fuel: 0.5 } };

export function sdf(x, y, z, k) {
  const body = cylinder(x, y + 0.04, z, 0.09, 0.12);
  const cap = cappedCone(x, y - 0.12, z, 0.04, 0.09, 0.03);
  return min(body, cap);
}

export function color(x, y, z, k) {
  return abs(y + 0.04) < 0.07 ? rgb(255, 210, 120) : rgb(60, 50, 40);
}
