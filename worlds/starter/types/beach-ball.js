// A big beach ball striped red, yellow, blue and white, light and bouncy.
export const meta = {
  name: "beach ball",
  bounds: [0.25, 0.5, 0.25],
  tags: ["item", "toy", "ball"],
  props: { mass: 0.15, toy: 1, bounce: 0.85 },
};

export function sdf(x, y, z, k) {
  return sphere(x, y - 0.24, z, 0.24);
}

export function color(x, y, z, k) {
  const a = atan2(z, x);
  const s = floor(fract(a / 6.2832 + 0.5) * 6.0);
  if (abs(y - 0.24) > 0.22) return rgb(250, 250, 246);
  if (s < 1 || s == 3) return rgb(220, 40, 40);
  if (s == 1 || s == 4) return rgb(250, 210, 40);
  return rgb(40, 100, 200);
}
