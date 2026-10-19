// The boat shop on the landing: a tall single storey of barn-red clapboard
// under a white roof, wide doors to the water, inside a lobster boat on
// trestles, a workbench and the smell of fresh oak. Its door faces +z.
export const meta = {
  name: "boat shop",
  bounds: [5.50, 8.04, 4.75],
  tags: ['building', 'house', 'shop'],
  props: { mass: 60000, burns: 0.5, light: 1 },
  sound: { hard: 0.6, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "door", at: [0.0, 0, 4.2], face: 0, size: [2.4, 2.2] },
    { kind: "light", at: [1.55, 2.3, 4.3500000000000005] },
    { kind: "light", at: [0, 3.1, 0] },
  ],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 1.8, z, 5.0, 1.8, 4.2);
  const rooms = box(x, y - 1.700, z, 4.8, 1.700, 4.0);
  const door = box(x - (0.0), y - 1.1, z - 4.2, 1.2, 1.1, 0.4);
  const wins = min(min(box(x - (-3.2), y - 1.45, z, 0.45, 0.62, 4.5) , box(x - (3.2), y - 1.45, z, 0.45, 0.62, 4.5)), box(abs(x) - 5.0, y - 1.45, z, 0.4, 0.62, 0.45));
  const shell = max(body, -min(min(rooms, door), wins));
  const floor0 = box(x, y - 0.025, z, 4.81, 0.025, 4.01);
  const glass = min(box(abs(x) - 4.94, y - 1.45, z, 0.02, 0.6, 0.42), min(box(x - (-3.2), y - 1.45, abs(z) - 4.12, 0.45, 0.62, 0.02) , box(x - (3.2), y - 1.45, abs(z) - 4.12, 0.45, 0.62, 0.02)));
  // The roof: a steep shell over the gable walls, the gables clapboard.
  const pl = ((y - 7.240) + abs(z) * 0.8) * 0.7809;
  const roof = max(max(abs(pl + 0.09) - 0.09, abs(x) - 5.35), max(3.4 - y, abs(z) - 4.55));
  const gable = max(pl, max(abs(x) - 5.0, 3.5500000000000003 - y));
  const chim = 10.0;
  const inner = 10.0;
  const hearth = 10.0;
  const porch = 10.0;
  return min(min(min(shell, floor0), min(glass, roof)), min(min(gable, chim), min(min(inner, hearth), porch)));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  let wall = rgb(150, 50, 40);
  const cwall = floor(hash(k.seed, 4) * 1);
  let roof = rgb(232, 230, 222);
  const croof = floor(hash(k.seed, 5) * 1);
  let shut = rgb(40, 60, 46);
  const cshut = floor(hash(k.seed, 6) * 4);
  if (cshut == 1) shut = rgb(34, 40, 60);
  if (cshut == 2) shut = rgb(30, 30, 34);
  if (cshut == 3) shut = rgb(120, 40, 36);
  const pl = ((y - 7.240) + abs(z) * 0.8) * 0.7809;
  if (abs(x + 2.75) < 0.34 && abs(z + 0.5) < 0.34 && y > 3.8000000000000003) return y > 7.79 ? rgb(60, 54, 50) : mix(rgb(140, 64, 48), rgb(166, 84, 60), noise3(x * 8, y * 8, z * 8));
  if (pl > -0.2 && y > 3.4 && (abs(x) > 4.99 || pl > -0.17)) return roof * (fract((y - abs(z) * 0.8) * 3.0) < 0.12 ? 0.82 : 1.0) * (0.92 + 0.08 * n);

  const inside = abs(x) < 4.819999999999999 && abs(z) < 4.02;

  if (inside) {
    // Wide pine floorboards, whitewashed walls, beams overhead.
    if (y < 0.07) return fract(x * 3.0) < 0.06 ? rgb(110, 80, 52) : mix(rgb(176, 130, 84), rgb(196, 150, 100), noise3(x * 3, 1, z * 12));
    if (y > 3.15) return fract(x * 0.7) < 0.12 ? rgb(110, 80, 56) : rgb(236, 228, 210);
    return rgb(240, 234, 220);
  }
  // Glass: warm with lamplight.
  if (abs(y - 1.45) < 0.62 && ((abs(abs(x) - 4.94) < 0.04 && abs(z) < 0.44) || abs(abs(z) - 4.12) < 0.04)) return glow(mix(rgb(90, 76, 60), rgb(255, 200, 120), 0.65), 0.75);
  // Window frames and shutters.
  const fw = abs(z) > 4.17 || abs(x) > 4.97;
  if (fw && abs(y - 1.45) < 0.72) {
    const wx = abs(z) > 4.17 ? x : z;
    const near = abs(abs(wx - (-3.2)) - 0.45) < 0.07 || abs(abs(wx - (3.2)) - 0.45) < 0.07 || (abs(x) > 4.97 && abs(abs(z) - 0.45) < 0.07);
    if (near) return rgb(246, 244, 238);
    const sh = abs(abs(wx - (-3.2)) - 0.7) < 0.18 || abs(abs(wx - (3.2)) - 0.7) < 0.18 || (abs(x) > 4.97 && abs(abs(z) - 0.7) < 0.18);
    if (sh && abs(y - 1.45) < 0.64) return shut * (fract(y * 10.0) < 0.3 ? 0.8 : 1.0);
  }
  if (abs(z - 4.2) < 0.06 && abs(x - (0.0)) < 1.3199999999999998 && y < 2.32) return rgb(246, 244, 238);
  if (y < 0.3) return mix(rgb(120, 118, 112), rgb(150, 146, 138), noise3(x * 4, y * 4, z * 4));
  // Corner boards, then the clapboard.
  if (abs(abs(x) - 5.0) < 0.14 && abs(abs(z) - 4.2) < 0.14) return rgb(246, 244, 238);
  return wall * (fract(y * 5.0) < 0.14 ? 0.86 : 1.0) * (0.95 + 0.05 * n);
}
