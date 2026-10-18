// A palazzo on Marinella's piazza: three floors of painted plaster over an
// arcade of white stone arches, shops behind them (a café, a baker, the
// harbourmaster), green shutters, wrought-iron balconies on the first floor,
// a cornice and a low tiled roof. The arcade faces the piazza (+z).
export const meta = {
  name: "palazzo",
  bounds: [8.4, 13.4, 5.5],
  tags: ["building", "house", "shop"],
  props: { mass: 100000, burns: 0.2, light: 1 },
  sound: { hard: 0.9, dry: 0.8, ring: 0.15 },
  anchors: [
    { kind: "door", at: [-2.0, 0, 2.0], face: 0, size: [1.2, 2.3] },
    { kind: "door", at: [4.0, 0, 2.0], face: 0, size: [1.2, 2.3] },
    { kind: "light", at: [0, 3.4, 4.6] },
    { kind: "light", at: [-6, 3.4, 4.6] },
    { kind: "bed", at: [5.5, 0.05, -3.5] },
  ],
};

export function sdf(x, y, z, k) {
  const H = 11.6;
  const body = box(x, y - H * 0.5, z, 8.0, H * 0.5, 5.0);
  // The arcade: arches in the front, open behind them back to the shop wall.
  const px = x - 4.0 * floor(x / 4.0) - 2.0;
  const arch = length2(px, max(y - 2.5, 0)) - 1.55;
  const arcade = max(box(x, y - 2.1, z - 3.6, 7.6, 2.1, 1.6), min(arch, z - 4.4));
  const shops = box(x, y - 1.75, z + 1.5, 7.6, 1.55, 3.2);
  const doors = min(box(x + 2.0, y - 1.15, z - 2.0, 0.6, 1.15, 0.5), box(x - 4.0, y - 1.15, z - 2.0, 0.6, 1.15, 0.5));
  const wx = x - 2.0 * floor(x / 2.0) - 1.0;
  const fy = y - 4.2 - 3.4 * floor((y - 4.2) / 3.4);
  const win = max(max(box(wx, fy - 1.5, abs(z) - 5.0, 0.42, 0.8, 0.14), y - H + 0.6), 4.2 - y);
  const shell = max(body, -min(min(arcade, shops), min(doors, win)));
  const sh = max(max(box(abs(wx) - 0.68, fy - 1.5, abs(z) - 5.06, 0.24, 0.82, 0.04), abs(x) - 7.8), max(y - H + 0.6, 4.2 - y));
  const balc = max(box(wx, y - 4.3, z - 5.4, 0.62, 0.08, 0.4), abs(x) - 7.6);
  const rail = max(max(box(wx, y - 4.8, z - 5.76, 0.62, 0.45, 0.03), abs(x) - 7.6), -box(wx, y - 4.8, z - 5.76, 0.56, 0.38, 0.1));
  const cornice = box(x, y - H, z, 8.3, 0.22, 5.3);
  const roof = max(box(x, y - H - 0.9, z, 8.3, 0.9, 5.3), (y - H - 1.7 + max(abs(z) - 2.0, 0) * 0.32) * 0.9);
  return min(min(shell, sh), min(min(balc, rail), min(cornice, roof)));
}

export function color(x, y, z, k) {
  const H = 11.6;
  const n = noise3(x * 1.2, y * 1.2, z * 1.2);
  if (y > H + 0.2) return mix(rgb(176, 78, 44), rgb(206, 104, 60), n) * (fract(x * 3.4) < 0.18 ? 0.82 : 1);
  if (y > H - 0.25) return rgb(242, 236, 224);
  if (y < 4.2) {
    if (z < 2.15 && z > -4.8 && abs(x) < 7.65 && y < 3.35) {
      if (y < 0.05) return rgb(190, 160, 120);
      return rgb(232, 218, 190);
    }
    if (z > 1.85 && z < 2.3 && y > 2.4 && y < 3.2) {
      if (abs(x + 2.0) < 1.4) return glow(rgb(250, 230, 160), 0.4);
      if (abs(x - 4.0) < 1.4) return rgb(30, 90, 60);
    }
    return mix(rgb(234, 228, 214), rgb(246, 242, 232), n);
  }
  const wx = x - 2.0 * floor(x / 2.0) - 1.0;
  const fy = y - 4.2 - 3.4 * floor((y - 4.2) / 3.4);
  if (abs(z) > 4.95) {
    if (abs(wx) < 0.42 && abs(fy - 1.5) < 0.8) return glow(mix(rgb(40, 38, 36), rgb(255, 214, 150), 0.3 * step(0.5, hash(floor(x / 2), floor(y / 3.4), k.seed))), 0.8);
    if (abs(abs(wx) - 0.68) < 0.26 && abs(fy - 1.5) < 0.84) return rgb(46, 112, 74) * (fract(fy * 9) < 0.3 ? 0.82 : 1);
  }
  if (z > 5.3 && y < 5.3) return y < 4.4 ? rgb(214, 206, 190) : rgb(34, 34, 36);
  if (abs(abs(x) - 8.0) < 0.35 && abs(abs(z) - 5.0) < 0.35) return rgb(242, 236, 224);
  let wall = rgb(232, 168, 96);
  const c = floor(hash(k.seed, 4) * 4);
  if (c == 1) wall = rgb(226, 144, 120);
  if (c == 2) wall = rgb(238, 206, 128);
  if (c == 3) wall = rgb(214, 120, 84);
  return wall * (0.9 + 0.1 * n);
}
