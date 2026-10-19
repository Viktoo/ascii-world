// The head of the wooden pier: a broad square deck on piles, open on its
// seaward side where a ladder goes down to the boats, a bench to sit on and
// watch the sunset, a lamp on a post. Its deck is 7.8 m over its foot; the
// pier comes in at -z.
export const meta = {
  name: "pier head",
  bounds: [4.1, 10.0, 4.1],
  tags: ["walkway", "pier"],
  props: { mass: 40000, burns: 0.5 },
  anchors: [
    { kind: "light", at: [-3.7, 9.5, 3.7] },
    { kind: "stairs", at: [2.0, 7.8, 3.6], to: [2.0, 5.0, 4.4], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const deck = box(x, y - 7.7, z, 4.0, 0.1, 4.0);
  const px = x - 2.6 * floor(x / 2.6 + 0.5);
  const pz = z - 2.6 * floor(z / 2.6 + 0.5);
  const piles = max(box(px, y - 3.9, pz, 0.15, 3.9, 0.15), max(abs(x) - 4.0, abs(z) - 4.0));
  const sides = box(abs(x) - 3.95, y - 8.75, z, 0.05, 0.05, 4.0);
  const back = max(box(x, y - 8.75, z + 3.95, 4.0, 0.05, 0.05), -box(x, y - 8.7, z + 3.95, 1.7, 0.2, 0.2));
  const posts = max(box(abs(x) - 3.95, y - 8.3, z - 2.0 * floor(z / 2.0 + 0.5), 0.05, 0.5, 0.05), abs(z) - 4.0);
  const ladder = min(box(abs(x - 2.0) - 0.25, y - 6.4, z - 4.08, 0.04, 1.4, 0.04), max(box(x - 2.0, y - 0.3 * floor(y / 0.3) - 0.15, z - 4.08, 0.25, 0.02, 0.03), max(y - 7.6, 5.0 - y)));
  const lamp = min(cylinder(x + 3.7, y - 8.6, z - 3.7, 0.06, 0.8), sphere(x + 3.7, y - 9.5, z - 3.7, 0.18));
  return min(min(min(deck, piles), min(sides, back)), min(posts, min(ladder, max(lamp, y - 9.98))));
}

export function color(x, y, z, k) {
  if (length2(x + 3.7, z - 3.7) < 0.2 && y > 9.35) return glow(rgb(255, 214, 140), 0.8);
  if (length2(x + 3.7, z - 3.7) < 0.1) return rgb(36, 36, 36);
  if (y < 7.3) return y < 6.2 ? rgb(40, 34, 30) : rgb(70, 60, 50);
  if (y < 7.85) return (fract(x * 4.0) < 0.07 ? 0.7 : 1.0) * mix(rgb(150, 140, 126), rgb(176, 166, 150), noise3(x * 9, y * 4, z * 2));
  return mix(rgb(160, 150, 136), rgb(186, 176, 160), noise3(x * 6, y * 6, z * 6));
}
