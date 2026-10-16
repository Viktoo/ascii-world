// Built-in boat: a small rowing boat, front +z. One rows from the stern
// seat; two more sit forward. k.c: hull hue, k.d: shade.
export const meta = {
  name: "boat",
  bounds: [0.75, 0.45, 2.3],
  tags: ["builtin", "boat", "vehicle", "water"],
  props: { mass: 120, burns: 0.4, fuel: 0.5 },
  sound: { hard: 0.5, dry: 0.6, ring: 0.1 },
  drive: { speed: 3, on: "water", by: "oars" },
  anchors: [
    { kind: "seat", at: [0, -0.12, -1.2], face: 0 },
    { kind: "seat", at: [0, -0.12, 0.0], face: 0 },
    { kind: "seat", at: [0, -0.12, 1.1], face: 180 },
  ],
};

export function sdf(x, y, z, k) {
  // A hull narrowing to the bow, hollowed out above its floor.
  const t = clamp(z / 2.3, -1, 1);
  const w = 0.72 * sqrt(max(1 - t * t * t * t, 0.02)) * (t > 0 ? 1 - 0.45 * t * t : 1);
  const lift = 0.12 * t * t;
  const hull = max(length2(x / max(w, 0.05), (y + 0.05 - lift) / 0.42) - 1, y - 0.3 - lift, abs(z) - 2.28) * min(w, 0.42);
  const inside = max(length2(x / max(w - 0.06, 0.04), (y + 0.0 - lift) / 0.36) - 1, -0.24 - y, abs(z) - 2.15) * min(w - 0.06, 0.36);
  const shell = max(hull, -inside);
  const thwarts = box(x, y + 0.14, abs(z + 0.05) - 1.12, w * 0.95, 0.025, 0.12);
  const mid = box(x, y + 0.14, z, w * 0.95, 0.025, 0.12);
  return min(shell, thwarts, mid);
}

export function color(x, y, z, k) {
  const hue = hsv(0.03 + k.c * 0.6, 0.55, 0.35 + k.d * 0.3);
  const wood = mix(rgb(120, 86, 54), rgb(150, 112, 72), noise3(x * 6, y * 20, z * 6));
  if (y > 0.18) return hue;
  if (y < -0.26 && abs(x) < 0.5) return wood * 0.8;
  return y > -0.05 ? wood : mix(hue, hue * 0.6, 0.5);
}
