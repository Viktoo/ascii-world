// Built-in broadleaf tree.
export const meta = { name: "oak", bounds: [3.6, 7.2, 3.6], tags: ["builtin", "tree"] };

export function sdf(x, y, z, k) {
  const ox = (hash(k.seed) - 0.5) * 1.2;
  const oz = (hash(k.seed, 2) - 0.5) * 1.2;
  const trunk = cappedCone(x, y - 1.9, z, 1.9, 0.32, 0.2);
  const c1 = sphere(x, y - 4.3, z, 2.0);
  const c2 = sphere(x - ox, y - 5.0, z - oz, 1.5);
  const c3 = sphere(x + oz, y - 3.8, z - ox, 1.4);
  const crown = smoothUnion(smoothUnion(c1, c2, 0.6), c3, 0.6);
  const bumpy = crown + (noise3(x * 1.4, y * 1.4, z * 1.4) - 0.5) * 0.7;
  return min(trunk, bumpy * 0.8);
}

export function color(x, y, z, k) {
  if (y < 2.6 && length2(x, z) < 0.45) {
    return mix(rgb(84, 62, 44), rgb(110, 84, 60), noise3(x * 6, y * 2, z * 6));
  }
  const n = noise3(x * 2.1, y * 2.1, z * 2.1);
  const hue = 0.24 + hash(k.seed, 5) * 0.08;
  return hsv(hue, 0.62, 0.36 + n * 0.22 + clamp((y - 3) * 0.04, 0, 0.12));
}
