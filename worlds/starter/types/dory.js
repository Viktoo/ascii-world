// A wooden dory, the fishermen's rowing boat: 4.5 m, flat-bottomed with high
// flared sides, buff inside, painted green, red or white outside, two thwarts
// and a pair of oars. Its front is +z.
export const meta = {
  name: "dory",
  bounds: [0.85, 0.75, 2.3],
  tags: ["boat", "vehicle", "water"],
  props: { mass: 200, burns: 0.4, fuel: 0.5 },
  anchors: [
    { kind: "seat", at: [0, 0.2, -1.1], face: 0 },
    { kind: "seat", at: [0, 0.2, 0], face: 0 },
  ],
  drive: { speed: 2.5, on: "water", by: "oars" },
};

export function sdf(x, y, z, k) {
  const t = clamp(z / 2.25, -1, 1);
  const w = 0.78 * sqrt(max(1 - t * t * t * t, 0.02)) * (0.55 + 0.45 * clamp((y + 0.2) / 0.6, 0, 1));
  const lift = 0.25 * t * t;
  const hull = max(max(abs(x) - w, abs(z) - 2.25), max(-0.2 - y, y - 0.4 - lift));
  const inside = max(max(abs(x) - w + 0.05, abs(z) - 2.15), -0.12 - y);
  const thwarts = box(x, y - 0.18, z - 1.1 * floor(z / 1.1 + 0.5), 0.7, 0.025, 0.12);
  return min(max(hull, -inside), max(thwarts, abs(z) - 1.5));
}

export function color(x, y, z, k) {
  const c = floor(hash(k.seed) * 3);
  let hue = rgb(70, 120, 80);
  if (c == 1) hue = rgb(170, 52, 40);
  if (c == 2) hue = rgb(240, 238, 230);
  const t = clamp(z / 2.25, -1, 1);
  const w = 0.78 * sqrt(max(1 - t * t * t * t, 0.02)) * (0.55 + 0.45 * clamp((y + 0.2) / 0.6, 0, 1));
  if (abs(x) < w - 0.03 && y > -0.13) return rgb(214, 196, 150);
  return y < -0.1 ? rgb(140, 50, 40) : hue;
}
