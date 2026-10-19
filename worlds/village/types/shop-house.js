// A shop-house of Marinella: three floors of painted plaster (ochre, coral, rose, lemon, sky, mint, cream or apricot), a shop on the street with a wide window under a striped awning, green or blue shutters above, a terracotta roof and chimney.
// Inside: a shop on the ground floor; stairs up the back wall to the home
// over it (2 floors). Its door and shop window face the street (+z).
export const meta = {
  name: "shop house",
  bounds: [4.25, 12.1, 5.0],
  tags: ["building", "house", "shop"],
  props: { mass: 100000, burns: 0.25, light: 1 },
  sound: { hard: 0.9, dry: 0.8, ring: 0.1 },
  anchors: [
    { kind: "door", at: [1.9, 0, 4.4], face: 0, size: [1.1, 2.3] },
    { kind: "stairs", at: [-3.10, 0, -2.60], to: [0.62, 3.4, -3.65], face: 90 },
    { kind: "light", at: [1.9, 2.7, 4.7] },
    { kind: "stairs", at: [0.37, 3.4, -2.60], to: [3.15, 6.4, -2.55], face: 90 },
  ],
};

export function sdf(x, y, z, k) {
  // a: 14 steps up +x.
  const ua = (1 * (x - (-3.5))) / 0.2800;
  const ia = clamp(floor(ua), 0, 13);
  const sa = min(box(x - (-3.5) - 1 * (ia + 0.5) * 0.2800, y - (0.0) - ((ia + 1) * 0.2429) * 0.5, z - (-3.6500000000000004), 0.1400, ((ia + 1) * 0.2429) * 0.5, 0.500),
    box(x - (-3.5) - 1 * (clamp(ia + 1, 0, 13) + 0.5) * 0.2800, y - (0.0) - ((clamp(ia + 1, 0, 13) + 1) * 0.2429) * 0.5, z - (-3.6500000000000004), 0.1400, ((clamp(ia + 1, 0, 13) + 1) * 0.2429) * 0.5, 0.500));
  // b: 12 steps up +x.
  const ub = (1 * (x - (0.6700000000000004))) / 0.2400;
  const ib = clamp(floor(ub), 0, 11);
  const sb = min(box(x - (0.6700000000000004) - 1 * (ib + 0.5) * 0.2400, y - (3.4) - ((ib + 1) * 0.2500) * 0.5, z - (-3.6500000000000004), 0.1200, ((ib + 1) * 0.2500) * 0.5, 0.500),
    box(x - (0.6700000000000004) - 1 * (clamp(ib + 1, 0, 11) + 0.5) * 0.2400, y - (3.4) - ((clamp(ib + 1, 0, 11) + 1) * 0.2500) * 0.5, z - (-3.6500000000000004), 0.1200, ((clamp(ib + 1, 0, 11) + 1) * 0.2500) * 0.5, 0.500));
  const body = box(x, y - 4.7, z, 3.8, 4.7, 4.4);
  const rooms = box(x, y - 4.600, z, 3.55, 4.600, 4.15);
  const slabs = min(max(min(box(x, y - 3.3, z, 3.5599999999999996, 0.1, 4.16) , box(x, y - 6.300000000000001, z, 3.5599999999999996, 0.1, 4.16)), -min(box(x - (-1.070), y - 3.3, z - (-3.6500000000000004), 1.568, 0.3, 0.52), box(x - (2.398), y - 6.300000000000001, z - (-3.6500000000000004), 1.210, 0.3, 0.52))), box(x, y - 0.025, z, 3.5599999999999996, 0.025, 4.16));
  // Doorway and shop window on the street; windows above, front and back.
  const door = min(box(x - (1.9), y - 1.15, z - 4.4, 0.55, 1.15, 0.4), max(length2(x - (1.9), y - 2.3) - 0.55, abs(z - 4.4) - 0.4));
  const shopwin = box(x - (-1.5), y - 1.55, z - 4.4, 1.1, 0.85, 0.4);
  const wx = x - 1.9 * floor(x / 1.9 + 0.5);
  const fy = y - 3.4 - 3.0 * floor((y - 3.4) / 3.0);
  const win = max(max(box(wx, fy - 1.45, abs(z) - 4.4, 0.42, 0.7, 0.4), max(y - 9.1, 3.4 - y)), abs(x) - 3.3);
  const shell = max(body, -min(min(rooms, door), min(shopwin, win)));
  const glass = box(x - (-1.5), y - 1.55, z - 4.28, 1.1, 0.85, 0.03);
  const stairs = min(sa, sb);
  const sills = max(max(box(wx, fy - 0.68, abs(z) - 4.48, 0.5, 0.05, 0.1), abs(x) - 3.3), max(y - 9.1, 3.4 - y));
  const shutters = max(max(box(abs(wx) - 0.67, fy - 1.45, abs(z) - 4.45, 0.24, 0.7, 0.03), abs(x) - 3.5), max(y - 9.1, 3.4 - y));
  const awning = max(box(x - (-1.5), y - 2.75 + (z - 4.4) * 0.35, z - 5.0, 1.3, 0.03, 0.6), 4.4 - z);
  const roof = max(max(box(x, y - 10.3, z, 4.1499999999999995, 1.0, 4.800000000000001), (y - 11.15 + abs(z) * 0.42) * 0.9), 9.3 - y);
  const chim = box(x - 1.90, y - 10.9, z + 1.0, 0.32, 1.0, 0.32);
  return min(min(min(shell, slabs), min(glass, stairs)), min(min(sills, shutters), min(awning, min(roof, chim))));
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  if (y > 9.280000000000001) {
    if (abs(x + 1.90) < 0.36 && abs(z - 1.0) < 0.36) return y > 11.8 ? rgb(70, 60, 56) : rgb(226, 214, 196);
    return mix(rgb(176, 78, 44), rgb(206, 104, 60), n) * (fract(x * 3.6) < 0.18 ? 0.8 : 1);
  }
  const inside = abs(x) < 3.57 && abs(z) < 4.17;
  if (inside) {
    // Floors of terracotta tiles, plastered walls, wooden stairs.
    // Height over this storey's floor (a hit a hair under a floor counts as on it).
    const yy = y + 0.06;
    const ly = yy - 3.4 * step(3.4, yy) - 3.0 * max(floor((yy - 3.4) / 3.0), 0) * step(3.4, yy) - 0.06;
    if (z > -4.12 && z < -3.1000000000000005 && y < 9.0 && ly > 0.05) return mix(rgb(120, 84, 52), rgb(146, 104, 66), n);
    if (ly < 0.09) return (fract(x * 2.5) < 0.06 || fract(z * 2.5) < 0.06) ? rgb(150, 90, 62) : mix(rgb(196, 112, 76), rgb(214, 132, 92), n);
    return y < 3.4 ? rgb(236, 226, 206) : rgb(244, 236, 220);
  }
  if (abs(z - 4.28) < 0.05 && abs(x - (-1.5)) < 1.1 && abs(y - 1.55) < 0.85) return glow(mix(rgb(60, 72, 80), rgb(255, 220, 160), 0.35), 0.6);
  if (z > 4.45 && y > 2.5 && y < 3.0 && abs(x - (-1.5)) < 1.32) {
    const c = floor(hash(k.seed, 9) * 4);
    const st = fract(x * 1.6) < 0.5;
    if (c < 1) return st ? rgb(200, 40, 40) : rgb(246, 240, 226);
    if (c < 2) return st ? rgb(30, 100, 60) : rgb(246, 240, 226);
    if (c < 3) return st ? rgb(30, 70, 150) : rgb(246, 240, 226);
    return st ? rgb(220, 150, 40) : rgb(246, 240, 226);
  }
  const wx = x - 1.9 * floor(x / 1.9 + 0.5);
  const fy = y - 3.4 - 3.0 * floor((y - 3.4) / 3.0);
  if (abs(z) > 4.3500000000000005 && y > 3.4) {
    if (abs(abs(wx) - 0.67) < 0.25 && abs(fy - 1.45) < 0.72) return (hash(k.seed, 5) > 0.5 ? rgb(46, 104, 170) : rgb(52, 120, 82)) * (fract(fy * 9.0) < 0.3 ? 0.82 : 1);
    if (abs(fy - 0.68) < 0.06) return rgb(236, 232, 222);
  }
  if (y < 0.45) return mix(rgb(150, 140, 126), rgb(178, 168, 150), n);
  if (abs(y - 3.4) < 0.1 || (abs(abs(x) - 3.8) < 0.36 && abs(abs(z) - 4.4) < 0.36 && fract(y * 1.1) < 0.5)) return rgb(240, 236, 226);
  let wall = rgb(226, 170, 80);
  const c = floor(hash(k.seed, 4) * 8);
    if (c == 0) wall = rgb(226, 170, 80);
  if (c == 1) wall = rgb(238, 132, 100);
  if (c == 2) wall = rgb(234, 156, 152);
  if (c == 3) wall = rgb(240, 214, 112);
  if (c == 4) wall = rgb(152, 192, 222);
  if (c == 5) wall = rgb(164, 210, 176);
  if (c == 6) wall = rgb(242, 230, 204);
  if (c == 7) wall = rgb(248, 194, 146);
  return wall * (0.9 + 0.1 * n);
}
