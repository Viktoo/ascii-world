// Built-in lamppost: stands along roads and lights them at night.
export const meta = {
  name: "lamppost",
  bounds: [0.32, 1.85, 0.32],
  tags: ["builtin", "lamppost", "prop"],
  props: { mass: 120, light: 1 },
  sound: { hard: 0.9, dry: 0.5, ring: 0.5 },
};

export function sdf(x, y, z, k) {
  const pole = cylinder(x, y + 0.15, z, 0.055, 1.55);
  const foot = cappedCone(x, y + 1.7, z, 0.13, 0.17, 0.08);
  const lamp = roundBox(x, y - 1.52, z, 0.14, 0.17, 0.14, 0.03);
  const cap = cappedCone(x, y - 1.76, z, 0.07, 0.2, 0.03);
  return min(pole, foot, lamp, cap);
}

export function color(x, y, z, k) {
  if (y > 1.36 && y < 1.68 && max(abs(x), abs(z)) < 0.135) return glow(rgb(255, 214, 150));
  return mix(rgb(38, 40, 44), rgb(60, 62, 66), noise3(x * 6, y * 6, z * 6));
}
