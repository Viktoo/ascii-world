export const meta = {
  name: "test hut", bounds: [3.2, 2.8, 3.2], tags: ["building"],
  anchors: [
    { kind: "door", at: [0, 0, 3], face: 0, size: [1.1, 2.1] },
    { kind: "bed", at: [-1.5, 0, -1.5] },
    { kind: "slot", at: [1.6, 0.9, -2.2], holds: "test sword" },
  ],
};
export function sdf(x, y, z, k) {
  const outer = box(x, y - 1.2, z, 3, 1.4, 3);
  const room = box(x, y - 1.15, z, 2.75, 1.15, 2.75);
  const door = box(x, y - 1.05, z - 3, 0.55, 1.05, 0.4);
  const shelf = box(x - 1.6, y - 0.8, z + 2.2, 0.5, 0.1, 0.3);
  return min(subtract(subtract(outer, room), door), shelf);
}
export function color(x, y, z, k) { return rgb(180, 150, 110); }
