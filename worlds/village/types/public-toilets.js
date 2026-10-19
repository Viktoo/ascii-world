// The public toilets: a small whitewashed block with two doors, UOMINI and
// DONNE in blue letters, a washbasin outside. Doors face +z.
export const meta = {
  name: "public toilets",
  bounds: [2.3, 3.0, 1.8],
  tags: ["building", "bathroom"],
  props: { mass: 30000, wet: 0.2 },
  anchors: [
    { kind: "door", at: [-1.1, 0, 1.6], face: 0, size: [0.9, 2.1] },
    { kind: "door", at: [1.1, 0, 1.6], face: 0, size: [0.9, 2.1] },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 1.35, z, 2.2, 1.35, 1.6);
  const rooms = min(box(x - 1.1, y - 1.2, z, 0.9, 1.15, 1.35), box(x + 1.1, y - 1.2, z, 0.9, 1.15, 1.35));
  const doors = min(box(x - 1.1, y - 1.05, z - 1.6, 0.45, 1.05, 0.4), box(x + 1.1, y - 1.05, z - 1.6, 0.45, 1.05, 0.4));
  const roof = box(x, y - 2.8, z, 2.35, 0.12, 1.75);
  const toilets = roundBox(abs(x) - 1.1, y - 0.22, z + 0.9, 0.18, 0.22, 0.25, 0.08);
  return min(min(max(body, -min(rooms, doors)), roof), toilets);
}

export function color(x, y, z, k) {
  if (z > 1.55 && y > 2.2 && y < 2.55 && abs(abs(x) - 1.1) < 0.6) return rgb(40, 70, 150);
  if (y > 2.65) return rgb(176, 78, 44);
  if (abs(z - 0.9) < 0.3 && y < 0.5 && abs(abs(x) - 1.1) < 0.2) return rgb(246, 246, 246);
  if (y < 0.3) return rgb(160, 150, 136);
  return rgb(244, 240, 232);
}
