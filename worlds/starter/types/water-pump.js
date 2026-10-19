// The village pump on the green: a cast-iron hand pump with a long handle
// over a granite trough, cold, sweet well water for anyone.
export const meta = {
  name: "water pump",
  bounds: [0.5, 1.4, 0.75],
  tags: ["prop", "well", "source"],
  props: { mass: 300, wet: 1 },
  sound: { hard: 1, dry: 0.3, ring: 0.5 },
};

export function sdf(x, y, z, k) {
  const col = cylinder(x, y - 0.6, z, 0.09, 0.6);
  const head = cylinder(x, y - 1.25, z, 0.11, 0.07);
  const spout = capsule(x, y, z, 0, 0.95, 0, 0, 0.9, 0.22, 0.035);
  const handle = capsule(x, y, z, 0, 1.25, 0, 0, 1.38, -0.45, 0.025);
  const trough = max(box(x, y - 0.25, z - 0.5, 0.45, 0.25, 0.25), -box(x, y - 0.35, z - 0.5, 0.38, 0.2, 0.18));
  return min(min(col, head), min(min(spout, handle), trough));
}

export function color(x, y, z, k) {
  if (abs(z - 0.5) < 0.26 && y < 0.5) return y > 0.4 && abs(x) < 0.38 && abs(z - 0.5) < 0.18 ? rgb(70, 110, 130) : rgb(150, 148, 140);
  return rgb(40, 50, 46);
}
