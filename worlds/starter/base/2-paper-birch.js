// A paper birch: one or two slender white trunks marked with black, and a
// narrow, airy crown of small leaves gone bright gold.
export const meta = {
  name: "paper birch",
  bounds: [2.6, 9.0, 2.6],
  tags: ["birch"],
  props: { mass: 900, burns: 0.6, alive: 1, fuel: 0.9 },
};

export function sdf(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.3;
  const two = hash(k.seed, 2) > 0.55;
  const lean = two ? 0.5 * s : 0.0;
  const t1 = capsule(x, y, z, 0, 0, 0, lean, 6.0 * s, 0.1, 0.16 * s);
  const t2 = two ? capsule(x, y, z, 0.1, 0, 0, -0.9 * s, 5.4 * s, 0.3, 0.13 * s) : 10.0;
  const trunk = min(t1, t2);
  const far = length3(x, y - 6.0 * s, z) - 3.0 * s - 0.3;
  if (far > 0.4) return min(trunk, far * 0.8);
  const c1 = ellipsoid(x - lean * 0.6, y - 6.2 * s, z, 1.9 * s, 2.7 * s, 1.8 * s);
  const c2 = two ? ellipsoid(x + 0.9 * s, y - 5.4 * s, z - 0.3, 1.4 * s, 2.0 * s, 1.4 * s) : 10.0;
  const crown = smoothUnion(c1, c2, 0.6) + (noise3(x * 2.0, y * 2.0, z * 2.0) - 0.5) * 0.6;
  return min(trunk, crown * 0.8);
}

export function color(x, y, z, k) {
  const s = 0.75 + hash(k.seed) * 0.3;
  const n = noise3(x * 3.0, y * 3.0, z * 3.0);
  if (y < 4.6 * s && length2(x, z) < 1.4 * s && n < 0.9) {
    // Bark: white, ringed with dark marks.
    const mark = noise3(x * 4, y * 9, z * 4);
    return mark > 0.7 ? rgb(46, 42, 40) : mix(rgb(222, 218, 206), rgb(244, 240, 230), noise3(x * 8, y * 3, z * 8));
  }
  if (n > 0.86) return rgb(190, 110, 40);
  return hsv(0.115 + (n - 0.5) * 0.03, 0.8, 0.62 + n * 0.3);
}
