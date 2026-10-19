// A bunch of flowers wrapped in paper: roses, daisies and sprigs of lavender.
export const meta = {
  name: "bunch of flowers",
  bounds: [0.1, 0.25, 0.1],
  tags: ["item"],
  props: { mass: 0.3, alive: 1, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  return min(cappedCone(x, y - 0.1, z, 0.1, 0.02, 0.07), sphere(x, y - 0.2, z, 0.09));
}

export function color(x, y, z, k) {
  if (y < 0.18) return rgb(236, 224, 200);
  return hsv(fract(noise3(x * 30, y * 30, z * 30) * 2.0), 0.6, 0.9);
}
