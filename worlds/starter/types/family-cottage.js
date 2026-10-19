// A family cottage of Maple Cove: a wider single storey of clapboard under a
// steep, brightly painted roof, a brick chimney and shutters, the bedroom
// walled off at the back. Its door faces +z.
export const meta = {
  name: "family cottage",
  bounds: [5.10, 6.70, 3.95],
  tags: ['building', 'house'],
  props: { mass: 60000, burns: 0.5, light: 1 },
  sound: { hard: 0.6, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "door", at: [0.6, 0, 3.4], face: 0, size: [1.1, 2.2] },
    { kind: "light", at: [1.50, 2.3, 3.55] },
    { kind: "light", at: [0, 2.4, 0] },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 1.45, z, 4.6, 1.45, 3.4);
  const rooms = box(x, y - 1.350, z, 4.3999999999999995, 1.350, 3.1999999999999997);
  const door = box(x - (0.6), y - 1.1, z - 3.4, 0.55, 1.1, 0.4);
  const wins = min(min(box(x - (-2.6), y - 1.45, z, 0.45, 0.62, 3.6999999999999997) , box(x - (2.8), y - 1.45, z, 0.45, 0.62, 3.6999999999999997)), box(abs(x) - 4.6, y - 1.45, z, 0.4, 0.62, 0.45));
  const shell = max(body, -min(min(rooms, door), wins));
  const floor0 = box(x, y - 0.025, z, 4.409999999999999, 0.025, 3.2099999999999995);
  const glass = min(box(abs(x) - 4.54, y - 1.45, z, 0.02, 0.6, 0.42), min(box(x - (-2.6), y - 1.45, abs(z) - 3.32, 0.45, 0.62, 0.02) , box(x - (2.8), y - 1.45, abs(z) - 3.32, 0.45, 0.62, 0.02)));
  // The roof: a steep shell over the gable walls, the gables clapboard.
  const pl = ((y - 5.900) + abs(z) * 0.8) * 0.7809;
  const roof = max(max(abs(pl + 0.09) - 0.09, abs(x) - 4.949999999999999), max(2.6999999999999997 - y, abs(z) - 3.75));
  const gable = max(pl, max(abs(x) - 4.6, 2.85 - y));
  const chim = box(x + 2.53, y - 4.75, z + 0.5, 0.32, 1.85, 0.32);
  const inner = min(10.0, max(box(x - (0.0), y - 1.350, z - (-0.6), 4.4, 1.350, 0.08), -box(x - (2.6), y - 1.05, z - (-0.6), 0.5, 1.05, 0.3)));
  const hearth = 10.0;
  const porch = 10.0;
  return min(min(min(shell, floor0), min(glass, roof)), min(min(gable, chim), min(min(inner, hearth), porch)));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  let wall = rgb(240, 238, 230);
  const cwall = floor(hash(k.seed, 4) * 7);
  if (cwall == 1) wall = rgb(236, 224, 196);
  if (cwall == 2) wall = rgb(196, 198, 196);
  if (cwall == 3) wall = rgb(176, 194, 206);
  if (cwall == 4) wall = rgb(172, 186, 152);
  if (cwall == 5) wall = rgb(236, 214, 150);
  if (cwall == 6) wall = rgb(150, 50, 40);
  let roof = rgb(186, 48, 40);
  const croof = floor(hash(k.seed, 5) * 8);
  if (croof == 1) roof = rgb(36, 130, 130);
  if (croof == 2) roof = rgb(46, 80, 160);
  if (croof == 3) roof = rgb(46, 110, 70);
  if (croof == 4) roof = rgb(214, 160, 40);
  if (croof == 5) roof = rgb(124, 56, 104);
  if (croof == 6) roof = rgb(214, 104, 38);
  if (croof == 7) roof = rgb(78, 150, 204);
  let shut = rgb(40, 60, 46);
  const cshut = floor(hash(k.seed, 6) * 4);
  if (cshut == 1) shut = rgb(34, 40, 60);
  if (cshut == 2) shut = rgb(30, 30, 34);
  if (cshut == 3) shut = rgb(120, 40, 36);
  const pl = ((y - 5.900) + abs(z) * 0.8) * 0.7809;
  if (abs(x + 2.53) < 0.34 && abs(z + 0.5) < 0.34 && y > 3.1) return y > 6.45 ? rgb(60, 54, 50) : mix(rgb(140, 64, 48), rgb(166, 84, 60), noise3(x * 8, y * 8, z * 8));
  if (pl > -0.2 && y > 2.6999999999999997 && (abs(x) > 4.59 || pl > -0.17)) return roof * (fract((y - abs(z) * 0.8) * 3.0) < 0.12 ? 0.82 : 1.0) * (0.92 + 0.08 * n);

  const inside = abs(x) < 4.419999999999999 && abs(z) < 3.2199999999999998;

  if (inside) {
    // Wide pine floorboards, whitewashed walls, beams overhead.
    if (y < 0.07) return fract(x * 3.0) < 0.06 ? rgb(110, 80, 52) : mix(rgb(176, 130, 84), rgb(196, 150, 100), noise3(x * 3, 1, z * 12));
    if (y > 2.4499999999999997) return fract(x * 0.7) < 0.12 ? rgb(110, 80, 56) : rgb(236, 228, 210);
    return rgb(240, 234, 220);
  }
  // Glass: warm with lamplight.
  if (abs(y - 1.45) < 0.62 && ((abs(abs(x) - 4.54) < 0.04 && abs(z) < 0.44) || abs(abs(z) - 3.32) < 0.04)) return glow(mix(rgb(90, 76, 60), rgb(255, 200, 120), 0.65), 0.75);
  // Window frames and shutters.
  const fw = abs(z) > 3.37 || abs(x) > 4.569999999999999;
  if (fw && abs(y - 1.45) < 0.72) {
    const wx = abs(z) > 3.37 ? x : z;
    const near = abs(abs(wx - (-2.6)) - 0.45) < 0.07 || abs(abs(wx - (2.8)) - 0.45) < 0.07 || (abs(x) > 4.569999999999999 && abs(abs(z) - 0.45) < 0.07);
    if (near) return rgb(246, 244, 238);
    const sh = abs(abs(wx - (-2.6)) - 0.7) < 0.18 || abs(abs(wx - (2.8)) - 0.7) < 0.18 || (abs(x) > 4.569999999999999 && abs(abs(z) - 0.7) < 0.18);
    if (sh && abs(y - 1.45) < 0.64) return shut * (fract(y * 10.0) < 0.3 ? 0.8 : 1.0);
  }
  if (abs(z - 3.4) < 0.06 && abs(x - (0.6)) < 0.67 && y < 2.32) return rgb(246, 244, 238);
  if (y < 0.3) return mix(rgb(120, 118, 112), rgb(150, 146, 138), noise3(x * 4, y * 4, z * 4));
  // Corner boards, then the clapboard.
  if (abs(abs(x) - 4.6) < 0.14 && abs(abs(z) - 3.4) < 0.14) return rgb(246, 244, 238);
  return wall * (fract(y * 5.0) < 0.14 ? 0.86 : 1.0) * (0.95 + 0.05 * n);
}
