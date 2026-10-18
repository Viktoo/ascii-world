// Marinella's land gate in the town wall: a square gatehouse of limestone
// with merlons along its top, a round arch through it wide enough for a
// cart, and MARINELLA carved and painted in deep blue over the arch on its
// outer face (+z, toward the road); a lantern hangs in the arch.
export const meta = {
  name: "town gate",
  bounds: [5.6, 9.6, 2.3],
  tags: ["building", "landmark", "gate"],
  props: { mass: 100000, light: 1 },
  sound: { hard: 1, dry: 0.4, ring: 0.3 },
  anchors: [{ kind: "light", at: [0, 4.0, 0] }],
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 2.8, z, 5.5, 6.0, 2.2);
  const arch = min(box(x, y - 1.4, z, 1.9, 1.6, 2.6), max(length2(x, y - 3.0) - 1.9, abs(z) - 2.6));
  const mx = x - 1.8 * floor(x / 1.8) - 0.9;
  const merlons = max(box(mx, y - 9.15, abs(z) - 1.95, 0.5, 0.35, 0.25), abs(x) - 5.5);
  const mz = z - 1.8 * floor(z / 1.8) - 0.9;
  const merlons2 = max(box(abs(x) - 5.25, y - 9.15, mz, 0.25, 0.35, 0.5), abs(z) - 2.2);
  const lamp = min(cappedCone(x, y - 3.9, z, 0.22, 0.14, 0.2), cylinder(x, y - 4.5, z, 0.02, 0.4));
  return min(min(max(body, -arch), lamp), min(merlons, merlons2));
}

