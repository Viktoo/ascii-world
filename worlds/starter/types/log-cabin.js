// A woodcutter's cabin in Pine Hollow: walls of round pine logs with
// their ends crossing at the corners, a steep roof of wooden shingles
// grown with moss, a fieldstone chimney, a porch with a bench under the
// eaves and a door and windows with red-painted frames. Inside: a hearth,
// a bed, a table. Its door faces +z.
export const meta = {
  name: "log cabin",
  bounds: [4.4, 6.0, 4.6],
  tags: ["building", "house"],
  props: { mass: 40000, burns: 0.6, light: 1 },
  sound: { hard: 0.5, dry: 0.8, ring: 0.05 },
  anchors: [
    { kind: "door", at: [0.6, 0, 2.75], face: 0, size: [1.0, 2.0] },
    { kind: "light", at: [-1.8, 1.5, 3.0] },
    { kind: "bed", at: [-2.0, 0.05, -1.4] },
    { kind: "seat", at: [-2.2, 0.45, 3.7], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  // Log walls: a box ribbed by round logs every 0.3 m.
  const ly = y - 0.3 * floor(y / 0.3) - 0.15;
  const walls = max(box(x, y - 1.5, z, 3.5, 1.5, 2.7), -box(x, y - 1.4, z, 3.15, 1.4, 2.35));
  const ribs = max(max(length2(ly, abs(z) - 2.7) - 0.17, abs(x) - 3.75), max(y - 3.0, -y));
  const ribs2 = max(max(length2(ly, abs(x) - 3.5) - 0.17, abs(z) - 2.95), max(y - 3.0, -y));
  const door = box(x - 0.6, y - 1.0, z - 2.7, 0.5, 1.0, 0.6);
  const win = box(x + 1.8, y - 1.5, z - 2.7, 0.45, 0.4, 0.6);
  const shell = max(min(walls, min(ribs, ribs2)), -min(door, win));
  // A steep roof along x with eaves over the porch.
  const roof = max(max(box(x, y - 4.2, z + 0.4, 3.9, 1.7, 4.1), (y - 5.4 + abs(z + 0.4) * 0.95) * 0.72), -((y - 5.2 + abs(z + 0.4) * 0.95) * 0.72));
  const attic = max(box(x, y - 4.2, z + 0.4, 3.5, 1.25, 2.7), (y - 5.3 + abs(z + 0.4) * 0.95) * 0.72);
  const posts = box(abs(x) - 3.3, y - 1.3, z - 3.9, 0.1, 1.3, 0.1);
  const porch = box(x, y - 0.08, z - 3.3, 3.5, 0.08, 0.6);
  const bench = box(x + 2.2, y - 0.4, z - 3.75, 0.8, 0.05, 0.18);
  const chim = box(x + 3.1, y - 2.8, z + 0.6, 0.45, 2.8, 0.5);
  const hearth = box(x + 2.8, y - 0.5, z + 0.6, 0.35, 0.5, 0.6);
  const bed = box(x + 2.0, y - 0.25, z + 1.4, 0.95, 0.25, 0.55);
  const table = box(x - 0.6, y - 0.75, z + 1.6, 0.6, 0.04, 0.4);
  return min(min(min(shell, attic), roof), min(min(posts, porch), min(min(bench, chim), min(hearth, min(bed, table)))));
}

export function color(x, y, z, k) {
  const n = noise3(x * 2, y * 2, z * 2);
  if (y > 3.05 && abs(x + 3.1) > 0.5) return mix(rgb(96, 74, 52), rgb(84, 116, 56), smoothstep(0.4, 0.8, n)) * (fract((5.4 - y) * 3.0) < 0.2 ? 0.8 : 1);
  if (abs(x + 3.1) < 0.5 && abs(z + 0.6) < 0.55) return mix(rgb(128, 122, 112), rgb(160, 152, 138), n);
  if (z > 2.6 && abs(x + 1.8) < 0.6 && abs(y - 1.5) < 0.55) {
    if (abs(x + 1.8) < 0.45 && abs(y - 1.5) < 0.4) return glow(rgb(70, 60, 44), 0.5);
    return rgb(176, 50, 40);
  }
  if (z > 2.6 && abs(x - 0.6) < 0.62 && y < 2.1) return rgb(176, 50, 40);
  if (y < 0.17 && z > 2.7) return rgb(120, 90, 60);
  const ly = fract(y / 0.3);
  return mix(rgb(120, 82, 50), rgb(150, 108, 66), n) * (ly < 0.12 || ly > 0.88 ? 0.7 : 1);
}
