// Built-in bridge: carries a way over water or a cut. The deck runs along z,
// level with the ground at both ends (y = 0 there), rising a little in the
// middle, on an arch whose feet stand in the banks; low walls along its
// sides. k.a: span (4–24 m), k.b: width (1.5–6 m), k.c: 0 stone … 1 timber,
// k.d: shade.
export const meta = {
  name: "bridge",
  bounds: [3.1, 2.7, 12.2],
  tags: ["builtin", "bridge", "structure"],
  props: { mass: 40000 },
  sound: { hard: 0.95, dry: 0.6, ring: 0.1 },
};

export function sdf(x, y, z, k) {
  const L = 2 + clamp(k.a, 0, 1) * 10;
  const W = 0.75 + clamp(k.b, 0, 1) * 2.25;
  const u = clamp(z / L, -1, 1);
  const rise = min(L * 0.07, 0.9);
  const top = rise * (1 - u * u);
  const a = clamp(z / max(L - 0.7, 0.5), -1, 1);
  const arch = -2.6 + (2.2 + rise) * sqrt(max(1 - a * a, 0));
  const under = min(top - 0.45, arch);
  const deck = max(abs(x) - W, abs(z) - L, y - top, under - y) * 0.6;
  const wall = max(abs(abs(x) - W + 0.12) - 0.12, abs(z) - L, y - top - 0.75, top - 0.1 - y) * 0.6;
  return min(deck, wall);
}

export function color(x, y, z, k) {
  const L = 2 + clamp(k.a, 0, 1) * 10;
  const W = 0.75 + clamp(k.b, 0, 1) * 2.25;
  const n = noise3(x * 3, y * 3, z * 3);
  const stone = mix(rgb(176, 168, 150), rgb(214, 206, 188), n) * (0.75 + k.d * 0.4);
  const course = step(0.93, fract(y * 2.6)) + step(0.95, fract(z * 1.3 + floor(y * 2.6) * 0.5));
  const masonry = mix(stone, stone * 0.72, clamp(course, 0, 1));
  const wood = mix(rgb(104, 74, 46), rgb(132, 96, 60), n) * (0.75 + k.d * 0.4);
  const plank = mix(wood, wood * 0.7, step(0.9, fract(z * 3.3)));
  const timber = k.c > 0.5 ? plank : masonry;
  if (abs(x) > W - 0.26 && y > 0.05) return k.c > 0.5 ? wood * 0.85 : stone * 0.9;
  return timber;
}
