// The barber's chair: red leather on chrome, a footrest, before a tall
// mirror with a shelf of combs, scissors and bottles of tonic. Faces +z, its
// back to the mirror at -z.
export const meta = {
  name: "barber chair",
  bounds: [0.7, 1.95, 0.6],
  tags: ["furniture", "seat", "shop"],
  props: { mass: 80 },
  anchors: [
    { kind: "seat", at: [0, 0.7, 0.05], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const base = cylinder(x, y - 0.15, z, 0.25, 0.15);
  const post = cylinder(x, y - 0.35, z, 0.06, 0.2);
  const seat = roundBox(x, y - 0.6, z + 0.05, 0.3, 0.08, 0.3, 0.05);
  const back = roundBox(x, y - 0.95, z - 0.25, 0.3, 0.32, 0.06, 0.05);
  const mirror = box(x, y - 1.4, z + 0.57, 0.65, 0.5, 0.02);
  const shelf = box(x, y - 0.85, z + 0.48, 0.65, 0.025, 0.1);
  return min(min(base, post), min(min(seat, back), min(mirror, shelf)));
}

export function color(x, y, z, k) {
  if (z < -0.5 && y > 0.88) return glow(rgb(200, 220, 230), 0.15);
  if (y < 0.5 || z < -0.4) return rgb(200, 204, 210);
  return rgb(170, 30, 36);
}
