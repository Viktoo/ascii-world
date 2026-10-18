export const meta = {
  name: "basement house", bounds: [3.4, 3.2, 3.4], tags: ["building"], in_ground: 0,
  anchors: [
    { kind: "door", at: [0, 0, 3], face: 0, size: [1.1, 2.1] },
    { kind: "stairs", at: [-1.9, -2.6, 2.1], to: [-1.9, 0, -1.8], face: 180 },
    { kind: "bed", at: [1.5, -2.6, -1.5] },
  ],
};
export function sdf(x, y, z, k) {
  // A ground floor and a basement under it, one stair between.
  const outer = box(x, y - 0, z, 3, 2.8, 3);
  const upstairs = box(x, y - 1.15, z, 2.75, 1.15, 2.75);
  const downstairs = box(x, y + 1.45, z, 2.75, 1.15, 2.75);
  const door = box(x, y - 1.05, z - 3, 0.55, 1.05, 0.4);
  const well = box(x + 1.9, y + 0.15, z - 0.25, 0.6, 0.3, 1.75);
  let shape = subtract(subtract(subtract(subtract(outer, upstairs), downstairs), door), well);
  // Steps down along the west wall, from the basement floor (z = 1.6) up to the ground floor (z = -1.6).
  for (let i = 0; i < 13; i++) {
    shape = min(shape, box(x + 1.9, y - (-2.5 + i * 0.1), z - (1.6 - i * 0.25), 0.55, 0.1 + i * 0.1, 0.13));
  }
  return shape;
}
export function color(x, y, z, k) { return rgb(190, 170, 140); }
