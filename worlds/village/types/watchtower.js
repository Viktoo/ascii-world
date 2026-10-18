// The watchtower at the corner of Marinella's wall, 15 m of limestone:
// a door at its foot (+z, toward the village), and inside a wooden ladder
// up the back wall through an opening in the floor of the lookout, where
// a parapet runs round under four corner posts holding a little tiled
// roof. From there the watch sees the bay, the cape and the road.
export const meta = {
  name: "watchtower",
  bounds: [2.75, 16.6, 2.75],
  tags: ["building", "landmark", "lookout"],
  props: { mass: 100000 },
  sound: { hard: 1, dry: 0.4, ring: 0.2 },
  anchors: [
    { kind: "door", at: [0, 0, 2.5], face: 0, size: [1.1, 2.2] },
    { kind: "stairs", at: [0, 0, -1.2], to: [0, 12.15, 0.2], face: 180 },
  ],
};

export function sdf(x, y, z, k) {
  const shaft = box(x, y - 6.0, z, 2.5, 6.0, 2.5);
  const inside = box(x, y - 5.9, z, 2.1, 5.9, 2.1);
  const door = box(x, y - 1.1, z - 2.5, 0.55, 1.1, 0.6);
  const slits = min(box(x, y - 5.5, z, 0.12, 0.6, 3.0), box(x, y - 8.5, z, 3.0, 0.6, 0.12));
  const walls = max(shaft, -min(min(inside, door), slits));
  // The lookout's floor, with the ladder's opening at the back.
  const floor0 = max(box(x, y - 12.05, z, 2.7, 0.1, 2.7), -box(x, y - 12.05, z + 1.7, 0.55, 0.3, 0.45));
  const parapet = max(box(x, y - 12.65, z, 2.7, 0.55, 2.7), -box(x, y - 12.8, z, 2.45, 0.8, 2.45));
  const posts = box(abs(x) - 2.5, y - 13.9, abs(z) - 2.5, 0.16, 1.8, 0.16);
  const roof = max((max(abs(x), abs(z)) - 3.0 + (y - 15.6) * 1.3) * 0.6, max(15.6 - y, y - 16.6));
  const eave = box(x, y - 15.6, z, 3.0, 0.08, 3.0);
  // The ladder: two rails and rungs 0.3 m apart up the back wall.
  const rails = box(abs(x) - 0.3, y - 6.1, z + 1.95, 0.04, 6.1, 0.04);
  const rungs = max(box(x, y - 0.3 * floor(y / 0.3) - 0.15, z + 1.95, 0.3, 0.025, 0.025), y - 12.1);
  return min(min(min(walls, floor0), min(parapet, posts)), min(min(roof, eave) , min(rails, rungs)));
}

export function color(x, y, z, k) {
  if (y > 15.5) return mix(rgb(176, 78, 44), rgb(206, 104, 60), noise3(x * 2, y * 2, z * 2));
  if (y > 13.2 || (abs(x) < 2.0 && abs(z + 1.95) < 0.1 && y < 12.1)) return rgb(120, 86, 54);
  const n = noise3(x * 1.6, y * 1.6, z * 1.6);
  const course = fract(y * 2.2) < 0.08 ? 0.86 : 1;
  const joint = fract(x * 1.1 + floor(y * 2.2) * 0.5) < 0.05 ? 0.88 : 1;
  const stone = mix(rgb(196, 182, 152), rgb(222, 208, 178), n) * course * joint;
  return stone;
}
