// A pale limestone boulder, weathered and pitted, lichen on its top.
export const meta = {
  name: "limestone boulder",
  bounds: [1.6, 1.3, 1.6],
  tags: ["rock"],
  props: { mass: 1800 },
  sound: { hard: 0.9, dry: 0.8, ring: 0.1 },
};

export function sdf(x, y, z, k) {
  const s = 0.5 + hash(k.seed) * 0.8;
  const q = rotY(x, y, z, hash(k.seed, 2) * 6.28);
  const b = roundBox(q.x, q.y - 0.5 * s, q.z, 0.95 * s, 0.55 * s, 0.75 * s, 0.4 * s);
  return (b + (noise3(x * 2.2, y * 2.2, z * 2.2) - 0.5) * 0.3 * s) * 0.8;
}

export function color(x, y, z, k) {
  const n = noise3(x * 3, y * 3, z * 3);
  if (y > 0.7 && n > 0.62) return rgb(178, 176, 120);
  return mix(rgb(196, 188, 168), rgb(226, 220, 200), n);
}
