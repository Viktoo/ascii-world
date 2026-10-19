// The vet's examining table: a steel top on a white stand, a cabinet of
// medicines and bandages beside it, a weighing scale for dogs.
export const meta = {
  name: "vet table",
  bounds: [1.2, 1.0, 0.45],
  tags: ["furniture", "shop"],
  props: { mass: 90 },
};

export function sdf(x, y, z, k) {
  const top = box(x, y - 0.85, z, 0.6, 0.03, 0.35);
  const stand = box(x, y - 0.42, z, 0.5, 0.42, 0.3);
  const cab = box(x - 0.95, y - 0.5, z, 0.25, 0.5, 0.25);
  return min(min(top, stand), cab);
}

export function color(x, y, z, k) {
  if (y > 0.8 && abs(x) < 0.6) return rgb(196, 200, 206);
  if (x < -0.7) return fract(y * 4.0) < 0.06 ? rgb(60, 60, 60) : rgb(236, 240, 240);
  return rgb(246, 246, 246);
}
