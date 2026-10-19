// Six metres of the wooden pier: a deck of grey weathered planks on stout
// tarred piles driven into the cove's floor, a rail along each side. Its
// deck is 7.8 m over its foot; it runs along z.
export const meta = {
  name: "pier section",
  bounds: [1.75, 8.9, 3.02],
  tags: ["walkway", "pier"],
  props: { mass: 20000, burns: 0.5 },
};

export function sdf(x, y, z, k) {
  const deck = box(x, y - 7.7, z, 1.6, 0.1, 3.0);
  const piles = box(abs(x) - 1.45, y - 3.9, z - 3.0 * floor(z / 3.0 + 0.5), 0.14, 3.9, 0.14);
  const cap = box(x, y - 7.45, z - 3.0 * floor(z / 3.0 + 0.5), 1.62, 0.12, 0.12);
  const rail = box(abs(x) - 1.55, y - 8.75, z, 0.05, 0.05, 3.0);
  const posts = box(abs(x) - 1.55, y - 8.3, z - 1.5 * floor(z / 1.5 + 0.5), 0.05, 0.5, 0.05);
  return min(min(deck, piles), min(cap, min(rail, posts)));
}

export function color(x, y, z, k) {
  if (y < 7.3) return y < 6.2 ? rgb(40, 34, 30) : rgb(70, 60, 50);
  if (y < 7.85) return (fract(z * 4.0) < 0.07 ? 0.7 : 1.0) * mix(rgb(150, 140, 126), rgb(176, 166, 150), noise3(x * 2, y * 4, z * 9));
  return mix(rgb(160, 150, 136), rgb(186, 176, 160), noise3(x * 6, y * 6, z * 6));
}
