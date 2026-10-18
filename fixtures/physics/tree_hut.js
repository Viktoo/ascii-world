export const meta = {
  name: "tree hut", bounds: [1.7, 2.5, 1.7], tags: ["building"],
  anchors: [{ kind: "door", at: [0, 0, 1.5], face: 0, size: [0.9, 1.9] }, { kind: "bed", at: [0, 0, -0.6] }],
};
export function sdf(x, y, z, k) {
  const outer = box(x, y - 1.15, z, 1.5, 1.25, 1.5);
  const room = box(x, y - 1.1, z, 1.3, 1.05, 1.3);
  const door = box(x, y - 0.98, z - 1.5, 0.45, 0.98, 0.4);
  return subtract(subtract(outer, room), door);
}
export function color(x, y, z, k) { return rgb(150, 120, 80); }
