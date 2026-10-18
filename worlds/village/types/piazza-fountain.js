// The fountain in the middle of the Piazza del Mare: a round stone basin of
// clear blue water, a fluted pedestal holding a shell-shaped bowl, a stone
// dolphin on top spouting water that falls in a ring into the basin.
export const meta = {
  name: "piazza fountain",
  bounds: [2.9, 3.0, 2.9],
  tags: ["landmark", "fountain"],
  props: { mass: 30000, wet: 1 },
  sound: { hard: 1, dry: 0.2, ring: 0.3 },
  anchors: [{ kind: "seat", at: [0, 0.55, 2.95], face: 0 }, { kind: "seat", at: [0, 0.55, -2.95], face: 180 }],
};

export function sdf(x, y, z, k) {
  const r = length2(x, z);
  const basin = max(max(abs(r - 2.65) - 0.2, y - 0.6), -y - 0.2);
  const water = max(r - 2.5, max(y - 0.45, -y));
  const ped = cappedCone(x, y - 0.95, z, 0.95, 0.5, 0.28);
  const bowl = max(max(length3(x, y - 2.3, z) - 1.15, -(length3(x, y - 2.45, z) - 1.05)), y - 2.3);
  const neck = cappedCone(x, y - 2.45, z, 0.3, 0.22, 0.12);
  const fish = ellipsoid(x, y - 2.95, z, 0.18, 0.32, 0.18);
  const falls = max(abs(r - 1.12 - (2.3 - y) * 0.15) - 0.04, max(y - 2.25, 0.4 - y));
  return min(min(basin, water), min(min(ped, bowl), min(min(neck, fish), falls * 0.9)));
}

export function color(x, y, z, k) {
  const r = length2(x, z);
  if (r < 2.48 && y < 0.5 && y > 0.3) return mix(rgb(60, 156, 196), rgb(120, 200, 220), noise3(x * 2, 0, z * 2));
  if (abs(r - 1.12 - (2.3 - y) * 0.15) < 0.08 && y < 2.25 && y > 0.4) return rgb(190, 226, 240);
  if (y > 2.6) return rgb(150, 160, 150);
  return mix(rgb(222, 212, 192), rgb(240, 234, 220), noise3(x * 3, y * 3, z * 3));
}
