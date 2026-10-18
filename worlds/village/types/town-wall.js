// A length of Marinella's old town wall: 8 m of warm limestone 1.6 m
// thick, 5.5 m high with a walk behind square merlons, its footing going
// deep into the hillside so it can stand across a terrace's step.
export const meta = {
  name: "town wall",
  bounds: [4.05, 7.0, 0.85],
  tags: ["structure", "wall"],
  props: { mass: 100000 },
  sound: { hard: 1, dry: 0.4, ring: 0.1 },
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 0.75, z, 4.0, 5.25, 0.8);
  const mx = x - 2.0 * floor(x / 2.0) - 1.0;
  const merlons = max(box(mx, y - 6.4, z - 0.55, 0.55, 0.4, 0.25), abs(x) - 4.0);
  return min(body, merlons);
}

export function color(x, y, z, k) {
  const n = noise3(x * 1.6, y * 1.6, z * 1.6);
  const course = fract(y * 2.2) < 0.08 ? 0.86 : 1;
  const joint = fract(x * 1.1 + floor(y * 2.2) * 0.5) < 0.05 ? 0.88 : 1;
  const stone = mix(rgb(196, 182, 152), rgb(222, 208, 178), n) * course * joint;
  if (y > 5.95) return stone * 1.05;
  return stone;
}
