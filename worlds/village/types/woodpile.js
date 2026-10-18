// A woodpile: split chestnut logs stacked neatly against two posts, their
// pale cut ends out, under a little lean-to roof of planks.
export const meta = {
  name: "woodpile",
  bounds: [1.7, 1.75, 0.65],
  tags: ["prop"],
  props: { mass: 600, burns: 0.9, fuel: 1 },
  sound: { hard: 0.5, dry: 0.9, ring: 0.05 },
};

export function sdf(x, y, z, k) {
  const pile = box(x, y - 0.6, z, 1.45, 0.6, 0.42);
  const top = ellipsoid(x, y - 1.2, z, 1.4, 0.18, 0.4);
  const posts = box(abs(x) - 1.55, y - 0.8, z + 0.45, 0.06, 0.8, 0.06);
  const roof = box(x, y - 1.65 + z * 0.25, z, 1.7, 0.04, 0.62);
  return min(min(pile, top), min(posts, roof));
}

export function color(x, y, z, k) {
  if (y > 1.4) return rgb(96, 76, 56);
  if (abs(z) > 0.38) {
    const cx = x * 3.6 + floor(y * 3.6) * 0.5;
    const r = length2(fract(cx) - 0.5, fract(y * 3.6) - 0.5);
    return r < 0.38 ? mix(rgb(214, 176, 120), rgb(190, 146, 96), r * 2) : rgb(84, 60, 42);
  }
  return mix(rgb(100, 72, 48), rgb(124, 92, 60), noise3(x * 5, y * 5, z * 5));
}
