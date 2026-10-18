export const meta = {
  name: "earth house", bounds: [3.4, 2.8, 3.4], tags: ["building"], in_ground: 3,
  anchors: [
    { kind: "door", at: [0, 0, 3], face: 0, size: [1.1, 2.1] },
    { kind: "bed", at: [-1.5, 0, -1.5] },
  ],
};
export function sdf(x, y, z, k) {
  const outer = box(x, y - 1.2, z, 3, 1.4, 3);
  const room = box(x, y - 1.15, z, 2.75, 1.15, 2.75);
  const door = box(x, y - 1.05, z - 3, 0.55, 1.05, 0.4);
  return subtract(subtract(outer, room), door);
}
export function color(x, y, z, k) { return rgb(170, 140, 100); }
