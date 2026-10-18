export const meta = {
  name: "great tree", bounds: [4.2, 16.2, 4.6], tags: ["tree", "landmark"],
  anchors: [{ kind: "stairs", at: [0, 0, 4.3], to: [0, 8, 3.0], face: 180 }],
};
export function sdf(x, y, z, k) {
  const trunk = cylinder(x, y - 4, z, 1.2, 4);
  const deck = cylinder(x, y - 7.85, z, 3.5, 0.15);
  const crown = sphere(x, y - 13.6, z, 2.5);
  let rungs = 1e9;
  for (let i = 0; i < 26; i++) {
    rungs = min(rungs, capsule(x, y - (0.3 + i * 0.3), z - 3.65, -0.3, 0, 0, 0.3, 0, 0, 0.03));
  }
  const rails = min(box(x - 0.32, y - 4, z - 3.65, 0.03, 4, 0.03), box(x + 0.32, y - 4, z - 3.65, 0.03, 4, 0.03));
  return min(min(trunk, deck), min(crown, min(rungs, rails)));
}
export function color(x, y, z, k) { return y > 10 ? rgb(60, 110, 50) : rgb(110, 80, 55); }
