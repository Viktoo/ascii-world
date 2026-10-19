// The mayor's house at the head of the green: the biggest home in Maple Cove,
// still one storey, white clapboard with black shutters under a plum roof, a
// wide porch with columns, a parlour with a hearth, a study and a bedroom
// walled off behind. Its door faces +z.
export const meta = {
  name: "mayor's house",
  bounds: [6.50, 8.22, 7.75],
  tags: ['building', 'house', 'shop'],
  props: { mass: 60000, burns: 0.5, light: 1 },
  sound: { hard: 0.6, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "door", at: [0.0, 0, 4.8], face: 0, size: [1.1, 2.2] },
    { kind: "light", at: [0.90, 2.3, 4.95] },
    { kind: "light", at: [0, 2.8, 0] },
    { kind: "light", at: [-3.8, 0.4, -4.00] },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 1.65, z, 6.0, 1.65, 4.8);
  const rooms = box(x, y - 1.550, z, 5.8, 1.550, 4.6);
  const door = box(x - (0.0), y - 1.1, z - 4.8, 0.55, 1.1, 0.4);
  const wins = min(min(box(x - (-3.6), y - 1.45, z, 0.45, 0.62, 5.1) , box(x - (-1.8), y - 1.45, z, 0.45, 0.62, 5.1) , box(x - (1.8), y - 1.45, z, 0.45, 0.62, 5.1) , box(x - (3.6), y - 1.45, z, 0.45, 0.62, 5.1)), box(abs(x) - 6.0, y - 1.45, z, 0.4, 0.62, 0.45));
  const shell = max(body, -min(min(rooms, door), wins));
  const floor0 = box(x, y - 0.025, z, 5.81, 0.025, 4.609999999999999);
  const glass = min(box(abs(x) - 5.94, y - 1.45, z, 0.02, 0.6, 0.42), min(box(x - (-3.6), y - 1.45, abs(z) - 4.72, 0.45, 0.62, 0.02) , box(x - (-1.8), y - 1.45, abs(z) - 4.72, 0.45, 0.62, 0.02) , box(x - (1.8), y - 1.45, abs(z) - 4.72, 0.45, 0.62, 0.02) , box(x - (3.6), y - 1.45, abs(z) - 4.72, 0.45, 0.62, 0.02)));
  // The roof: a steep shell over the gable walls, the gables clapboard.
  const pl = ((y - 7.420) + abs(z) * 0.8) * 0.7809;
  const roof = max(max(abs(pl + 0.09) - 0.09, abs(x) - 6.35), max(3.0999999999999996 - y, abs(z) - 5.1499999999999995));
  const gable = max(pl, max(abs(x) - 6.0, 3.25 - y));
  const chim = box(x + 3.30, y - 5.71, z + 0.5, 0.32, 2.41, 0.32);
  const inner = min(min(10.0, max(box(x - (0.0), y - 1.550, z - (-0.6), 5.8, 1.550, 0.08), -box(x - (-1.0), y - 1.05, z - (-0.6), 0.5, 1.05, 0.3))), max(box(x - (1.4), y - 1.550, z - (-2.5999999999999996), 0.08, 1.550, 1.9999999999999998), -box(x - (1.4), y - 1.05, z - (-2.0), 0.3, 1.05, 0.5)));
  const hearth = max(box(x - (-3.8), y - 0.85, z + 4.30, 0.8, 0.85, 0.35), -box(x - (-3.8), y - 0.45, z + 4.15, 0.5, 0.42, 0.3));
  const porch = min(min(min(box(x, y - 0.1, z - 6.00, 6.0, 0.1, 1.20), box(abs(x) - 5.85, y - 1.65, z - 7.05, 0.08, 1.65, 0.08)), box(x, y - (3.22 - (z - 4.8) * 0.22), z - 6.10, 6.15, 0.06, 1.45)), box(abs(x) - 2.00, y - 1.65, z - 7.05, 0.08, 1.65, 0.08));
  return min(min(min(shell, floor0), min(glass, roof)), min(min(gable, chim), min(min(inner, hearth), porch)));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  let wall = rgb(244, 242, 236);
  const cwall = floor(hash(k.seed, 4) * 1);
  let roof = rgb(124, 56, 104);
  const croof = floor(hash(k.seed, 5) * 1);
  let shut = rgb(40, 60, 46);
  const cshut = floor(hash(k.seed, 6) * 4);
  if (cshut == 1) shut = rgb(34, 40, 60);
  if (cshut == 2) shut = rgb(30, 30, 34);
  if (cshut == 3) shut = rgb(120, 40, 36);
  const pl = ((y - 7.420) + abs(z) * 0.8) * 0.7809;
  if (abs(x + 3.30) < 0.34 && abs(z + 0.5) < 0.34 && y > 3.5) return y > 7.97 ? rgb(60, 54, 50) : mix(rgb(140, 64, 48), rgb(166, 84, 60), noise3(x * 8, y * 8, z * 8));
  if (pl > -0.2 && y > 3.0999999999999996 && (abs(x) > 5.99 || pl > -0.17)) return roof * (fract((y - abs(z) * 0.8) * 3.0) < 0.12 ? 0.82 : 1.0) * (0.92 + 0.08 * n);
  if (z > 4.819999999999999 && y < 0.21) return fract(x * 5.0) < 0.08 ? rgb(110, 90, 70) : rgb(170, 146, 116);
  if (z > 4.85 && y > 2.70) return roof * 0.85;
  if (z > 4.85) return rgb(244, 242, 236);
  const inside = abs(x) < 5.819999999999999 && abs(z) < 4.619999999999999;
  if (abs(x - (-3.8)) < 0.82 && z < -3.93 && y < 1.72 && abs(z) < 4.8) {
    if (abs(x - (-3.8)) < 0.5 && y < 0.6 && y > 0.04) return glow(rgb(255, 150, 60), 0.9);
    return mix(rgb(130, 60, 44), rgb(160, 80, 56), noise3(x * 6, y * 6, z * 6)) * (fract(y * 8.0) < 0.12 ? 0.8 : 1.0);
  }
  if (inside) {
    // Wide pine floorboards, whitewashed walls, beams overhead.
    if (y < 0.07) return fract(x * 3.0) < 0.06 ? rgb(110, 80, 52) : mix(rgb(176, 130, 84), rgb(196, 150, 100), noise3(x * 3, 1, z * 12));
    if (y > 2.8499999999999996) return fract(x * 0.7) < 0.12 ? rgb(110, 80, 56) : rgb(236, 228, 210);
    return rgb(240, 234, 220);
  }
  // Glass: warm with lamplight.
  if (abs(y - 1.45) < 0.62 && ((abs(abs(x) - 5.94) < 0.04 && abs(z) < 0.44) || abs(abs(z) - 4.72) < 0.04)) return glow(mix(rgb(90, 76, 60), rgb(255, 200, 120), 0.65), 0.75);
  // Window frames and shutters.
  const fw = abs(z) > 4.77 || abs(x) > 5.97;
  if (fw && abs(y - 1.45) < 0.72) {
    const wx = abs(z) > 4.77 ? x : z;
    const near = abs(abs(wx - (-3.6)) - 0.45) < 0.07 || abs(abs(wx - (-1.8)) - 0.45) < 0.07 || abs(abs(wx - (1.8)) - 0.45) < 0.07 || abs(abs(wx - (3.6)) - 0.45) < 0.07 || (abs(x) > 5.97 && abs(abs(z) - 0.45) < 0.07);
    if (near) return rgb(246, 244, 238);
    const sh = abs(abs(wx - (-3.6)) - 0.7) < 0.18 || abs(abs(wx - (-1.8)) - 0.7) < 0.18 || abs(abs(wx - (1.8)) - 0.7) < 0.18 || abs(abs(wx - (3.6)) - 0.7) < 0.18 || (abs(x) > 5.97 && abs(abs(z) - 0.7) < 0.18);
    if (sh && abs(y - 1.45) < 0.64) return shut * (fract(y * 10.0) < 0.3 ? 0.8 : 1.0);
  }
  if (abs(z - 4.8) < 0.06 && abs(x - (0.0)) < 0.67 && y < 2.32) return rgb(246, 244, 238);
  if (y < 0.3) return mix(rgb(120, 118, 112), rgb(150, 146, 138), noise3(x * 4, y * 4, z * 4));
  // Corner boards, then the clapboard.
  if (abs(abs(x) - 6.0) < 0.14 && abs(abs(z) - 4.8) < 0.14) return rgb(246, 244, 238);
  return wall * (fract(y * 5.0) < 0.14 ? 0.86 : 1.0) * (0.95 + 0.05 * n);
}
