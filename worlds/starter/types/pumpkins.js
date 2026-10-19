// A heap of pumpkins and squash by a door or a porch step: big orange
// pumpkins, little white ones and warty green gourds.
export const meta = {
  name: "pumpkins",
  bounds: [0.9, 0.5, 0.6],
  tags: ["prop", "decoration"],
  props: { mass: 40, edible: 0.6, burns: 0.3 },
};

export function sdf(x, y, z, k) {
  const a = length3(x + 0.4, (y - 0.2) * 1.3, z) - 0.24;
  const b = length3(x - 0.15, (y - 0.17) * 1.3, z + 0.15) - 0.2;
  const c = length3(x - 0.55, (y - 0.12) * 1.3, z - 0.15) - 0.14;
  const d = length3(x + 0.05, (y - 0.42) * 1.3, z + 0.05) - 0.12;
  return min(min(a, b), min(c, d)) * 0.7;
}

export function color(x, y, z, k) {
  if (x > 0.38) return rgb(236, 230, 210);
  if (y > 0.33) return rgb(100, 120, 50);
  return mix(rgb(214, 100, 20), rgb(236, 130, 30), noise3(x * 10, y * 10, z * 10));
}
