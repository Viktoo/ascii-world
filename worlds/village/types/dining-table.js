// The family table: a long wooden table under a red-and-white checked cloth,
// four chairs round it, laid with plates of pasta, a bowl of salad, a loaf
// and a jug of wine. Four can sit and eat.
export const meta = {
  name: "dining table",
  bounds: [1.0, 1.0, 0.95],
  tags: ["furniture", "table"],
  props: { mass: 50, burns: 0.6 },
  anchors: [
    { kind: "seat", at: [-0.38, 0.47, 0.66], face: 180 },
    { kind: "seat", at: [0.38, 0.47, 0.66], face: 180 },
    { kind: "seat", at: [-0.38, 0.47, -0.66], face: 0 },
    { kind: "seat", at: [0.38, 0.47, -0.66], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const top = box(x, y - 0.74, z, 0.75, 0.025, 0.45);
  const legs = box(abs(x) - 0.68, y - 0.36, abs(z) - 0.38, 0.03, 0.36, 0.03);
  const plates = max(cylinder(abs(x) - 0.4, y - 0.78, abs(z) - 0.2, 0.13, 0.015), -0.0 + 0.0);
  const food = ellipsoid(abs(x) - 0.4, y - 0.8, abs(z) - 0.2, 0.08, 0.035, 0.08);
  const jug = cappedCone(x, y - 0.88, z, 0.12, 0.08, 0.05);
  const bread = ellipsoid(x - 0.0, y - 0.8, z - 0.22, 0.14, 0.05, 0.06);
  const cx = abs(x) - 0.38;
  const seats = box(cx, y - 0.45, abs(z) - 0.68, 0.2, 0.025, 0.19);
  const backs = box(cx, y - 0.72, abs(z) - 0.86, 0.2, 0.25, 0.02);
  const clegs = box(abs(cx) - 0.17, y - 0.22, abs(abs(z) - 0.68) - 0.16, 0.02, 0.22, 0.02);
  return min(min(min(top, legs), min(plates, food)), min(min(jug, bread), min(seats, min(backs, clegs))));
}

export function color(x, y, z, k) {
  if (y > 0.84 && length2(x, z) < 0.13) return rgb(150, 40, 50);
  if (y > 0.76 && abs(z - 0.22) < 0.07 && abs(x) < 0.15) return rgb(200, 150, 80);
  if (y > 0.79 && abs(abs(z) - 0.2) < 0.1 && abs(abs(x) - 0.4) < 0.1) return noise3(x * 20, y * 20, z * 20) > 0.6 ? rgb(200, 50, 30) : rgb(240, 200, 110);
  if (y > 0.762 && abs(abs(z) - 0.2) < 0.14 && abs(abs(x) - 0.4) < 0.14) return rgb(246, 246, 240);
  if (abs(y - 0.74) < 0.03 && abs(z) < 0.46) return step(0.5, fract(x * 5.0)) + step(0.5, fract(z * 5.0)) == 1 ? rgb(200, 40, 40) : rgb(246, 242, 236);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
