// A wooden lobster boat of Maple Cove: 5.6 m, its stem post standing up at
// the bow, painted (white, sea green, buff, red or navy) with a contrasting
// band along the gunwale and a small engine housing amidships. Its front is +z.
export const meta = {
  name: "fishing boat",
  bounds: [1.05, 0.95, 2.95],
  tags: ["boat", "vehicle", "water"],
  props: { mass: 700, burns: 0.4, fuel: 0.5 },
  sound: { hard: 0.5, dry: 0.6, ring: 0.1 },
  drive: { speed: 4, on: "water", by: "engine" },
  anchors: [
    { kind: "seat", at: [0, -0.1, -1.7], face: 0 },
    { kind: "seat", at: [0, -0.1, 1.1], face: 0 },
    { kind: "seat", at: [0, -0.1, 0.3], face: 180 },
  ],
};

export function sdf(x, y, z, k) {
  const t = clamp(z / 2.8, -1, 1);
  const w = 0.98 * sqrt(max(1 - t * t, 0.02));
  const lift = 0.16 * t * t;
  const hull = max(length2(x / max(w, 0.05), (y + 0.05 - lift) / 0.55) - 1, y - 0.36 - lift, abs(z) - 2.78) * min(w, 0.5);
  const inside = max(length2(x / max(w - 0.08, 0.04), (y - lift) / 0.47) - 1, -0.3 - y, abs(z) - 2.6) * min(w - 0.08, 0.45);
  const shell = max(hull, -inside);
  const stem = box(x, y - 0.55, z - 2.74, 0.05, 0.3, 0.1);
  const engine = box(x, y + 0.1, z + 0.6, 0.32, 0.22, 0.35);
  const thwarts = box(x, y + 0.14, abs(z - 0.3) - 0.8, w * 0.92, 0.03, 0.13);
  return min(min(shell, stem), min(engine, thwarts));
}

export function color(x, y, z, k) {
  const c = floor(hash(k.seed) * 5);
  let hue = rgb(240, 238, 230);
  let band = rgb(40, 60, 100);
  if (c == 1) { hue = rgb(96, 150, 130); band = rgb(240, 236, 226); }
  if (c == 2) { hue = rgb(222, 206, 160); band = rgb(150, 50, 40); }
  if (c == 3) { hue = rgb(170, 52, 40); band = rgb(240, 236, 226); }
  if (c == 4) { hue = rgb(36, 54, 92); band = rgb(220, 190, 90); }
  const t = clamp(z / 2.8, -1, 1);
  const lift = 0.16 * t * t;
  const wood = mix(rgb(150, 110, 70), rgb(176, 134, 88), noise3(x * 6, y * 20, z * 6));
  if (abs(z + 0.6) < 0.36 && abs(x) < 0.34 && y > -0.2) return rgb(60, 62, 66);
  if (z > 2.6 && y > 0.25) return band;
  if (abs(x) < 0.85 && y < 0.2 && y > -0.35 && abs(z) < 2.55) return wood;
  if (abs(y - 0.26 - lift) < 0.06) return band;
  if (y < -0.32 + lift) return rgb(150, 50, 40);
  return hue;
}
