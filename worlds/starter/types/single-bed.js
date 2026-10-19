// A narrow single bed: a painted iron frame, a mattress with a patterned blanket
// and one pillow. The headboard is at -z.
export const meta = {
  name: "single bed",
  bounds: [0.5, 0.75, 1.02],
  tags: ["furniture", "bed"],
  props: { mass: 30, burns: 0.5 },
  anchors: [
    { kind: "bed", at: [0, 0.5, 0] },
  ],
};

export function sdf(x, y, z, k) {
  const frame = max(box(x, y - 0.35, z, 0.48, 0.35, 1.0), -box(x, y - 0.42, z, 0.44, 0.33, 0.96));
  const mattress = roundBox(x, y - 0.42, z, 0.44, 0.08, 0.96, 0.04);
  const head = box(x, y - 0.6, z + 0.98, 0.48, 0.6, 0.025);
  const pillow = roundBox(x, y - 0.54, z + 0.72, 0.34, 0.06, 0.16, 0.05);
  return min(min(frame, mattress), min(head, pillow));
}

export function color(x, y, z, k) {
  if (y > 0.48 && z < -0.55) return rgb(244, 242, 236);
  if (abs(x) > 0.44 || z < -0.97 || y < 0.33) return rgb(60, 84, 110);
  return step(0.5, fract(x * 4.0)) + step(0.5, fract(z * 4.0)) == 1 ? rgb(220, 120, 60) : rgb(240, 220, 170);
}
