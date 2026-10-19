// A long driftwood log, bleached silver-grey by the sea and smooth from
// years of waves, washed up on the sand: a seat for three by the fire ring.
export const meta = {
  name: "driftwood log",
  bounds: [1.6, 0.5, 0.35],
  tags: ["prop", "seat", "wood"],
  props: { mass: 120, burns: 0.8, fuel: 0.8 },
  anchors: [
    { kind: "seat", at: [-0.8, 0.44, 0], face: 0 },
    { kind: "seat", at: [0, 0.44, 0], face: 0 },
    { kind: "seat", at: [0.8, 0.44, 0], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const log = capsule(x, y, z, -1.4, 0.24, 0.0, 1.4, 0.2, 0.05, 0.22 - abs(x) * 0.03);
  const knot = capsule(x, y, z, 0.6, 0.3, 0.05, 0.85, 0.45, 0.25, 0.05);
  return min(log, knot);
}

export function color(x, y, z, k) {
  return mix(rgb(170, 164, 152), rgb(206, 200, 188), noise3(x * 3, y * 20, z * 20));
}
