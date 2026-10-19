// The Anchor, the tavern on the landing: a long single storey of grey clapboard
// with a red roof, a deep porch along its front looking out over the pier and
// the sunset, a big stone hearth inside, a bar, barrels and long tables. Its
// door faces +z.
export const meta = {
  name: "tavern",
  bounds: [6.70, 7.96, 7.55],
  tags: ['building', 'house', 'shop'],
  props: { mass: 60000, burns: 0.5, light: 1 },
  sound: { hard: 0.6, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "door", at: [1.0, 0, 4.6], face: 0, size: [1.4, 2.2] },
    { kind: "light", at: [2.05, 2.3, 4.75] },
    { kind: "light", at: [0, 2.7, 0] },
    { kind: "light", at: [-3.5, 0.4, -3.80] },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 1.6, z, 6.2, 1.6, 4.6);
  const rooms = box(x, y - 1.500, z, 6.0, 1.500, 4.3999999999999995);
  const door = box(x - (1.0), y - 1.1, z - 4.6, 0.7, 1.1, 0.4);
  const wins = min(min(box(x - (-3.6), y - 1.45, z, 0.45, 0.62, 4.8999999999999995) , box(x - (4.0), y - 1.45, z, 0.45, 0.62, 4.8999999999999995)), box(abs(x) - 6.2, y - 1.45, z, 0.4, 0.62, 0.45));
  const shell = max(body, -min(min(rooms, door), wins));
  const floor0 = box(x, y - 0.025, z, 6.01, 0.025, 4.409999999999999);
  const glass = min(box(abs(x) - 6.14, y - 1.45, z, 0.02, 0.6, 0.42), min(box(x - (-3.6), y - 1.45, abs(z) - 4.52, 0.45, 0.62, 0.02) , box(x - (4.0), y - 1.45, abs(z) - 4.52, 0.45, 0.62, 0.02)));
  // The roof: a steep shell over the gable walls, the gables clapboard.
  const pl = ((y - 7.160) + abs(z) * 0.8) * 0.7809;
  const roof = max(max(abs(pl + 0.09) - 0.09, abs(x) - 6.55), max(3.0 - y, abs(z) - 4.949999999999999));
  const gable = max(pl, max(abs(x) - 6.2, 3.1500000000000004 - y));
  const chim = box(x + 3.41, y - 5.53, z + 0.5, 0.32, 2.33, 0.32);
  const inner = 10.0;
  const hearth = max(box(x - (-3.5), y - 0.85, z + 4.10, 0.8, 0.85, 0.35), -box(x - (-3.5), y - 0.45, z + 3.95, 0.5, 0.42, 0.3));
  const porch = min(min(min(box(x, y - 0.1, z - 5.80, 6.2, 0.1, 1.20), box(abs(x) - 6.05, y - 1.60, z - 6.85, 0.08, 1.60, 0.08)), box(x, y - (3.12 - (z - 4.6) * 0.22), z - 5.90, 6.35, 0.06, 1.45)), box(abs(x) - 2.07, y - 1.60, z - 6.85, 0.08, 1.60, 0.08));
  return min(min(min(shell, floor0), min(glass, roof)), min(min(gable, chim), min(min(inner, hearth), porch)));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  let wall = rgb(186, 186, 180);
  const cwall = floor(hash(k.seed, 4) * 1);
  let roof = rgb(176, 44, 38);
  const croof = floor(hash(k.seed, 5) * 1);
  let shut = rgb(40, 60, 46);
  const cshut = floor(hash(k.seed, 6) * 4);
  if (cshut == 1) shut = rgb(34, 40, 60);
  if (cshut == 2) shut = rgb(30, 30, 34);
  if (cshut == 3) shut = rgb(120, 40, 36);
  const pl = ((y - 7.160) + abs(z) * 0.8) * 0.7809;
  if (abs(x + 3.41) < 0.34 && abs(z + 0.5) < 0.34 && y > 3.4000000000000004) return y > 7.71 ? rgb(60, 54, 50) : mix(rgb(140, 64, 48), rgb(166, 84, 60), noise3(x * 8, y * 8, z * 8));
  if (pl > -0.2 && y > 3.0 && (abs(x) > 6.19 || pl > -0.17)) return roof * (fract((y - abs(z) * 0.8) * 3.0) < 0.12 ? 0.82 : 1.0) * (0.92 + 0.08 * n);
  if (z > 4.619999999999999 && y < 0.21) return fract(x * 5.0) < 0.08 ? rgb(110, 90, 70) : rgb(170, 146, 116);
  if (z > 4.6499999999999995 && y > 2.60) return roof * 0.85;
  if (z > 4.6499999999999995) return rgb(244, 242, 236);
  const inside = abs(x) < 6.02 && abs(z) < 4.419999999999999;
  if (abs(x - (-3.5)) < 0.82 && z < -3.73 && y < 1.72 && abs(z) < 4.6) {
    if (abs(x - (-3.5)) < 0.5 && y < 0.6 && y > 0.04) return glow(rgb(255, 150, 60), 0.9);
    return mix(rgb(130, 60, 44), rgb(160, 80, 56), noise3(x * 6, y * 6, z * 6)) * (fract(y * 8.0) < 0.12 ? 0.8 : 1.0);
  }
  if (inside) {
    // Wide pine floorboards, whitewashed walls, beams overhead.
    if (y < 0.07) return fract(x * 3.0) < 0.06 ? rgb(110, 80, 52) : mix(rgb(176, 130, 84), rgb(196, 150, 100), noise3(x * 3, 1, z * 12));
    if (y > 2.75) return fract(x * 0.7) < 0.12 ? rgb(110, 80, 56) : rgb(236, 228, 210);
    return rgb(240, 234, 220);
  }
  // Glass: warm with lamplight.
  if (abs(y - 1.45) < 0.62 && ((abs(abs(x) - 6.14) < 0.04 && abs(z) < 0.44) || abs(abs(z) - 4.52) < 0.04)) return glow(mix(rgb(90, 76, 60), rgb(255, 200, 120), 0.65), 0.75);
  // Window frames and shutters.
  const fw = abs(z) > 4.569999999999999 || abs(x) > 6.17;
  if (fw && abs(y - 1.45) < 0.72) {
    const wx = abs(z) > 4.569999999999999 ? x : z;
    const near = abs(abs(wx - (-3.6)) - 0.45) < 0.07 || abs(abs(wx - (4.0)) - 0.45) < 0.07 || (abs(x) > 6.17 && abs(abs(z) - 0.45) < 0.07);
    if (near) return rgb(246, 244, 238);
    const sh = abs(abs(wx - (-3.6)) - 0.7) < 0.18 || abs(abs(wx - (4.0)) - 0.7) < 0.18 || (abs(x) > 6.17 && abs(abs(z) - 0.7) < 0.18);
    if (sh && abs(y - 1.45) < 0.64) return shut * (fract(y * 10.0) < 0.3 ? 0.8 : 1.0);
  }
  if (abs(z - 4.6) < 0.06 && abs(x - (1.0)) < 0.82 && y < 2.32) return rgb(246, 244, 238);
  if (y < 0.3) return mix(rgb(120, 118, 112), rgb(150, 146, 138), noise3(x * 4, y * 4, z * 4));
  // Corner boards, then the clapboard.
  if (abs(abs(x) - 6.2) < 0.14 && abs(abs(z) - 4.6) < 0.14) return rgb(246, 244, 238);
  return wall * (fract(y * 5.0) < 0.14 ? 0.86 : 1.0) * (0.95 + 0.05 * n);
}
