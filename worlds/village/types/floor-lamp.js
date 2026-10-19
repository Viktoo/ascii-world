// A standing lamp with a fringed cream shade, warm light at night.
export const meta = {
  name: "floor lamp",
  bounds: [0.25, 1.7, 0.25],
  tags: ["furniture", "lamp"],
  props: { mass: 6, light: 1, fragile: 0.4 },
  anchors: [
    { kind: "light", at: [0, 1.5, 0] },
  ],
};

export function sdf(x, y, z, k) {
  return min(min(cylinder(x, y - 0.03, z, 0.18, 0.03), cylinder(x, y - 0.8, z, 0.02, 0.8)), cappedCone(x, y - 1.5, z, 0.16, 0.24, 0.14));
}

export function color(x, y, z, k) {
  if (y > 1.33) return glow(rgb(255, 226, 170), 0.8);
  return rgb(60, 50, 40);
}
