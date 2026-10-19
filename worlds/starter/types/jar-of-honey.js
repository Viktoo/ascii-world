// A glass jar of golden wildflower honey from the hives at Pine Hollow, a cloth tied over its lid.
export const meta = {
  name: "jar of honey",
  bounds: [0.06, 0.13, 0.06],
  tags: ["item", "food"],
  props: { mass: 0.6, edible: 0.6, fragile: 0.7 },
};

export function sdf(x, y, z, k) {
  return min(cylinder(x, y - 0.055, z, 0.05, 0.055), cylinder(x, y - 0.12, z, 0.058, 0.012));
}

export function color(x, y, z, k) {
  if (y > 0.105) return rgb(220, 70, 60);
  return glow(rgb(226, 150, 30), 0.2);
}
