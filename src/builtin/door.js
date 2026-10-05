// Built-in door: hung in a doorway by the engine, hinged at its left edge
// (the origin), swinging inwards. k.a × 2 = width, k.b × 4 = height (m);
// k.c shifts the wood's hue, k.d its shade.
export const meta = {
  name: "door",
  bounds: [2.2, 3.4, 0.14],
  tags: ["builtin", "door", "fixture"],
  props: { mass: 30, burns: 0.5, fuel: 0.6 },
  sound: { hard: 0.55, dry: 0.7, ring: 0.05 },
  joint: { axis: "y", at: [0, 0, 0], open: 100 },
};

export function sdf(x, y, z, k) {
  const w = clamp(k.a * 2, 0.4, 2.2);
  const h = clamp(k.b * 4, 0.6, 3.4);
  const leaf = box(x - w * 0.5, y - h * 0.5, z, w * 0.5 - 0.02, h * 0.5 - 0.02, 0.04);
  const knob = sphere(x - (w - 0.12), y - h * 0.48, abs(z) - 0.07, 0.035);
  return min(leaf, knob);
}

export function color(x, y, z, k) {
  const w = clamp(k.a * 2, 0.4, 2.2);
  const h = clamp(k.b * 4, 0.6, 3.4);
  if (abs(z) > 0.045 && length2(x - (w - 0.12), y - h * 0.48) < 0.06) return rgb(70, 60, 44);
  const base = hsv(0.07 + (k.c - 0.5) * 0.08, 0.55, 0.3 + k.d * 0.3);
  const seam = step(0.92, fract(x * 4 + hash(k.seed) * 0.3));
  const rail = step(abs(y - h * 0.25), 0.05) + step(abs(y - h * 0.75), 0.05);
  return mix(base, base * 0.7, clamp(seam + rail, 0, 1));
}
