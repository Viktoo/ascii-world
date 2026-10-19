// The Albergo Marinella: a wide three-floor hotel of warm plaster on the piazza, its lobby on the ground floor behind a big window and an awning, guest rooms upstairs behind green shutters.
// Inside: a shop on the ground floor; stairs up the back wall to the home
// over it (2 floors). Its door and shop window face the street (+z).
export const meta = {
  name: "hotel",
  bounds: [7.05, 12.1, 5.3999999999999995],
  tags: ["building", "house", "shop"],
  props: { mass: 100000, burns: 0.25, light: 1 },
  sound: { hard: 0.9, dry: 0.8, ring: 0.1 },
  anchors: [
    { kind: "door", at: [2.6, 0, 4.8], face: 0, size: [1.1, 2.3] },
    { kind: "stairs", at: [-5.90, 0, -3.00], to: [-2.18, 3.4, -4.05], face: 90 },
    { kind: "light", at: [2.6, 2.7, 5.1] },
    { kind: "stairs", at: [-2.43, 3.4, -3.00], to: [5.95, 6.4, -2.95], face: 90 },
  ],
};

export function sdf(x, y, z, k) {
  // a: 14 steps up +x.
  const ua = (1 * (x - (-6.3))) / 0.2800;
  const ia = clamp(floor(ua), 0, 13);
  const sa = min(box(x - (-6.3) - 1 * (ia + 0.5) * 0.2800, y - (0.0) - ((ia + 1) * 0.2429) * 0.5, z - (-4.05), 0.1400, ((ia + 1) * 0.2429) * 0.5, 0.500),
    box(x - (-6.3) - 1 * (clamp(ia + 1, 0, 13) + 0.5) * 0.2800, y - (0.0) - ((clamp(ia + 1, 0, 13) + 1) * 0.2429) * 0.5, z - (-4.05), 0.1400, ((clamp(ia + 1, 0, 13) + 1) * 0.2429) * 0.5, 0.500));
  // b: 12 steps up +x.
  const ub = (1 * (x - (-2.1299999999999994))) / 0.7067;
  const ib = clamp(floor(ub), 0, 11);
  const sb = min(box(x - (-2.1299999999999994) - 1 * (ib + 0.5) * 0.7067, y - (3.4) - ((ib + 1) * 0.2500) * 0.5, z - (-4.05), 0.3533, ((ib + 1) * 0.2500) * 0.5, 0.500),
    box(x - (-2.1299999999999994) - 1 * (clamp(ib + 1, 0, 11) + 0.5) * 0.7067, y - (3.4) - ((clamp(ib + 1, 0, 11) + 1) * 0.2500) * 0.5, z - (-4.05), 0.3533, ((clamp(ib + 1, 0, 11) + 1) * 0.2500) * 0.5, 0.500));
  const body = box(x, y - 4.7, z, 6.6, 4.7, 4.8);
  const rooms = box(x, y - 4.600, z, 6.35, 4.600, 4.55);
  const slabs = min(max(min(box(x, y - 3.3, z, 6.359999999999999, 0.1, 4.56) , box(x, y - 6.300000000000001, z, 6.359999999999999, 0.1, 4.56)), -min(box(x - (-3.870), y - 3.3, z - (-4.05), 1.568, 0.3, 0.52), box(x - (2.958), y - 6.300000000000001, z - (-4.05), 3.562, 0.3, 0.52))), box(x, y - 0.025, z, 6.359999999999999, 0.025, 4.56));
  // Doorway and shop window on the street; windows above, front and back.
  const door = min(box(x - (2.6), y - 1.15, z - 4.8, 0.55, 1.15, 0.4), max(length2(x - (2.6), y - 2.3) - 0.55, abs(z - 4.8) - 0.4));
  const shopwin = box(x - (-2.6), y - 1.55, z - 4.8, 1.1, 0.85, 0.4);
  const wx = x - 1.9 * floor(x / 1.9 + 0.5);
  const fy = y - 3.4 - 3.0 * floor((y - 3.4) / 3.0);
  const win = max(max(box(wx, fy - 1.45, abs(z) - 4.8, 0.42, 0.7, 0.4), max(y - 9.1, 3.4 - y)), abs(x) - 6.1);
  const shell = max(body, -min(min(rooms, door), min(shopwin, win)));
  const glass = box(x - (-2.6), y - 1.55, z - 4.68, 1.1, 0.85, 0.03);
  const stairs = min(sa, sb);
  const sills = max(max(box(wx, fy - 0.68, abs(z) - 4.88, 0.5, 0.05, 0.1), abs(x) - 6.1), max(y - 9.1, 3.4 - y));
  const shutters = max(max(box(abs(wx) - 0.67, fy - 1.45, abs(z) - 4.85, 0.24, 0.7, 0.03), abs(x) - 6.3), max(y - 9.1, 3.4 - y));
  const awning = max(box(x - (-2.6), y - 2.75 + (z - 4.8) * 0.35, z - 5.3999999999999995, 1.3, 0.03, 0.6), 4.8 - z);
  const roof = max(max(box(x, y - 10.3, z, 6.949999999999999, 1.0, 5.2), (y - 11.15 + abs(z) * 0.42) * 0.9), 9.3 - y);
  const chim = box(x - 3.30, y - 10.9, z + 1.0, 0.32, 1.0, 0.32);
  return min(min(min(shell, slabs), min(glass, stairs)), min(min(sills, shutters), min(awning, min(roof, chim))));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  if (y > 9.280000000000001) {
    if (abs(x + 3.30) < 0.36 && abs(z - 1.0) < 0.36) return y > 11.8 ? rgb(70, 60, 56) : rgb(226, 214, 196);
    return mix(rgb(176, 78, 44), rgb(206, 104, 60), n) * (fract(x * 3.6) < 0.18 ? 0.8 : 1);
  }
  const inside = abs(x) < 6.369999999999999 && abs(z) < 4.569999999999999;
  if (inside) {
    // Floors of terracotta tiles, plastered walls, wooden stairs.
    // Height over this storey's floor (a hit a hair under a floor counts as on it).
    const yy = y + 0.06;
    const ly = yy - 3.4 * step(3.4, yy) - 3.0 * max(floor((yy - 3.4) / 3.0), 0) * step(3.4, yy) - 0.06;
    if (z > -4.52 && z < -3.5 && y < 9.0 && ly > 0.05) return mix(rgb(120, 84, 52), rgb(146, 104, 66), n);
    if (ly < 0.09) return (fract(x * 2.5) < 0.06 || fract(z * 2.5) < 0.06) ? rgb(150, 90, 62) : mix(rgb(196, 112, 76), rgb(214, 132, 92), n);
    return y < 3.4 ? rgb(236, 226, 206) : rgb(244, 236, 220);
  }
  if (abs(z - 4.68) < 0.05 && abs(x - (-2.6)) < 1.1 && abs(y - 1.55) < 0.85) return glow(mix(rgb(60, 72, 80), rgb(255, 220, 160), 0.35), 0.6);
  if (z > 4.85 && y > 2.5 && y < 3.0 && abs(x - (-2.6)) < 1.32) {
    const c = floor(hash(k.seed, 9) * 4);
    const st = fract(x * 1.6) < 0.5;
    if (c < 1) return st ? rgb(200, 40, 40) : rgb(246, 240, 226);
    if (c < 2) return st ? rgb(30, 100, 60) : rgb(246, 240, 226);
    if (c < 3) return st ? rgb(30, 70, 150) : rgb(246, 240, 226);
    return st ? rgb(220, 150, 40) : rgb(246, 240, 226);
  }
  const wx = x - 1.9 * floor(x / 1.9 + 0.5);
  const fy = y - 3.4 - 3.0 * floor((y - 3.4) / 3.0);
  if (abs(z) > 4.75 && y > 3.4) {
    if (abs(abs(wx) - 0.67) < 0.25 && abs(fy - 1.45) < 0.72) return (hash(k.seed, 5) > 0.5 ? rgb(46, 104, 170) : rgb(52, 120, 82)) * (fract(fy * 9.0) < 0.3 ? 0.82 : 1);
    if (abs(fy - 0.68) < 0.06) return rgb(236, 232, 222);
  }
  if (y < 0.45) return mix(rgb(150, 140, 126), rgb(178, 168, 150), n);
  if (abs(y - 3.4) < 0.1 || (abs(abs(x) - 6.6) < 0.36 && abs(abs(z) - 4.8) < 0.36 && fract(y * 1.1) < 0.5)) return rgb(240, 236, 226);
  let wall = rgb(226, 170, 80);
  const c = floor(hash(k.seed, 4) * 2);
    if (c == 0) wall = rgb(232, 190, 120);
  if (c == 1) wall = rgb(226, 160, 120);
  return wall * (0.9 + 0.1 * n);
}
