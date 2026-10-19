// A home's kitchen along a wall: a counter of white tiles with a sink and a
// gas stove with a pot of sauce on it, a shelf of jars and copper pans above,
// strings of garlic and peppers. The cook stands at its front (+z).
export const meta = {
  name: "kitchen",
  bounds: [1.25, 1.95, 0.32],
  tags: ["furniture", "kitchen"],
  props: { mass: 200, burns: 0.3, heat: 60 },
  anchors: [
    { kind: "light", at: [0, 1.8, 0.2] },
  ],
};

export function sdf(x, y, z, k) {
  const counter = box(x, y - 0.45, z, 1.22, 0.45, 0.3);
  const sink = box(x + 0.6, y - 0.86, z, 0.28, 0.1, 0.2);
  const pot = cylinder(x - 0.55, y - 1.0, z + 0.02, 0.14, 0.11);
  const shelf = box(x, y - 1.55, z + 0.12, 1.15, 0.025, 0.16);
  const jars = max(cylinder(x - 0.3 * floor(x / 0.3 + 0.5), y - 1.66, z + 0.12, 0.06, 0.09), abs(x) - 1.1);
  const pans = sphere(x + 0.3 * floor(-x / 0.3 + 0.5) * 0.0 - 0.2, y - 1.3, z + 0.25, 0.1);
  return min(min(max(counter, -sink), pot), min(shelf, min(jars, pans)));
}

export function color(x, y, z, k) {
  if (y > 1.5) return y > 1.6 ? hsv(fract(floor(x / 0.3) * 0.27), 0.5, 0.8) : rgb(130, 90, 56);
  if (y > 0.89 && abs(x - 0.6) < 0.3) return rgb(190, 196, 200);
  if (y > 0.89 && abs(x + 0.55) < 0.16) return y > 1.08 ? rgb(190, 40, 30) : rgb(70, 70, 76);
  if (y > 1.1) return rgb(196, 120, 70);
  if (z > 0.28 && y < 0.82) return fract(x * 1.6) < 0.04 ? rgb(60, 60, 60) : rgb(70, 140, 170);
  if (y > 0.86) return (fract(x * 6.0) < 0.06 || fract(z * 6.0) < 0.06) ? rgb(180, 180, 180) : rgb(240, 238, 232);
  return rgb(240, 238, 232);
}
