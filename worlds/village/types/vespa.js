// A Vespa scooter in mint green, cream or red: the round steel body over its
// little back wheel, a long saddle for two, a chrome headlamp on the handlebar
// and a spare wheel behind. Its front is +z.
export const meta = {
  name: "vespa",
  bounds: [0.4, 1.12, 0.95],
  tags: ["vehicle", "scooter"],
  props: { mass: 90, fuel: 0.4 },
  anchors: [
    { kind: "seat", at: [0, 0.75, -0.05], face: 0 },
    { kind: "seat", at: [0, 0.75, -0.45], face: 0 },
  ],
  drive: { speed: 12, on: "land", by: "engine" },
};

export function sdf(x, y, z, k) {
  const wf = rotZ(x, y - 0.18, z - 0.6, 1.5708);
  const wb = rotZ(x, y - 0.18, z + 0.6, 1.5708);
  const wheels = min(torus(wf.x, wf.y, wf.z, 0.14, 0.06), torus(wb.x, wb.y, wb.z, 0.14, 0.06));
  const body = ellipsoid(x, y - 0.42, z + 0.42, 0.28, 0.25, 0.42);
  const floorboard = box(x, y - 0.25, z - 0.15, 0.17, 0.04, 0.35);
  const shield = box(x, y - 0.55, z - 0.5, 0.2, 0.32, 0.05);
  const stem = capsule(x, y, z, 0, 0.3, 0.6, 0, 0.95, 0.52, 0.03);
  const bars = capsule(x, y, z, -0.3, 0.97, 0.5, 0.3, 0.97, 0.5, 0.022);
  const lamp = sphere(x, y - 0.98, z - 0.58, 0.07);
  const saddle = roundBox(x, y - 0.7, z + 0.3, 0.13, 0.05, 0.38, 0.04);
  return min(min(wheels, body), min(min(floorboard, shield), min(min(stem, bars), min(lamp, saddle))));
}

export function color(x, y, z, k) {
  if (y > 0.92 && z > 0.52) return glow(rgb(250, 246, 220), 0.6);
  if (abs(y - 0.7) < 0.06 && z < -0.05) return rgb(40, 34, 30);
  if (y < 0.32 && abs(z) > 0.45) return rgb(30, 30, 30);
  const c = floor(hash(k.seed) * 3);
  return c < 1 ? rgb(150, 210, 190) : c < 2 ? rgb(240, 230, 200) : rgb(200, 40, 40);
}
