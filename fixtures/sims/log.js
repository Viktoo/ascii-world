// A felled log: too heavy for one person.
export const meta = { name: "log", bounds: [1.5, 0.22, 0.22], tags: ["wood"], props: { mass: 40, burns: 0.6 } };

export function sdf(x, y, z, k) {
  return capsule(x, y, z, -1.3, 0, 0, 1.3, 0, 0, 0.2);
}

export function color(x, y, z, k) {
  return rgb(110, 80, 50);
}
