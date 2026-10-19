// The sign over a Maple Cove business: BOAT SHOP, in painted letters on a
// board over its door. Its face is +z.
export const meta = {
  name: "boatshop sign",
  bounds: [1.47, 0.6, 0.1],
  tags: ["sign", "prop"],
  props: { mass: 20, light: 1 },
};

export function sdf(x, y, z, k) {
  return roundBox(x, y - 0.28, z, 1.42, 0.28, 0.04, 0.02);
}

export function color(x, y, z, k) {
  if (z < 0.02) return rgb(90, 70, 50);
  if (abs(abs(x) - 1.35) < 0.03 || abs(abs(y - 0.28) - 0.22) < 0.03) return rgb(150, 40, 34);
  const tx = -x;
  const ty = y;
  let lit0 = 0;
  const gx0 = (tx - -1.1424) / 0.2624;
  const ci0 = floor(gx0);
  if (gx0 >= 0 && ci0 < 9 && ty > 0.0752 && ty < 0.4848) {
    const cx0 = (gx0 - ci0) * 1.4138;
    const cy0 = (ty - 0.12) / 0.32;
    const m0 = ci0 < 1 ? 191 : ci0 < 2 ? 63 : ci0 < 3 ? 247 : ci0 < 4 ? 4609 : ci0 < 5 ? 0 : ci0 < 6 ? 237 : ci0 < 7 ? 246 : ci0 < 8 ? 63 : 243;
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
  if (lit0 > 0) return glow(rgb(150, 40, 34), 0.5);
  return rgb(246, 242, 230);
}
