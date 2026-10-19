// A red-and-white checked picnic blanket spread on the sand, a wicker basket
// on it with a loaf, apples and a thermos of cider.
export const meta = {
  name: "picnic blanket",
  bounds: [1.0, 0.35, 0.8],
  tags: ["prop", "seat", "nonsolid"],
  props: { mass: 4, burns: 0.8 },
  anchors: [
    { kind: "seat", at: [0.3, 0.05, 0.2], face: 0 },
    { kind: "seat", at: [-0.3, 0.05, -0.2], face: 0 },
  ],
};

export function sdf(x, y, z, k) {
  const cloth = box(x, y - 0.01, z, 0.95, 0.01, 0.75);
  const basket = box(x - 0.5, y - 0.14, z + 0.35, 0.22, 0.13, 0.15);
  const handle = max(abs(length2(x - 0.5, y - 0.27) - 0.15) - 0.015, abs(z - 0.35) - 0.015);
  const apples = min(sphere(x + 0.3, y - 0.05, z - 0.2, 0.045), sphere(x + 0.2, y - 0.05, z - 0.3, 0.045));
  return min(min(cloth, basket), min(max(handle, 0.27 - y), apples));
}

export function color(x, y, z, k) {
  if (y > 0.03 && abs(x + 0.25) < 0.1) return rgb(196, 30, 30);
  if (y > 0.03) return y > 0.26 ? rgb(150, 110, 64) : (fract(y * 20.0) < 0.4 ? rgb(150, 110, 64) : rgb(196, 156, 100));
  return step(0.5, fract(x * 5.0)) + step(0.5, fract(z * 5.0)) == 1 ? rgb(200, 40, 40) : rgb(246, 242, 236);
}
