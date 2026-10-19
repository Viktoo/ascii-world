// The bakery: a single storey of rose-pink clapboard under a pumpkin-orange
// roof, its window stacked with loaves and pies, a brick oven glowing inside,
// the baker's rooms at the back. Its door faces +z.
export const meta = {
  name: "bakery",
  bounds: [4.90, 6.86, 4.15],
  tags: ['building', 'house', 'shop'],
  props: { mass: 60000, burns: 0.5, light: 1 },
  sound: { hard: 0.6, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "door", at: [0.6, 0, 3.6], face: 0, size: [1.1, 2.2] },
    { kind: "light", at: [1.50, 2.3, 3.75] },
    { kind: "light", at: [0, 2.4, 0] },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 1.45, z, 4.4, 1.45, 3.6);
  const rooms = box(x, y - 1.350, z, 4.2, 1.350, 3.4);
  const door = box(x - (0.6), y - 1.1, z - 3.6, 0.55, 1.1, 0.4);
  const wins = min(box(x - (-2.2), y - 1.45, z, 0.45, 0.62, 3.9), box(abs(x) - 4.4, y - 1.45, z, 0.4, 0.62, 0.45));
  const shell = max(body, -min(min(rooms, door), wins));
  const floor0 = box(x, y - 0.025, z, 4.21, 0.025, 3.4099999999999997);
  const glass = min(box(abs(x) - 4.34, y - 1.45, z, 0.02, 0.6, 0.42), box(x - (-2.2), y - 1.45, abs(z) - 3.52, 0.45, 0.62, 0.02));
  // The roof: a steep shell over the gable walls, the gables clapboard.
  const pl = ((y - 6.060) + abs(z) * 0.8) * 0.7809;
  const roof = max(max(abs(pl + 0.09) - 0.09, abs(x) - 4.75), max(2.6999999999999997 - y, abs(z) - 3.95));
  const gable = max(pl, max(abs(x) - 4.4, 2.85 - y));
  const chim = box(x + 2.42, y - 4.83, z + 0.5, 0.32, 1.93, 0.32);
  const inner = min(10.0, max(box(x - (0.0), y - 1.350, z - (-0.8), 4.2, 1.350, 0.08), -box(x - (2.6), y - 1.05, z - (-0.8), 0.5, 1.05, 0.3)));
  const hearth = 10.0;
  const porch = 10.0;
  return min(min(min(shell, floor0), min(glass, roof)), min(min(gable, chim), min(min(inner, hearth), porch)));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  let wall = rgb(226, 176, 166);
  const cwall = floor(hash(k.seed, 4) * 1);
  let roof = rgb(214, 104, 38);
  const croof = floor(hash(k.seed, 5) * 1);
  let shut = rgb(40, 60, 46);
  const cshut = floor(hash(k.seed, 6) * 4);
  if (cshut == 1) shut = rgb(34, 40, 60);
  if (cshut == 2) shut = rgb(30, 30, 34);
  if (cshut == 3) shut = rgb(120, 40, 36);
  const pl = ((y - 6.060) + abs(z) * 0.8) * 0.7809;
  if (abs(x + 2.42) < 0.34 && abs(z + 0.5) < 0.34 && y > 3.1) return y > 6.61 ? rgb(60, 54, 50) : mix(rgb(140, 64, 48), rgb(166, 84, 60), noise3(x * 8, y * 8, z * 8));
  if (pl > -0.2 && y > 2.6999999999999997 && (abs(x) > 4.390000000000001 || pl > -0.17)) return roof * (fract((y - abs(z) * 0.8) * 3.0) < 0.12 ? 0.82 : 1.0) * (0.92 + 0.08 * n);

  const inside = abs(x) < 4.22 && abs(z) < 3.42;

  if (inside) {
    // Wide pine floorboards, whitewashed walls, beams overhead.
    if (y < 0.07) return fract(x * 3.0) < 0.06 ? rgb(110, 80, 52) : mix(rgb(176, 130, 84), rgb(196, 150, 100), noise3(x * 3, 1, z * 12));
    if (y > 2.4499999999999997) return fract(x * 0.7) < 0.12 ? rgb(110, 80, 56) : rgb(236, 228, 210);
    return rgb(240, 234, 220);
  }
  // Glass: warm with lamplight.
  if (abs(y - 1.45) < 0.62 && ((abs(abs(x) - 4.34) < 0.04 && abs(z) < 0.44) || abs(abs(z) - 3.52) < 0.04)) return glow(mix(rgb(90, 76, 60), rgb(255, 200, 120), 0.65), 0.75);
  // Window frames and shutters.
  const fw = abs(z) > 3.5700000000000003 || abs(x) > 4.37;
  if (fw && abs(y - 1.45) < 0.72) {
    const wx = abs(z) > 3.5700000000000003 ? x : z;
    const near = abs(abs(wx - (-2.2)) - 0.45) < 0.07 || (abs(x) > 4.37 && abs(abs(z) - 0.45) < 0.07);
    if (near) return rgb(246, 244, 238);
    const sh = abs(abs(wx - (-2.2)) - 0.7) < 0.18 || (abs(x) > 4.37 && abs(abs(z) - 0.7) < 0.18);
    if (sh && abs(y - 1.45) < 0.64) return shut * (fract(y * 10.0) < 0.3 ? 0.8 : 1.0);
  }
  if (abs(z - 3.6) < 0.06 && abs(x - (0.6)) < 0.67 && y < 2.32) return rgb(246, 244, 238);
  if (y < 0.3) return mix(rgb(120, 118, 112), rgb(150, 146, 138), noise3(x * 4, y * 4, z * 4));
  // Corner boards, then the clapboard.
  if (abs(abs(x) - 4.4) < 0.14 && abs(abs(z) - 3.6) < 0.14) return rgb(246, 244, 238);
  return wall * (fract(y * 5.0) < 0.14 ? 0.86 : 1.0) * (0.95 + 0.05 * n);
}
