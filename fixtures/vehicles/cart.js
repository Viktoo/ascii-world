// A hand cart with a bench seat: a land vehicle for tests.
export const meta = {
  name: "hand cart", bounds: [0.9, 0.9, 1.4], tags: ["prop"], props: { mass: 120, burns: 0.5 },
  anchors: [{ kind: "seat", at: [0, 0.75, -0.4], face: 0 }],
  drive: { speed: 6, on: "land" },
};

export function sdf(x, y, z, k) {
  const bed = box(x, y - 0.55, z, 0.8, 0.12, 1.3);
  const wheels = box(abs(x) - 0.82, y - 0.35, z, 0.06, 0.35, 0.35);
  return min(bed, wheels);
}

export function color(x, y, z, k) {
  return y < 0.45 ? rgb(70, 50, 30) : rgb(150, 110, 70);
}
