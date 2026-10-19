// The little schoolhouse: one room of barn-red clapboard under a white roof
// with a bell in a cupola on its ridge, rows of desks and a blackboard inside.
// Its door faces +z.
export const meta = {
  name: "schoolhouse",
  bounds: [4.70, 7.60, 3.95],
  tags: ['building', 'school'],
  props: { mass: 60000, burns: 0.5, light: 1 },
  sound: { hard: 0.6, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "door", at: [0.0, 0, 3.4], face: 0, size: [1.1, 2.2] },
    { kind: "light", at: [0.90, 2.3, 3.55] },
    { kind: "light", at: [0, 2.7, 0] },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 1.6, z, 4.2, 1.6, 3.4);
  const rooms = box(x, y - 1.500, z, 4.0, 1.500, 3.1999999999999997);
  const door = box(x - (0.0), y - 1.1, z - 3.4, 0.55, 1.1, 0.4);
  const wins = min(min(box(x - (-2.4), y - 1.45, z, 0.45, 0.62, 3.6999999999999997) , box(x - (2.4), y - 1.45, z, 0.45, 0.62, 3.6999999999999997)), box(abs(x) - 4.2, y - 1.45, z, 0.4, 0.62, 0.45));
  const shell = max(body, -min(min(rooms, door), wins));
  const floor0 = box(x, y - 0.025, z, 4.01, 0.025, 3.2099999999999995);
  const glass = min(box(abs(x) - 4.14, y - 1.45, z, 0.02, 0.6, 0.42), min(box(x - (-2.4), y - 1.45, abs(z) - 3.32, 0.45, 0.62, 0.02) , box(x - (2.4), y - 1.45, abs(z) - 3.32, 0.45, 0.62, 0.02)));
  // The roof: a steep shell over the gable walls, the gables clapboard.
  const pl = ((y - 6.200) + abs(z) * 0.8) * 0.7809;
  const roof = max(max(abs(pl + 0.09) - 0.09, abs(x) - 4.55), max(3.0 - y, abs(z) - 3.75));
  const gable = max(pl, max(abs(x) - 4.2, 3.1500000000000004 - y));
  const chim = box(x + 2.31, y - 5.05, z + 0.5, 0.32, 1.85, 0.32);
  const inner = 10.0;
  const hearth = 10.0;
  const porch = 10.0;
  const cup = min(box(x, y - 6.45, z, 0.5, 0.5, 0.5), max(sphere(x, y - 6.95, z, 0.55), 6.95 - y));
  return min(min(min(shell, floor0), min(glass, roof)), min(min(gable, chim), min(min(inner, hearth), porch)));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  let wall = rgb(156, 48, 38);
  const cwall = floor(hash(k.seed, 4) * 1);
  let roof = rgb(232, 230, 222);
  const croof = floor(hash(k.seed, 5) * 1);
  let shut = rgb(40, 60, 46);
  const cshut = floor(hash(k.seed, 6) * 4);
  if (cshut == 1) shut = rgb(34, 40, 60);
  if (cshut == 2) shut = rgb(30, 30, 34);
  if (cshut == 3) shut = rgb(120, 40, 36);
  if (y > 5.9 && abs(x) < 0.66 && abs(z) < 0.66) return y > 6.95 ? rgb(232, 230, 222) : (abs(y - 6.45) < 0.25 && abs(x) < 0.25 ? rgb(196, 160, 60) : rgb(244, 242, 236));
  const pl = ((y - 6.200) + abs(z) * 0.8) * 0.7809;
  if (abs(x + 2.31) < 0.34 && abs(z + 0.5) < 0.34 && y > 3.4000000000000004) return y > 6.75 ? rgb(60, 54, 50) : mix(rgb(140, 64, 48), rgb(166, 84, 60), noise3(x * 8, y * 8, z * 8));
  if (pl > -0.2 && y > 3.0 && (abs(x) > 4.19 || pl > -0.17)) return roof * (fract((y - abs(z) * 0.8) * 3.0) < 0.12 ? 0.82 : 1.0) * (0.92 + 0.08 * n);

  const inside = abs(x) < 4.02 && abs(z) < 3.2199999999999998;

  if (inside) {
    // Wide pine floorboards, whitewashed walls, beams overhead.
    if (y < 0.07) return fract(x * 3.0) < 0.06 ? rgb(110, 80, 52) : mix(rgb(176, 130, 84), rgb(196, 150, 100), noise3(x * 3, 1, z * 12));
    if (y > 2.75) return fract(x * 0.7) < 0.12 ? rgb(110, 80, 56) : rgb(236, 228, 210);
    return rgb(240, 234, 220);
  }
  // Glass: warm with lamplight.
  if (abs(y - 1.45) < 0.62 && ((abs(abs(x) - 4.14) < 0.04 && abs(z) < 0.44) || abs(abs(z) - 3.32) < 0.04)) return glow(mix(rgb(90, 76, 60), rgb(255, 200, 120), 0.65), 0.75);
  // Window frames and shutters.
  const fw = abs(z) > 3.37 || abs(x) > 4.17;
  if (fw && abs(y - 1.45) < 0.72) {
    const wx = abs(z) > 3.37 ? x : z;
    const near = abs(abs(wx - (-2.4)) - 0.45) < 0.07 || abs(abs(wx - (2.4)) - 0.45) < 0.07 || (abs(x) > 4.17 && abs(abs(z) - 0.45) < 0.07);
    if (near) return rgb(246, 244, 238);
    const sh = abs(abs(wx - (-2.4)) - 0.7) < 0.18 || abs(abs(wx - (2.4)) - 0.7) < 0.18 || (abs(x) > 4.17 && abs(abs(z) - 0.7) < 0.18);
    if (sh && abs(y - 1.45) < 0.64) return shut * (fract(y * 10.0) < 0.3 ? 0.8 : 1.0);
  }
  if (abs(z - 3.4) < 0.06 && abs(x - (0.0)) < 0.67 && y < 2.32) return rgb(246, 244, 238);
  if (y < 0.3) return mix(rgb(120, 118, 112), rgb(150, 146, 138), noise3(x * 4, y * 4, z * 4));
  // Corner boards, then the clapboard.
  if (abs(abs(x) - 4.2) < 0.14 && abs(abs(z) - 3.4) < 0.14) return rgb(246, 244, 238);
  return wall * (fract(y * 5.0) < 0.14 ? 0.86 : 1.0) * (0.95 + 0.05 * n);
}
