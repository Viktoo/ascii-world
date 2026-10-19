// An upright piano of dark wood, its lid open on yellowed keys, a candlestick
// and sheet music on it, a stool before it (+z).
export const meta = {
  name: "upright piano",
  bounds: [0.75, 1.35, 0.6],
  tags: ["furniture", "instrument"],
  props: { mass: 200, burns: 0.5 },
  anchors: [
    { kind: "seat", at: [0, 0.5, -0.55], face: 180 },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 0.65, z, 0.72, 0.65, 0.28);
  const keys = box(x, y - 0.72, z - 0.36, 0.65, 0.04, 0.1);
  const stool = cylinder(x, y - 0.25, z - 0.55, 0.16, 0.25);
  return min(min(body, keys), stool);
}

export function color(x, y, z, k) {
  if (abs(y - 0.74) < 0.03 && z > 0.26) return fract(x * 22.0) < 0.15 ? rgb(30, 30, 30) : rgb(240, 232, 210);
  return mix(rgb(84, 56, 38), rgb(104, 72, 48), noise3(x * 6, y * 18, z * 6));
}