export function color(x, y, z, k) {
  if (y > 3.6 && y < 4.15 && length2(x, z) < 0.3) return glow(rgb(255, 210, 140));
  const n = noise3(x * 1.6, y * 1.6, z * 1.6);
  const course = fract(y * 2.2) < 0.08 ? 0.86 : 1;
  const joint = fract(x * 1.1 + floor(y * 2.2) * 0.5) < 0.05 ? 0.88 : 1;
  const stone = mix(rgb(196, 182, 152), rgb(222, 208, 178), n) * course * joint;
  if (z > 2.15 && abs(x) < 4.9 && abs(y - 6.6) < 0.95) {
    const tx = -x;
    const ty = y;
  let lit0 = 0;
  const gx0 = (tx - -3.3915) / 0.779;
  const ci0 = floor(gx0);
  if (gx0 >= 0 && ci0 < 9 && ty > 6.0075 && ty < 7.2425) {
    const cx0 = (gx0 - ci0) * 1.4138;
    const cy0 = (ty - 6.15) / 0.95;
    const m0 = ci0 < 1 ? 1334 : ci0 < 2 ? 247 : ci0 < 3 ? 8435 : ci0 < 4 ? 4617 : ci0 < 5 ? 8502 : ci0 < 6 ? 121 : ci0 < 7 ? 56 : ci0 < 8 ? 56 : 247;
    if (fract(m0 / 2) >= 0.5 && max(abs(cy0 - 1) * 0.95, max(max(0.08 - cx0, cx0 - 0.92), 0) * 0.551) < 0.1425) lit0 = 1;
    if (fract(m0 / 4) >= 0.5 && max(abs(cx0 - 1) * 0.551, max(max(0.58 - cy0, cy0 - 0.92), 0) * 0.95) < 0.1425) lit0 = 1;
    if (fract(m0 / 8) >= 0.5 && max(abs(cx0 - 1) * 0.551, max(max(0.08 - cy0, cy0 - 0.42), 0) * 0.95) < 0.1425) lit0 = 1;
    if (fract(m0 / 16) >= 0.5 && max(abs(cy0 - 0) * 0.95, max(max(0.08 - cx0, cx0 - 0.92), 0) * 0.551) < 0.1425) lit0 = 1;
    if (fract(m0 / 32) >= 0.5 && max(abs(cx0 - 0) * 0.551, max(max(0.08 - cy0, cy0 - 0.42), 0) * 0.95) < 0.1425) lit0 = 1;
    if (fract(m0 / 64) >= 0.5 && max(abs(cx0 - 0) * 0.551, max(max(0.58 - cy0, cy0 - 0.92), 0) * 0.95) < 0.1425) lit0 = 1;
    if (fract(m0 / 128) >= 0.5 && max(abs(cy0 - 0.5) * 0.95, max(max(0.08 - cx0, cx0 - 0.42), 0) * 0.551) < 0.1425) lit0 = 1;
    if (fract(m0 / 256) >= 0.5 && max(abs(cy0 - 0.5) * 0.95, max(max(0.58 - cx0, cx0 - 0.92), 0) * 0.551) < 0.1425) lit0 = 1;
    if (fract(m0 / 512) >= 0.5 && length2((cx0 - 0) * 0.551 - 0.2755 * clamp(((cx0 - 0) * 0.551 * 0.2755 + (cy0 - 1) * 0.95 * -0.475) / 0.3015, 0.12, 0.88), (cy0 - 1) * 0.95 - -0.475 * clamp(((cx0 - 0) * 0.551 * 0.2755 + (cy0 - 1) * 0.95 * -0.475) / 0.3015, 0.12, 0.88)) < 0.1425) lit0 = 1;
    if (fract(m0 / 1024) >= 0.5 && max(abs(cx0 - 0.5) * 0.551, max(max(0.58 - cy0, cy0 - 0.92), 0) * 0.95) < 0.1425) lit0 = 1;
    if (fract(m0 / 2048) >= 0.5 && length2((cx0 - 1) * 0.551 - -0.2755 * clamp(((cx0 - 1) * 0.551 * -0.2755 + (cy0 - 1) * 0.95 * -0.475) / 0.3015, 0.12, 0.88), (cy0 - 1) * 0.95 - -0.475 * clamp(((cx0 - 1) * 0.551 * -0.2755 + (cy0 - 1) * 0.95 * -0.475) / 0.3015, 0.12, 0.88)) < 0.1425) lit0 = 1;
    if (fract(m0 / 4096) >= 0.5 && length2((cx0 - 0) * 0.551 - 0.2755 * clamp(((cx0 - 0) * 0.551 * 0.2755 + (cy0 - 0) * 0.95 * 0.475) / 0.3015, 0.12, 0.88), (cy0 - 0) * 0.95 - 0.475 * clamp(((cx0 - 0) * 0.551 * 0.2755 + (cy0 - 0) * 0.95 * 0.475) / 0.3015, 0.12, 0.88)) < 0.1425) lit0 = 1;
    if (fract(m0 / 8192) >= 0.5 && max(abs(cx0 - 0.5) * 0.551, max(max(0.08 - cy0, cy0 - 0.42), 0) * 0.95) < 0.1425) lit0 = 1;
    if (fract(m0 / 16384) >= 0.5 && length2((cx0 - 0.5) * 0.551 - 0.2755 * clamp(((cx0 - 0.5) * 0.551 * 0.2755 + (cy0 - 0.5) * 0.95 * -0.475) / 0.3015, 0.12, 0.88), (cy0 - 0.5) * 0.95 - -0.475 * clamp(((cx0 - 0.5) * 0.551 * 0.2755 + (cy0 - 0.5) * 0.95 * -0.475) / 0.3015, 0.12, 0.88)) < 0.1425) lit0 = 1;
  }
    if (lit0 > 0) return rgb(30, 64, 140);
    if (abs(abs(x) - 4.85) < 0.08 || abs(abs(y - 6.6) - 0.9) < 0.08) return rgb(30, 64, 140);
    return rgb(240, 234, 220);
  }
  if (length2(x, y - 3.0) < 2.35 && length2(x, y - 3.0) > 1.9 && y > 2.9) return rgb(238, 232, 218);
  return stone;
}
