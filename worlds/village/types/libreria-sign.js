// The sign over a Marinella shop: LIBRI E GIORNALI, in painted letters on a board
// fixed to the wall over its door and window. Its face is +z.
export const meta = {
  name: "libreria sign",
  bounds: [2.41, 0.6, 0.1],
  tags: ["sign", "prop"],
  props: { mass: 20, light: 1 },
};

export function sdf(x, y, z, k) {
  return roundBox(x, y - 0.28, z, 2.36, 0.28, 0.04, 0.02);
}

export function color(x, y, z, k) {
  if (z < 0.02) return rgb(90, 70, 50);
  if (abs(abs(x) - 2.30) < 0.03 || abs(abs(y - 0.28) - 0.22) < 0.03) return rgb(240, 220, 170);
  const tx = -x;
  const ty = y;
  let lit0 = 0;
  const gx0 = (tx - -2.0608) / 0.2624;
  const ci0 = floor(gx0);
  if (gx0 >= 0 && ci0 < 16 && ty > 0.0752 && ty < 0.4848) {
    const cx0 = (gx0 - ci0) * 1.4138;
    const cy0 = (ty - 0.12) / 0.32;
    const m0 = ci0 < 1 ? 56 : ci0 < 2 ? 4617 : ci0 < 3 ? 191 : ci0 < 4 ? 8435 : ci0 < 5 ? 4617 : ci0 < 6 ? 0 : ci0 < 7 ? 121 : ci0 < 8 ? 0 : ci0 < 9 ? 189 : ci0 < 10 ? 4617 : ci0 < 11 ? 63 : ci0 < 12 ? 8435 : ci0 < 13 ? 8502 : ci0 < 14 ? 247 : ci0 < 15 ? 56 : 4617;
    if (fract(m0 / 2) >= 0.5 && max(abs(cy0 - 1) * 0.32, max(max(0.08 - cx0, cx0 - 0.92), 0) * 0.1856) < 0.0448) lit0 = 1;
    if (fract(m0 / 4) >= 0.5 && max(abs(cx0 - 1) * 0.1856, max(max(0.58 - cy0, cy0 - 0.92), 0) * 0.32) < 0.0448) lit0 = 1;
    if (fract(m0 / 8) >= 0.5 && max(abs(cx0 - 1) * 0.1856, max(max(0.08 - cy0, cy0 - 0.42), 0) * 0.32) < 0.0448) lit0 = 1;
    if (fract(m0 / 16) >= 0.5 && max(abs(cy0 - 0) * 0.32, max(max(0.08 - cx0, cx0 - 0.92), 0) * 0.1856) < 0.0448) lit0 = 1;
    if (fract(m0 / 32) >= 0.5 && max(abs(cx0 - 0) * 0.1856, max(max(0.08 - cy0, cy0 - 0.42), 0) * 0.32) < 0.0448) lit0 = 1;
    if (fract(m0 / 64) >= 0.5 && max(abs(cx0 - 0) * 0.1856, max(max(0.58 - cy0, cy0 - 0.92), 0) * 0.32) < 0.0448) lit0 = 1;
    if (fract(m0 / 128) >= 0.5 && max(abs(cy0 - 0.5) * 0.32, max(max(0.08 - cx0, cx0 - 0.42), 0) * 0.1856) < 0.0448) lit0 = 1;
    if (fract(m0 / 256) >= 0.5 && max(abs(cy0 - 0.5) * 0.32, max(max(0.58 - cx0, cx0 - 0.92), 0) * 0.1856) < 0.0448) lit0 = 1;
    if (fract(m0 / 512) >= 0.5 && length2((cx0 - 0) * 0.1856 - 0.0928 * clamp(((cx0 - 0) * 0.1856 * 0.0928 + (cy0 - 1) * 0.32 * -0.16) / 0.0342, 0.12, 0.88), (cy0 - 1) * 0.32 - -0.16 * clamp(((cx0 - 0) * 0.1856 * 0.0928 + (cy0 - 1) * 0.32 * -0.16) / 0.0342, 0.12, 0.88)) < 0.0448) lit0 = 1;
    if (fract(m0 / 1024) >= 0.5 && max(abs(cx0 - 0.5) * 0.1856, max(max(0.58 - cy0, cy0 - 0.92), 0) * 0.32) < 0.0448) lit0 = 1;
    if (fract(m0 / 2048) >= 0.5 && length2((cx0 - 1) * 0.1856 - -0.0928 * clamp(((cx0 - 1) * 0.1856 * -0.0928 + (cy0 - 1) * 0.32 * -0.16) / 0.0342, 0.12, 0.88), (cy0 - 1) * 0.32 - -0.16 * clamp(((cx0 - 1) * 0.1856 * -0.0928 + (cy0 - 1) * 0.32 * -0.16) / 0.0342, 0.12, 0.88)) < 0.0448) lit0 = 1;
    if (fract(m0 / 4096) >= 0.5 && length2((cx0 - 0) * 0.1856 - 0.0928 * clamp(((cx0 - 0) * 0.1856 * 0.0928 + (cy0 - 0) * 0.32 * 0.16) / 0.0342, 0.12, 0.88), (cy0 - 0) * 0.32 - 0.16 * clamp(((cx0 - 0) * 0.1856 * 0.0928 + (cy0 - 0) * 0.32 * 0.16) / 0.0342, 0.12, 0.88)) < 0.0448) lit0 = 1;
    if (fract(m0 / 8192) >= 0.5 && max(abs(cx0 - 0.5) * 0.1856, max(max(0.08 - cy0, cy0 - 0.42), 0) * 0.32) < 0.0448) lit0 = 1;
    if (fract(m0 / 16384) >= 0.5 && length2((cx0 - 0.5) * 0.1856 - 0.0928 * clamp(((cx0 - 0.5) * 0.1856 * 0.0928 + (cy0 - 0.5) * 0.32 * -0.16) / 0.0342, 0.12, 0.88), (cy0 - 0.5) * 0.32 - -0.16 * clamp(((cx0 - 0.5) * 0.1856 * 0.0928 + (cy0 - 0.5) * 0.32 * -0.16) / 0.0342, 0.12, 0.88)) < 0.0448) lit0 = 1;
  }
  if (lit0 > 0) return glow(rgb(240, 220, 170), 0.5);
  return rgb(70, 50, 40);
}
