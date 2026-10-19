// A chest of drawers with brass pulls, on it a lace runner, framed family
// photographs and a vase of flowers.
export const meta = {
  name: "chest of drawers",
  bounds: [0.6, 1.25, 0.26],
  tags: ["furniture"],
  props: { mass: 50, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 0.42, z, 0.56, 0.42, 0.24);
  const frames = box(abs(x) - 0.3, y - 0.98 + 0.0, z + 0.05, 0.08, 0.1, 0.01);
  const vase = min(cappedCone(x, y - 0.95, z, 0.1, 0.06, 0.04), sphere(x, y - 1.1, z, 0.1));
  return min(body, min(max(frames, -0.0 + 0.0), vase));
}

export function color(x, y, z, k) {
  if (y > 1.02 && abs(x) < 0.11) return hsv(fract(hash(k.seed) + 0.9), 0.7, 0.9);
  if (y > 0.86 && abs(x) < 0.1) return rgb(60, 110, 160);
  if (y > 0.86) return rgb(210, 190, 150);
  if (z > 0.22 && abs(fract(y / 0.27) - 0.5) < 0.08 && abs(abs(x) - 0.25) < 0.05) return rgb(200, 160, 60);
  return mix(rgb(84, 56, 38), rgb(104, 72, 48), noise3(x * 6, y * 18, z * 6));
}
