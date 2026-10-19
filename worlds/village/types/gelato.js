// A gelato cone with two scoops, pistachio and strawberry, a little melting.
export const meta = {
  name: "gelato",
  bounds: [0.05, 0.22, 0.05],
  tags: ["item", "food"],
  props: { mass: 0.15, edible: 0.4 },
};

export function sdf(x, y, z, k) {
  return min(cappedCone(x, y - 0.06, z, 0.05, 0.005, 0.03), min(sphere(x, y - 0.13, z, 0.035), sphere(x, y - 0.18, z, 0.03)));
}

export function color(x, y, z, k) {
  if (y < 0.11) return rgb(210, 160, 90);
  return y < 0.155 ? rgb(170, 210, 120) : rgb(250, 170, 180);
}
