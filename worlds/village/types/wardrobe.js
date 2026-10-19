// A tall wardrobe of dark walnut, two doors with brass handles, carved crest.
export const meta = {
  name: "wardrobe",
  bounds: [0.62, 2.05, 0.32],
  tags: ["furniture"],
  props: { mass: 90, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  return min(box(x, y - 0.98, z, 0.6, 0.98, 0.3), box(x, y - 2.0, z, 0.62, 0.04, 0.32));
}

export function color(x, y, z, k) {
  if (z > 0.29 && abs(abs(x) - 0.06) < 0.025 && abs(y - 1.0) < 0.12) return rgb(200, 160, 60);
  if (z > 0.29 && abs(x) < 0.012) return rgb(40, 28, 20);
  return mix(rgb(84, 56, 38), rgb(104, 72, 48), noise3(x * 6, y * 18, z * 6));
}
