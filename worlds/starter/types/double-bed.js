// A double bed: a wooden frame and headboard, a thick mattress under a white
// sheet and a coloured bedspread, two pillows. The headboard is at -z.
export const meta = {
  name: "double bed",
  bounds: [0.85, 1.15, 1.08],
  tags: ["furniture", "bed"],
  props: { mass: 60, burns: 0.6 },
  anchors: [
    { kind: "bed", at: [-0.35, 0.56, 0] },
    { kind: "bed", at: [0.35, 0.56, 0] },
  ],
};

export function sdf(x, y, z, k) {
  const frame = box(x, y - 0.2, z, 0.82, 0.2, 1.02);
  const mattress = roundBox(x, y - 0.46, z + 0.04, 0.78, 0.1, 0.96, 0.05);
  const head = box(x, y - 0.55, z + 1.03, 0.82, 0.55, 0.04);
  const pillows = roundBox(abs(x) - 0.38, y - 0.6, z + 0.75, 0.3, 0.06, 0.16, 0.05);
  return min(min(frame, mattress), min(head, pillows));
}

export function color(x, y, z, k) {
  if (y < 0.36 || z < -1.0) return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
  if (y > 0.54 && z < -0.55) return rgb(244, 242, 236);
  const c = floor(hash(k.seed) * 4);
  const bed = c < 1 ? rgb(170, 40, 50) : c < 2 ? rgb(40, 90, 150) : c < 3 ? rgb(220, 180, 60) : rgb(60, 130, 90);
  return fract((x + z) * 3.0) < 0.5 ? bed : bed * 0.85;
}
