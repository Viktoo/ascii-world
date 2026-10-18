// The fire pit in the middle of the clearing: a ring of fieldstones round
// glowing embers and a crackling fire of split logs.
export const meta = {
  name: "fire pit",
  bounds: [1.2, 1.2, 1.2],
  tags: ["prop", "fire"],
  props: { mass: 500, heat: 500, light: 1 },
  sound: { hard: 0.8, dry: 0.6, ring: 0.0 },
  anchors: [{ kind: "light", at: [0, 0.6, 0] }],
};

export function sdf(x, y, z, k) {
  const a = atan2(z, x);
  const r = length2(x, z);
  const stones = max(length2(r - 0.95, y - 0.12) - 0.2 - sin(a * 9.0) * 0.04, -y);
  const logs = min(capsule(x, y, z, -0.5, 0.1, -0.2, 0.1, 0.55, 0.0, 0.08), capsule(x, y, z, 0.5, 0.1, -0.15, -0.05, 0.55, 0.05, 0.08));
  const logs2 = capsule(x, y, z, 0.0, 0.1, 0.5, 0.0, 0.55, -0.05, 0.08);
  const flame = max(cappedCone(x, y - 0.7, z, 0.45, 0.35, 0.02) + (noise3(x * 6, y * 3, z * 6) - 0.5) * 0.12, -0.0 - y);
  return min(min(stones, logs), min(logs2, flame * 0.8));
}

export function color(x, y, z, k) {
  const r = length2(x, z);
  if (r > 0.72) return mix(rgb(120, 114, 106), rgb(156, 150, 140), noise3(x * 4, y * 4, z * 4));
  if (y > 0.35) return glow(mix(rgb(255, 210, 80), rgb(255, 100, 30), clamp((y - 0.35) * 1.5, 0, 1)));
  return glow(rgb(200, 70, 30), 0.6);
}
