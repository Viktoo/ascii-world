// A glass jar of cloudy water with paintbrushes standing in it.
export const meta = {
  name: "jar of brushes",
  bounds: [0.06, 0.26, 0.06],
  tags: ["item", "art"],
  props: { mass: 0.5, fragile: 0.7, wet: 0.5 },
};

export function sdf(x, y, z, k) {
  const jar = cappedCone(x, y - 0.06, z, 0.06, 0.045, 0.04);
  let brushes = 10.0;
  for (let i = 0; i < 3; i++) {
    const a = i * 2.1;
    brushes = min(brushes, capsule(x, y, z, cos(a) * 0.01, 0.02, sin(a) * 0.01, cos(a) * 0.035, 0.24, sin(a) * 0.035, 0.005));
  }
  return min(jar, brushes);
}

export function color(x, y, z, k) {
  if (y > 0.12) return y > 0.21 ? rgb(40, 30, 26) : rgb(170, 60, 40);
  return y > 0.09 ? rgb(200, 220, 230) : rgb(150, 160, 170);
}
