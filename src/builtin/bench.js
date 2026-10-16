// Built-in bench: two people sit on it (its seat anchors), facing +z.
// k.c shifts the wood's hue, k.d its shade.
export const meta = {
  name: "bench",
  bounds: [0.95, 0.48, 0.32],
  tags: ["builtin", "bench", "furniture", "prop"],
  props: { mass: 35, burns: 0.5, fuel: 0.6 },
  sound: { hard: 0.6, dry: 0.7, ring: 0.05 },
  anchors: [
    { kind: "seat", at: [-0.42, -0.02, 0.04], face: 0 },
    { kind: "seat", at: [0.42, -0.02, 0.04], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const seat = roundBox(x, y + 0.04, z - 0.04, 0.92, 0.035, 0.2, 0.015);
  const back = roundBox(x, y - 0.26, z + 0.22, 0.92, 0.12, 0.025, 0.012);
  const lx = abs(x) - 0.78;
  const legs = min(box(lx, y + 0.27, z - 0.04, 0.035, 0.2, 0.17), box(lx, y - 0.1, z + 0.22, 0.035, 0.38, 0.03));
  return min(seat, back, legs);
}

export function color(x, y, z, k) {
  const base = hsv(0.07 + (k.c - 0.5) * 0.06, 0.5, 0.32 + k.d * 0.3);
  const grain = noise3(x * 8, y * 30, z * 8);
  if (abs(x) > 0.74 && y < 0.0) return rgb(46, 46, 50);
  return mix(base, base * 0.75, grain * 0.6);
}
