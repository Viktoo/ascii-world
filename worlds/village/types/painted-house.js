// A fisherman's house in Marinella: two to four floors of painted plaster
// (ochre, coral, rose, lemon, sky, mint, cream or brick), a terracotta tiled
// roof with a chimney, green or blue shutters at every window, flower boxes
// of geraniums, a balcony over the arched front door, white quoins at the
// corners and a stone base. Its door opens on the lane in front (+z).
export const meta = {
  name: "painted house",
  bounds: [4.4, 14.4, 4.6],
  tags: ["building", "house"],
  props: { mass: 100000, burns: 0.25, light: 1 },
  sound: { hard: 0.9, dry: 0.8, ring: 0.1 },
  anchors: [
    { kind: "door", at: [-1.2, 0, 4.0], face: 0, size: [1.1, 2.2] },
    { kind: "light", at: [-1.2, 2.6, 4.5] },
    { kind: "bed", at: [1.6, 0.05, -2.4] },
  ],
};

export function sdf(x, y, z, k) {
  const w = 3.3 + hash(k.seed) * 0.9;
  const floors = 1 + floor(hash(k.seed, 2) * 2.99);
  const H = 3.4 + floors * 3.0;
  const body = box(x, y - H * 0.5, z, w, H * 0.5, 4.0);
  // The room downstairs, and its arched doorway.
  const room = box(x, y - 1.6, z, w - 0.3, 1.45, 3.7);
  const door = min(box(x + 1.2, y - 0.95, z - 4.0, 0.55, 0.95, 0.5), max(length2(x + 1.2, y - 1.9) - 0.55, abs(z - 4.0) - 0.5));
  // Windows set into the walls: two or three across each floor, front and back.
  const wx = x - 2.2 * floor(x / 2.2 + 0.5);
  const fy = y - 3.4 - 3.0 * floor((y - 3.4) / 3.0);
  const win = max(max(box(wx, fy - 1.45, abs(z) - 4.0, 0.42, 0.68, 0.12), y - H + 0.4), 3.4 - y);
  const shell = max(body, -min(min(room, door), win));
  // Shutters folded open beside each window, a sill and a box of flowers.
  const sh = max(max(box(abs(wx) - 0.68, fy - 1.45, abs(z) - 4.06, 0.24, 0.7, 0.04), abs(x) - w + 0.2), max(y - H + 0.4, 3.4 - y));
  const sill = max(max(box(wx, fy - 0.7, abs(z) - 4.1, 0.5, 0.06, 0.12), abs(x) - w + 0.2), max(y - H + 0.4, 3.4 - y));
  const fbox = max(max(box(wx, fy - 0.58, z - 4.22, 0.4, 0.13, 0.13), abs(x) - w + 0.2), max(y - H + 0.4, 3.4 - y));
  // A balcony over the door, on the first floor.
  const balc = min(box(x + 1.2, y - 3.45, z - 4.45, 0.95, 0.08, 0.45), max(box(x + 1.2, y - 3.95, z - 4.86, 0.95, 0.45, 0.03), -1.0));
  // The roof: tiles pitched down to front and back, a chimney through it.
  const roof = max(max(box(x, y - H - 1.0, z, w + 0.35, 1.2, 4.45), y - H - (4.45 - abs(z)) * 0.42), H - 0.15 - y);
  const chim = box(x - w * 0.5, y - H - 1.5, z + 1.6, 0.35, 1.0, 0.35);
  const base = max(max(box(x, y - 0.25, z, w + 0.06, 0.25, 4.06), -box(x, y - 0.25, z, w - 0.3, 0.5, 3.7)), -door);
  return min(min(min(shell, roof * 0.85), min(chim, base)), min(min(sh, sill), min(fbox, balc)));
}

export function color(x, y, z, k) {
  const w = 3.3 + hash(k.seed) * 0.9;
  const floors = 1 + floor(hash(k.seed, 2) * 2.99);
  const H = 3.4 + floors * 3.0;
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  if (y > H - 0.1) {
    if (abs(x - (-w * 0.5)) < 0.4 && abs(z - 1.6) < 0.4) return y > H + 2.3 ? rgb(70, 60, 56) : rgb(226, 214, 196);
    const tile = fract(x * 3.6) < 0.18 ? 0.8 : 1;
    const row = fract((4.45 - abs(z)) * 2.4) < 0.2 ? 0.86 : 1;
    return mix(rgb(176, 78, 44), rgb(206, 104, 60), n) * tile * row;
  }
  if (y < 0.5) return mix(rgb(150, 140, 126), rgb(178, 168, 150), n);
  const wx = x - 2.2 * floor(x / 2.2 + 0.5);
  const fy = y - 3.4 - 3.0 * floor((y - 3.4) / 3.0);
  const face = abs(z) > 3.95;
  if (face && y > 3.4) {
    if (abs(wx) < 0.42 && abs(fy - 1.45) < 0.68) return glow(mix(rgb(44, 40, 38), rgb(255, 210, 150), 0.25 * step(0.55, hash(floor(x / 2.2), floor(y / 3.0), k.seed))), 0.8);
    if (abs(abs(wx) - 0.68) < 0.25 && abs(fy - 1.45) < 0.7) {
      const blue = hash(k.seed, 5) > 0.5;
      const slat = fract(fy * 9.0) < 0.3 ? 0.82 : 1;
      return (blue ? rgb(46, 104, 170) : rgb(52, 120, 82)) * slat;
    }
    if (z > 4.1 && abs(fy - 0.58) < 0.14 && abs(wx) < 0.4) return rgb(172, 82, 50);
    if (z > 4.1 && abs(fy - 0.78) < 0.1 && abs(wx) < 0.4) return hsv(fract(hash(floor(x / 2.2), k.seed) * 0.15 + 0.95), 0.8, 0.95);
    if (abs(fy - 0.7) < 0.06) return rgb(236, 232, 222);
  }
  if (z > 4.3 && y > 3.3 && y < 4.5 && abs(x + 1.2) < 1.0) return rgb(40, 40, 42);
  if (abs(y - 3.4 - 3.0 * floor((y - 3.4) / 3.0 + 0.5)) < 0.09 && y > 3.0) return rgb(240, 236, 226);
  if (abs(abs(x) - w) < 0.38 && abs(abs(z) - 4.0) < 0.38 && fract(y * 1.1) < 0.5) return rgb(240, 236, 226);
  let wall = rgb(226, 170, 80);
  const c = floor(hash(k.seed, 4) * 8);
  if (c == 1) wall = rgb(238, 132, 100);
  if (c == 2) wall = rgb(234, 156, 152);
  if (c == 3) wall = rgb(240, 214, 112);
  if (c == 4) wall = rgb(152, 192, 222);
  if (c == 5) wall = rgb(164, 210, 176);
  if (c == 6) wall = rgb(242, 230, 204);
  if (c == 7) wall = rgb(248, 194, 146);
  return wall * (0.9 + 0.1 * n);
}
