// Three wicker lobster pots stacked by the quay, round-topped cages of
// brown cane bound with rope, a cork float tied to one.
export const meta = {
  name: "lobster pots",
  bounds: [0.9, 0.98, 0.5],
  tags: ["prop"],
  props: { mass: 15, burns: 0.8 },
  sound: { hard: 0.2, dry: 0.9, ring: 0.0 },
};

export function sdf(x, y, z, k) {
  const a = max(length3(x + 0.45, max(y - 0.22, 0), z) - 0.36, -y);
  const b = max(length3(x - 0.35, max(y - 0.22, 0), z - 0.05) - 0.34, -y);
  const c = max(length3(x + 0.05, max(y - 0.62, 0), z) - 0.3, 0.38 - y);
  const cork = sphere(x - 0.75, y - 0.1, z + 0.3, 0.1);
  return min(min(a, b), min(c, cork));
}

export function color(x, y, z, k) {
  if (length3(x - 0.75, y - 0.1, z + 0.3) < 0.13) return rgb(232, 90, 40);
  const weave = fract(atan2(z, x) * 3.0) < 0.3 || fract(y * 9.0) < 0.25;
  return weave ? rgb(110, 78, 46) : rgb(176, 132, 80);
}
