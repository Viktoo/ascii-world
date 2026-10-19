// A white bathtub on clawed iron feet, a shower hose and a striped curtain.
export const meta = {
  name: "bathtub",
  bounds: [0.45, 1.95, 0.85],
  tags: ["furniture", "bathroom"],
  props: { mass: 120, wet: 0.3 },
};

export function sdf(x, y, z, k) {
  const tub = max(roundBox(x, y - 0.35, z, 0.38, 0.2, 0.8, 0.12), -roundBox(x, y - 0.5, z, 0.32, 0.2, 0.74, 0.1));
  const feet = sphere(abs(x) - 0.3, y - 0.07, abs(z) - 0.68, 0.07);
  const pole = capsule(x + 0.42, y, z, 0, 1.9, -0.8, 0, 1.9, 0.8, 0.015);
  const curtain = max(box(x + 0.42, y - 1.3, z + 0.55, 0.01, 0.6, 0.25), -0.0 + 0.0);
  return min(min(tub, feet), min(pole, curtain));
}

export function color(x, y, z, k) {
  if (x < -0.38 && y > 0.6) return fract(z * 5.0) < 0.5 ? rgb(90, 160, 200) : rgb(246, 246, 246);
  if (y < 0.15) return rgb(60, 60, 64);
  return rgb(248, 248, 246);
}
