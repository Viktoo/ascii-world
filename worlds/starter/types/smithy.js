// The smithy: an open-fronted shed of dark boards under a slate-grey roof,
// the forge glowing in its stone hearth at the back, an anvil on a stump, a
// quench tub and tongs and horseshoes hung on the wall. It opens to +z.
export const meta = {
  name: "smithy",
  bounds: [4.0, 5.2, 3.4],
  tags: ["building", "workshop"],
  props: { mass: 30000, burns: 0.3, light: 1, heat: 60 },
  anchors: [
    { kind: "light", at: [1.6, 1.0, 2.1] },
  ],
};

export function sdf(x, y, z, k) {
  const back = box(x, y - 1.6, z + 3.0, 3.8, 1.6, 0.12);
  const sides = box(abs(x) - 3.7, y - 1.6, z, 0.12, 1.6, 3.0);
  const posts = box(abs(x) - 3.7, y - 1.6, z - 3.0, 0.12, 1.6, 0.12);
  const roof = max(box(x, y - 3.6 + z * 0.3, z, 4.0, 0.08, 3.4), 0.0 - 1.0);
  const forge = max(box(x + 1.6, y - 0.5, z + 2.2, 1.0, 0.5, 0.7), -box(x + 1.6, y - 0.95, z + 2.1, 0.6, 0.15, 0.4));
  const hood = cappedCone(x + 1.6, y - 2.3, z + 2.3, 0.8, 0.8, 0.25);
  const flue = box(x + 1.6, y - 3.8, z + 2.4, 0.25, 1.2, 0.25);
  const stump = cylinder(x - 0.6, y - 0.3, z + 0.6, 0.3, 0.3);
  const anvil = min(box(x - 0.6, y - 0.72, z + 0.6, 0.12, 0.12, 0.3), box(x - 0.6, y - 0.9, z + 0.6, 0.16, 0.06, 0.4));
  const tub = max(cylinder(x - 2.6, y - 0.35, z + 2.0, 0.45, 0.35), -cylinder(x - 2.6, y - 0.45, z + 2.0, 0.4, 0.35));
  return min(min(min(back, sides), min(posts, roof)), min(min(forge, min(hood, flue)), min(min(stump, anvil), tub)));
}

export function color(x, y, z, k) {
  if (abs(x + 1.6) < 0.62 && abs(z + 2.1) < 0.42 && y > 0.78 && y < 1.02) return glow(rgb(255, 120, 40), 1.0);
  if (y > 3.0 + 0.0 - z * 0.3 && abs(x + 1.6) > 0.3) return rgb(80, 86, 92);
  if (abs(x + 1.6) < 1.05 && z > 1.4 && y < 1.05) return mix(rgb(120, 116, 110), rgb(150, 144, 136), noise3(x * 5, y * 5, z * 5));
  if (abs(x - 0.6 - 0.0) < 0.2 && abs(z - 0.6) < 0.45 && y > 0.6 && abs(x + 0.6) < 0.3) return rgb(50, 52, 56);
  if (y > 1.4 && abs(x + 1.6) < 0.85 && z > 1.5) return rgb(40, 40, 42);
  if (length2(x - 2.6, z - 2.0) < 0.5) return y > 0.6 ? rgb(60, 90, 110) : rgb(110, 80, 50);
  return mix(rgb(70, 56, 44), rgb(90, 72, 56), noise3(x * 3, y * 12, z * 3));
}
