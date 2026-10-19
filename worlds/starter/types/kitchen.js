// A cottage kitchen along a wall: cream-painted cupboards with brass knobs
// under a butcher-block counter, a deep white sink, a black cast-iron stove
// with a kettle and a pot of stew on it, a shelf of jars, crocks and blue
// plates above, copper pans hung under it. The cook stands at its front (+z).
export const meta = {
  name: "kitchen",
  bounds: [1.25, 1.95, 0.32],
  tags: ["furniture", "kitchen", "oven"],
  props: { mass: 200, burns: 0.3, heat: 60 },
  anchors: [
    { kind: "light", at: [0, 1.8, 0.2] },
  ],
};

export function sdf(x, y, z, k) {
  const counter = box(x - 0.35, y - 0.45, z, 0.88, 0.45, 0.3);
  const top = box(x - 0.35, y - 0.915, z + 0.01, 0.9, 0.035, 0.31);
  const sink = box(x - 0.75, y - 0.86, z + 0.02, 0.26, 0.1, 0.2);
  const stove = roundBox(x - 0.89, y - 0.42, z, 0.36, 0.42, 0.28, 0.03);
  const flue = cylinder(x - 0.89, y - 1.5, z + 0.18, 0.05, 0.65);
  const kettle = min(ellipsoid(x - 0.78, y - 0.95, z - 0.02, 0.09, 0.07, 0.09), capsule(x, y, z, 0.86, 0.94, -0.02, 0.95, 1.0, -0.02, 0.012));
  const pot = cylinder(x - 1.02, y - 0.94, z + 0.05, 0.1, 0.09);
  const knobs = max(sphere(x - 0.3 * floor(x / 0.3 + 0.5) - 0.1, y - 0.62, z - 0.31, 0.018), x - 0.5);
  const shelf = box(x - 0.2, y - 1.55, z + 0.14, 0.95, 0.02, 0.14);
  const jars = max(cylinder(x - 0.22 * floor(x / 0.22 + 0.5), y - 1.65, z + 0.13, 0.055 + 0.02 * hash(floor(x / 0.22 + 0.5)), 0.08), abs(x + 0.2) - 0.9);
  const pans = max(cylinder(x - 0.3 * floor(x / 0.3 + 0.5), y - 1.3, z + 0.27, 0.09, 0.012), abs(x + 0.3) - 0.5);
  return min(min(min(max(min(counter, top), -sink), stove), min(flue, min(kettle, pot))), min(min(knobs, shelf), min(jars, pans)));
}

export function color(x, y, z, k) {
  if (x > 0.5 && y < 1.05 && y > 0.86) return abs(x - 1.02) < 0.11 && y > 0.98 ? rgb(130, 70, 40) : rgb(50, 50, 54);
  if (x > 0.5 && y < 0.86) return abs(y - 0.42) < 0.2 && z > 0.27 && abs(x - 0.89) < 0.22 ? glow(rgb(255, 120, 40), 0.4) : rgb(36, 36, 40);
  if (abs(x - 0.89) < 0.06 && y > 0.86) return rgb(40, 40, 44);
  if (y > 1.55) return hsv(fract(floor(x / 0.22 + 0.5) * 0.31), 0.45, 0.75);
  if (y > 1.52) return rgb(130, 90, 56);
  if (y > 1.2 && y < 1.4) return rgb(196, 110, 60);
  if (y > 0.88 && y < 0.96 && abs(x + 0.75) < 0.27 && abs(z) < 0.22) return rgb(236, 238, 240);
  if (y > 0.88 && y < 0.96) return mix(rgb(176, 130, 84), rgb(200, 156, 104), noise3(x * 3, y * 30, z * 30));
  if (abs(y - 0.62) < 0.03 && z > 0.29) return rgb(200, 170, 70);
  if (z > 0.28 && (fract((x + 0.35) / 0.44) < 0.04 || abs(y - 0.7) < 0.015)) return rgb(200, 190, 166);
  return rgb(236, 226, 196);
}
