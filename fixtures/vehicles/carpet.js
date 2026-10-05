// A flying carpet: an air vehicle for tests.
export const meta = {
  name: "flying carpet", bounds: [0.9, 0.1, 1.4], tags: ["prop"], props: { mass: 8, burns: 1 },
  anchors: [{ kind: "seat", at: [0, 0.1, 0], face: 0 }],
  drive: { speed: 8, on: "air" },
};

export function sdf(x, y, z, k) {
  return box(x, y, z, 0.85, 0.04, 1.35);
}

export function color(x, y, z, k) {
  return rgb(150, 30, 40);
}
