// Built-in hatch: a lid hung flat in an opening in a floor, a roof or the
// ground by the engine, hinged along its back edge (the origin), the leaf
// running out along +z with its top at y = 0; it swings up to open.
// k.a × 2 = width (x), k.b × 2 = depth (z) (m); k.c 0 timber … 1 iron, k.d shade.
export const meta = {
  name: "hatch",
  bounds: [1.2, 0.1, 2.1],
  tags: ["builtin", "hatch", "door", "fixture"],
  props: { mass: 25, burns: 0.4, fuel: 0.5 },
  sound: { hard: 0.6, dry: 0.7, ring: 0.1 },
  joint: { axis: "x", at: [0, 0, 0], open: -105 },
};

export function sdf(x, y, z, k) {
  const w = clamp(k.a * 2, 0.4, 2.2);
  const d = clamp(k.b * 2, 0.4, 2);
  const leaf = box(x, y + 0.035, z - d * 0.5, w * 0.5 - 0.02, 0.035, d * 0.5 - 0.02);
  const ring = torus(x, y - 0.02, z - (d - 0.16), 0.06, 0.012);
  return min(leaf, ring);
}

export function color(x, y, z, k) {
  const w = clamp(k.a * 2, 0.4, 2.2);
  const d = clamp(k.b * 2, 0.4, 2);
  const n = noise3(x * 4, y * 4, z * 4);
  const wood = hsv(0.07, 0.5, 0.25 + k.d * 0.3) * (0.85 + n * 0.3);
  const plank = mix(wood, wood * 0.7, step(0.9, fract(x * 4)));
  const iron = rgb(70, 72, 76) * (0.8 + n * 0.4);
  if (y > 0.0) return rgb(60, 58, 54);
  const strap = step(abs(z - d * 0.25), 0.05) + step(abs(z - d * 0.75), 0.05);
  return k.c > 0.5 ? iron : mix(plank, iron, clamp(strap, 0, 1));
}
