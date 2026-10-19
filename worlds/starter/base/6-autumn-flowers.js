// A clump of autumn wildflowers, each flower on its own stem: purple New
// England asters, golden goldenrod plumes, black-eyed Susans or white
// heath asters, a whole patch of a field in one kind with a few strays.
export const meta = {
  name: "autumn wildflowers",
  bounds: [0.7, 1.0, 0.7],
  tags: ["flower", "nonsolid"],
  props: { mass: 0.2, burns: 0.8, alive: 1, fuel: 0.2 },
};

export function sdf(x, y, z, k) {
  let d = 10;
  const tall = hash(k.seed, 9) * 0.2 + (k.a > 0.3 && k.a < 0.6 ? 0.25 : 0.0);
  for (let i = 0; i < 6; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 0.9;
    const r = 0.06 + hash(k.seed, i, 2) * 0.5;
    const h = 0.45 + tall + hash(k.seed, i, 3) * 0.3;
    const tx = cos(a) * r;
    const tz = sin(a) * r;
    d = min(d, capsule(x, y, z, tx * 0.7, 0, tz * 0.7, tx, h, tz, 0.012));
    if (tall > 0.2) {
      // Goldenrod: a plume of tiny flowers arching over at the top of its stem.
      d = min(d, (cappedCone(x - tx - 0.03, y - h + 0.02, z - tz, 0.14, 0.035, 0.008) + (noise3(x * 50, y * 50, z * 50) - 0.5) * 0.012) * 0.8);
    } else {
      // A daisy-like head: a flat disc of rays round a raised centre.
      d = min(d, min(ellipsoid(x - tx, y - h, z - tz, 0.065, 0.012, 0.065), sphere(x - tx, y - h - 0.008, z - tz, 0.018)));
    }
  }
  return d;
}

export function color(x, y, z, k) {
  let f = k.a;
  if (hash(k.seed, 11) < 0.15) f = hash(k.seed, 12);
  // Below the blooms, the green stems.
  const tall = hash(k.seed, 9) * 0.2 + (k.a > 0.3 && k.a < 0.6 ? 0.25 : 0.0);
  if (y < 0.4 + tall) return mix(rgb(70, 100, 44), rgb(110, 120, 56), clamp(y, 0, 1));
  const n = noise3(x * 30, y * 30, z * 30);
  if (f >= 0.3 && f < 0.6) return mix(rgb(206, 160, 26), rgb(250, 214, 70), noise3(x * 70, y * 70, z * 70));
  // The head: a centre, rays round it with darker gaps between.
  const ray = 0.85 + 0.15 * step(0.5, fract(atan2(z, x) * 7.0 + n));
  if (f < 0.3) return n > 0.78 ? rgb(240, 196, 60) : rgb(138, 82, 196) * ray;
  if (f < 0.85) return n > 0.72 ? rgb(70, 40, 24) : rgb(246, 180, 30) * ray;
  return n > 0.8 ? rgb(240, 210, 80) : rgb(246, 244, 236) * ray;
}
