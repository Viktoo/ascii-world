// A round bale of golden hay, a pumpkin or two set against it.
export const meta = {
  name: "hay bale",
  bounds: [0.75, 1.2, 0.7],
  tags: ["prop"],
  props: { mass: 250, burns: 0.95, fuel: 0.5 },
};

export function sdf(x, y, z, k) {
  const bale = roundBox(x, y - 0.6, z, 0.6, 0.6, 0.6, 0.45);
  return min(bale, (length3(x - 0.3, (y - 0.15) * 1.3, z - 0.62) - 0.17) * 0.7);
}

export function color(x, y, z, k) {
  if (z > 0.5 && y < 0.35) return rgb(220, 110, 24);
  return mix(rgb(196, 160, 80), rgb(226, 196, 110), noise3(x * 14, y * 4, z * 14));
}
