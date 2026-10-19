// An artist's easel holding a half-finished painting of the harbour, a stool
// and a palette of blobbed paints beside it.
export const meta = {
  name: "easel",
  bounds: [0.45, 1.85, 0.45],
  tags: ["furniture", "prop"],
  props: { mass: 8, burns: 0.8 },
};

export function sdf(x, y, z, k) {
  const legs = min(capsule(x, y, z, -0.3, 0, -0.1, 0, 1.8, 0.05, 0.02), min(capsule(x, y, z, 0.3, 0, -0.1, 0, 1.8, 0.05, 0.02), capsule(x, y, z, 0, 0, 0.4, 0, 1.6, 0.05, 0.02)));
  const canvas = box(x, y - 1.25, z + 0.02, 0.35, 0.28, 0.015);
  const ledge = box(x, y - 0.95, z + 0.05, 0.38, 0.02, 0.05);
  return min(legs, min(canvas, ledge));
}

export function color(x, y, z, k) {
  if (abs(z - 0.005) < 0.03 && abs(x) < 0.34 && abs(y - 1.25) < 0.27) {
    if (y < 1.12) return mix(rgb(40, 110, 180), rgb(90, 160, 210), noise3(x * 8, y * 8, 2.0));
    if (y < 1.3) return hsv(fract(floor(x * 12.0) * 0.13), 0.5, 0.9);
    return rgb(150, 200, 236);
  }
  return rgb(150, 110, 70);
}
