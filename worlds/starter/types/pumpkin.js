// A big ribbed orange pumpkin from the garden, deep grooves between its
// lobes, a dimple on top and its curled green stalk still on.
export const meta = {
  name: "pumpkin",
  bounds: [0.27, 0.42, 0.27],
  tags: ["item", "food"],
  props: { mass: 7, edible: 0.6, burns: 0.3 },
};

export function sdf(x, y, z, k) {
  const px = x - (0);
  const py = y - (0.18);
  const pz = z - (0);
  const pa = atan2(pz, px);
  const pr = 0.25 * (0.9 + 0.1 * abs(cos(pa * 2.5)));
  const pb = (length3(px, py * 1.35, pz) - pr) * 0.72;
  const p = smoothSubtract(pb, sphere(px, py - 0.205, pz, 0.070), 0.030);
  const ps = capsule(px, py, pz, 0, 0.145, 0, 0.030, 0.265, 0.013, 0.022);
  return min(p, ps);
}

export function color(x, y, z, k) {
  {
    const px = x - (0);
    const py = y - (0.18);
    const pz = z - (0);
    if (length3(px, py * 1.35, pz) < 0.270 || (py > 0.125 && length2(px, pz) < 0.050)) {
      if (py > 0.135 && length2(px, pz) < 0.040) return mix(rgb(96, 104, 54), rgb(130, 120, 70), noise3(px * 40, py * 40, pz * 40));
      const g = abs(cos(atan2(pz, px) * 2.5));
      return rgb(222, 108, 24) * (0.62 + 0.38 * g) * (0.88 + 0.12 * noise3(px * 20, py * 20, pz * 20)) * (0.85 + 0.15 * clamp(py / 0.250 + 0.6, 0, 1));
    }
  }
  return rgb(222, 108, 24);
}
