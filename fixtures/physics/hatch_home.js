export const meta = {
  name: "hatch home", bounds: [3.2, 2.9, 3.2], tags: ["building"], in_ground: 0,
  anchors: [
    { kind: "hatch", at: [2.1, 2.7, 0], face: 270, size: [1.0, 1.0] },
    { kind: "stairs", at: [1.8, 0, 0], to: [3.1, 2.7, 0], face: 90 },
    { kind: "bed", at: [-1.5, 0, -1.5] },
  ],
};
export function sdf(x, y, z, k) {
  const outer = box(x, y - 1.3, z, 3, 1.4, 3);
  const room = box(x, y - 1.2, z, 2.75, 1.2, 2.75);
  const opening = box(x - 2.1, y - 2.55, z, 0.5, 0.3, 0.5);
  const shell = subtract(subtract(outer, room), opening);
  // A ladder against the east wall, under the hatch.
  let rungs = 1e9;
  for (let i = 0; i < 9; i++) {
    rungs = min(rungs, capsule(x - 2.6, y - (0.3 + i * 0.3), z, 0, 0, -0.3, 0, 0, 0.3, 0.03));
  }
  const rails = min(box(x - 2.6, y - 1.35, z - 0.3, 0.03, 1.35, 0.03), box(x - 2.6, y - 1.35, z + 0.3, 0.03, 1.35, 0.03));
  return min(shell, min(rails, rungs));
}
export function color(x, y, z, k) { return rgb(160, 130, 95); }
