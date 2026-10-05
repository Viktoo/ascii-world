// A small rowing boat: a water vehicle for tests.
export const meta = {
  name: "rowing boat", bounds: [0.8, 0.45, 1.8], tags: ["prop", "water"], props: { mass: 90, burns: 0.5 },
  anchors: [{ kind: "seat", at: [0, 0.3, 0], face: 0 }],
  drive: { speed: 3, on: "water" },
};

export function sdf(x, y, z, k) {
  const hull = box(x, y - 0.2, z, 0.7, 0.2, 1.7);
  const inside = box(x, y - 0.35, z, 0.6, 0.2, 1.6);
  return max(hull, -inside);
}

export function color(x, y, z, k) {
  return rgb(120, 80, 50);
}
