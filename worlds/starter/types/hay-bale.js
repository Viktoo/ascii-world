// A round bale of golden hay, its straw wound round and round in a tight
// spiral, loose wisps at its edges and bound with twine; a pumpkin or two
// set against it.
export const meta = {
  name: "hay bale",
  bounds: [0.75, 1.2, 0.88],
  tags: ["prop"],
  props: { mass: 250, burns: 0.95, fuel: 0.5 },
};

export function sdf(x, y, z, k) {
  // The bale lies on its side, its round faces to ±z.
  const body = max(length2(x, y - 0.6) - 0.6, abs(z) - 0.55) + noise3(x * 9, y * 9, z * 9) * 0.035;
  const px = x - (0.32);
  const py = y - (0.17);
  const pz = z - (0.6699999999999999);
  const pa = atan2(pz, px);
  const pr = 0.18 * (0.9 + 0.1 * abs(cos(pa * 2.5)));
  const pb = (length3(px, py * 1.35, pz) - pr) * 0.72;
  const p = smoothSubtract(pb, sphere(px, py - 0.153, pz, 0.050), 0.022);
  const ps = capsule(px, py, pz, 0, 0.093, 0, 0.022, 0.191, 0.009, 0.016);
  return min(body * 0.85, min(p, ps));
}

export function color(x, y, z, k) {
  {
    const px = x - (0.32);
    const py = y - (0.17);
    const pz = z - (0.67);
    if (length3(px, py * 1.35, pz) < 0.194 || (py > 0.073 && length2(px, pz) < 0.036)) {
      if (py > 0.083 && length2(px, pz) < 0.029) return mix(rgb(96, 104, 54), rgb(130, 120, 70), noise3(px * 40, py * 40, pz * 40));
      const g = abs(cos(atan2(pz, px) * 2.5));
      return rgb(222, 108, 24) * (0.62 + 0.38 * g) * (0.88 + 0.12 * noise3(px * 20, py * 20, pz * 20)) * (0.85 + 0.15 * clamp(py / 0.180 + 0.6, 0, 1));
    }
  }
  const r = length2(x, y - 0.6);
  if (abs(z) > 0.53) return (fract(r * 22.0 + atan2(y - 0.6, x) * 0.16) < 0.35 ? 0.78 : 1.0) * mix(rgb(196, 160, 80), rgb(228, 198, 116), noise3(x * 20, y * 20, z * 20));
  if (abs(abs(z) - 0.25) < 0.015) return rgb(170, 140, 90);
  return (fract(atan2(y - 0.6, x) * 9.0 + noise3(x * 4, y * 4, z * 30) * 0.5) < 0.3 ? 0.82 : 1.0) * mix(rgb(196, 160, 80), rgb(226, 194, 112), noise3(x * 30, y * 6, z * 30));
}
