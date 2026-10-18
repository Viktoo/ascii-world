// A charcoal kiln: a great dome of stacked wood sealed under dark earth
// and turf, 4 m across, smouldering: small vents round it glow orange and
// a thin plume of smoke rises from its top.
export const meta = {
  name: "charcoal kiln",
  bounds: [2.3, 3.6, 2.3],
  tags: ["prop", "landmark"],
  props: { mass: 20000, heat: 300, light: 1 },
  sound: { hard: 0.2, dry: 0.4, ring: 0.0 },
  anchors: [{ kind: "light", at: [0, 0.8, 0] }],
};

export function sdf(x, y, z, k) {
  const dome = max(ellipsoid(x, y, z, 2.2, 1.9, 2.2) + (noise3(x * 2, y * 2, z * 2) - 0.5) * 0.15, -y);
  const smoke = max(length2(x + sin(y * 2.0) * 0.15, z) - 0.12 - (y - 1.9) * 0.08, max(1.7 - y, y - 3.5));
  return min(dome, smoke);
}

export function color(x, y, z, k) {
  if (y > 1.95) return rgb(200, 200, 204);
  const a = atan2(z, x);
  if (abs(y - 0.5) < 0.12 && fract(a * 0.95) < 0.12) return glow(rgb(255, 120, 40));
  return mix(rgb(52, 44, 38), rgb(80, 96, 50), smoothstep(0.5, 0.8, noise3(x * 3, y * 3, z * 3)));
}
