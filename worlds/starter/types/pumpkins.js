// A heap of pumpkins and squash by a door or a porch step: a big ribbed orange
// pumpkin and a smaller one on it, a little white pumpkin and a warty green
// gourd, their stalks still on.
export const meta = {
  name: "pumpkins",
  bounds: [0.95, 0.62, 0.62],
  tags: ["prop", "decoration"],
  props: { mass: 40, edible: 0.6, burns: 0.3 },
};

export function sdf(x, y, z, k) {
  const ax = x - (-0.38);
  const ay = y - (0.2);
  const az = z - (0);
  const aa = atan2(az, ax);
  const ar = 0.27 * (0.9 + 0.1 * abs(cos(aa * 2.5)));
  const ab = (length3(ax, ay * 1.35, az) - ar) * 0.72;
  const a = smoothSubtract(ab, sphere(ax, ay - 0.220, az, 0.076), 0.032);
  const as = capsule(ax, ay, az, 0, 0.160, 0, 0.032, 0.286, 0.014, 0.024);  const bx = x - (0.18);
  const by = y - (0.15);
  const bz = z - (0.12);
  const ba = atan2(bz, bx);
  const br = 0.2 * (0.9 + 0.1 * abs(cos(ba * 2.5)));
  const bb = (length3(bx, by * 1.35, bz) - br) * 0.72;
  const b = smoothSubtract(bb, sphere(bx, by - 0.168, bz, 0.056), 0.024);
  const bs = capsule(bx, by, bz, 0, 0.108, 0, 0.024, 0.212, 0.010, 0.018);  const cx = x - (0.62);
  const cy = y - (0.1);
  const cz = z - (-0.18);
  const ca = atan2(cz, cx);
  const cr = 0.13 * (0.9 + 0.1 * abs(cos(ca * 2.0)));
  const cb = (length3(cx, cy * 1.5, cz) - cr) * 0.72;
  const c = smoothSubtract(cb, sphere(cx, cy - 0.107, cz, 0.036), 0.016);
  const cs = capsule(cx, cy, cz, 0, 0.047, 0, 0.016, 0.128, 0.007, 0.012);  const dx = x - (-0.3);
  const dy = y - (0.47);
  const dz = z - (0.02);
  const da = atan2(dz, dx);
  const dr = 0.13 * (0.9 + 0.1 * abs(cos(da * 2.5)));
  const db = (length3(dx, dy * 1.35, dz) - dr) * 0.72;
  const d = smoothSubtract(db, sphere(dx, dy - 0.116, dz, 0.036), 0.016);
  const ds = capsule(dx, dy, dz, 0, 0.056, 0, 0.016, 0.138, 0.007, 0.012);
  const gourd = (length3(x - 0.2, (y - 0.09) * 1.0, (z + 0.32) * 0.7) - 0.09 + noise3(x * 30, y * 30, z * 30) * 0.02) * 0.6;
  return min(min(min(a, as), min(b, bs)), min(min(min(c, cs), min(d, ds)), gourd));
}

export function color(x, y, z, k) {
  {
    const px = x - (-0.3);
    const py = y - (0.47);
    const pz = z - (0.02);
    if (length3(px, py * 1.35, pz) < 0.140 || (py > 0.036 && length2(px, pz) < 0.026)) {
      if (py > 0.046 && length2(px, pz) < 0.021) return mix(rgb(96, 104, 54), rgb(130, 120, 70), noise3(px * 40, py * 40, pz * 40));
      const g = abs(cos(atan2(pz, px) * 2.5));
      return rgb(236, 140, 36) * (0.62 + 0.38 * g) * (0.88 + 0.12 * noise3(px * 20, py * 20, pz * 20)) * (0.85 + 0.15 * clamp(py / 0.130 + 0.6, 0, 1));
    }
  }  {
    const px = x - (-0.38);
    const py = y - (0.2);
    const pz = z - (0);
    if (length3(px, py * 1.35, pz) < 0.292 || (py > 0.140 && length2(px, pz) < 0.054)) {
      if (py > 0.150 && length2(px, pz) < 0.043) return mix(rgb(96, 104, 54), rgb(130, 120, 70), noise3(px * 40, py * 40, pz * 40));
      const g = abs(cos(atan2(pz, px) * 2.5));
      return rgb(222, 108, 24) * (0.62 + 0.38 * g) * (0.88 + 0.12 * noise3(px * 20, py * 20, pz * 20)) * (0.85 + 0.15 * clamp(py / 0.270 + 0.6, 0, 1));
    }
  }  {
    const px = x - (0.18);
    const py = y - (0.15);
    const pz = z - (0.12);
    if (length3(px, py * 1.35, pz) < 0.216 || (py > 0.088 && length2(px, pz) < 0.040)) {
      if (py > 0.098 && length2(px, pz) < 0.032) return mix(rgb(96, 104, 54), rgb(130, 120, 70), noise3(px * 40, py * 40, pz * 40));
      const g = abs(cos(atan2(pz, px) * 2.5));
      return rgb(214, 96, 22) * (0.62 + 0.38 * g) * (0.88 + 0.12 * noise3(px * 20, py * 20, pz * 20)) * (0.85 + 0.15 * clamp(py / 0.200 + 0.6, 0, 1));
    }
  }  {
    const px = x - (0.62);
    const py = y - (0.1);
    const pz = z - (-0.18);
    if (length3(px, py * 1.5, pz) < 0.140 || (py > 0.027 && length2(px, pz) < 0.026)) {
      if (py > 0.037 && length2(px, pz) < 0.021) return mix(rgb(96, 104, 54), rgb(130, 120, 70), noise3(px * 40, py * 40, pz * 40));
      const g = abs(cos(atan2(pz, px) * 2.0));
      return rgb(240, 236, 220) * (0.62 + 0.38 * g) * (0.88 + 0.12 * noise3(px * 20, py * 20, pz * 20)) * (0.85 + 0.15 * clamp(py / 0.130 + 0.6, 0, 1));
    }
  }
  if (z < -0.2) return mix(rgb(70, 110, 50), rgb(200, 190, 90), step(0.6, noise3(x * 25, y * 25, z * 25)));
  return rgb(222, 108, 24);
}
