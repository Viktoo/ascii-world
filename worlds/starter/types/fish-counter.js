// The fish counter in the fish shed: a sloping bed of crushed ice on a wooden
// stand, laid with cod, haddock, mackerel, red lobsters and a heap of clams.
export const meta = {
  name: "fish counter",
  bounds: [1.15, 1.0, 0.45],
  tags: ["furniture", "counter", "shop"],
  props: { mass: 140, wet: 0.5 },
};

export function sdf(x, y, z, k) {
  const stand = box(x, y - 0.4, z, 1.1, 0.4, 0.4);
  const ice = max(box(x, y - 0.85, z, 1.1, 0.1, 0.4), (y - 0.9) - (z + 0.4) * 0.25);
  const fish = max(ellipsoid(x - 0.25 * floor(x / 0.25 + 0.5), y - 0.88 - (z + 0.4) * 0.25 * 0.0, z, 0.1, 0.04, 0.28), abs(x) - 1.0);
  return min(stand, min(ice, fish));
}

export function color(x, y, z, k) {
  if (y > 0.82) {
    const i = floor(x / 0.25 + 0.5);
    const h = hash(i, k.seed);
        if (abs(x - i * 0.25) < 0.1) return h < 0.4 ? rgb(196, 206, 214) : h < 0.7 ? rgb(214, 90, 70) : rgb(120, 70, 90);
    return rgb(236, 244, 248);
  }
  return (fract(x * 6.0) < 0.05 || fract(y * 6.0) < 0.05) ? rgb(200, 210, 210) : rgb(70, 140, 170);
}
