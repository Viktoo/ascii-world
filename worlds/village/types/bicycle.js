// A sturdy town bicycle with a basket on the front, a bell and a rack behind:
// black, red or green paint. Ridden by pedalling; its front is +z.
export const meta = {
  name: "bicycle",
  bounds: [0.3, 1.05, 0.95],
  tags: ["vehicle", "bicycle"],
  props: { mass: 15 },
  anchors: [
    { kind: "seat", at: [0, 0.82, -0.15], face: 0 },
  ],
  drive: { speed: 5, on: "land", by: "legs" },
};

export function sdf(x, y, z, k) {
  const wf = rotZ(x, y - 0.34, z - 0.55, 1.5708);
  const wb = rotZ(x, y - 0.34, z + 0.55, 1.5708);
  const wheels = min(torus(wf.x, wf.y, wf.z, 0.32, 0.025), torus(wb.x, wb.y, wb.z, 0.32, 0.025));
  const frame = min(min(capsule(x, y, z, 0, 0.34, -0.55, 0, 0.75, -0.1, 0.02), capsule(x, y, z, 0, 0.75, -0.1, 0, 0.82, 0.45, 0.02)), min(capsule(x, y, z, 0, 0.82, 0.45, 0, 0.34, 0.55, 0.02), capsule(x, y, z, 0, 0.34, -0.55, 0, 0.4, 0.05, 0.02)));
  const seat = roundBox(x, y - 0.85, z + 0.15, 0.07, 0.03, 0.12, 0.02);
  const bars = capsule(x, y, z, -0.25, 0.98, 0.5, 0.25, 0.98, 0.5, 0.015);
  const basket = box(x, y - 0.85, z - 0.75, 0.16, 0.1, 0.13);
  return min(min(wheels, frame), min(seat, min(bars, basket)));
}

export function color(x, y, z, k) {
  if (abs(y - 0.85) < 0.12 && z > 0.6) return rgb(190, 150, 90);
  if (y > 0.82 && abs(z - 0.15) < 0.14) return rgb(60, 40, 30);
  const c = floor(hash(k.seed) * 3);
  return c < 1 ? rgb(30, 30, 32) : c < 2 ? rgb(170, 30, 30) : rgb(40, 110, 70);
}
