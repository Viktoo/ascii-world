// A little deer whittled from birch wood, smooth and pale, small enough to hold in one hand.
export const meta = {
  name: "carving",
  bounds: [0.05, 0.11, 0.07],
  tags: ["item", "toy"],
  props: { mass: 0.1, burns: 0.9 },
};

export function sdf(x, y, z, k) {
  const b = ellipsoid(x, y - 0.06, z, 0.025, 0.022, 0.045);
  const legs = box(abs(x) - 0.012, y - 0.02, abs(z) - 0.03, 0.006, 0.02, 0.006);
  const head = sphere(x, y - 0.09, z - 0.05, 0.016);
  return min(min(b, legs), head);
}

export function color(x, y, z, k) {
  return mix(rgb(220, 200, 160), rgb(236, 220, 186), noise3(x * 40, y * 40, z * 40));
}
