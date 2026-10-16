// Built-in conjuring: drawn where something is coming into the world. A
// thin violet sigil on the ground, two broken rings turning against each
// other, and motes of light spiralling up from it. s0 = time (seconds);
// s1 = arrival (0 while it is still coming; 0..1 as it comes: the rings
// open out and fade, the motes rise and go out). Origin on the ground.
export const meta = { name: "conjuring", bounds: [1.1, 1.7, 1.1], tags: ["builtin", "effect", "nonsolid"] };

export function sdf(x, y, z, k) {
  const t = k.s0;
  const a = clamp(k.s1, 0, 1);
  const fade = 1 - a;
  const yy = y - 0.03;
  const rr = length2(x, z);
  const ang = atan2(z, x);
  // The outer ring: three arcs turning slowly; on arrival it opens out.
  const r1 = 0.62 + a * 0.4;
  const th1 = 0.024 * fade + 0.003;
  const gap1 = sin(3 * (ang + t * 0.5));
  const ring1 = max(length2(rr - r1, yy) - th1, (-0.35 - gap1) * 0.12);
  // The inner ring: five shorter arcs the other way.
  const r2 = 0.42 + a * 0.55;
  const th2 = 0.015 * fade + 0.002;
  const gap2 = sin(5 * (ang - t * 0.8));
  const ring2 = max(length2(rr - r2, yy) - th2, (-0.1 - gap2) * 0.08);
  let d = min(ring1, ring2);
  // Motes: each rises from the ring in a slow spiral and dwindles; on
  // arrival they all go up fast and out.
  for (let i = 0; i < 7; i++) {
    const ph = fract(t * (0.32 + 0.05 * hash(k.seed, i)) + i / 7 + a * 0.8);
    const h = ph * (0.95 + a * 0.6);
    const an = i * 0.8976 + t * 0.9 + ph * 3.2;
    const rad = 0.55 * (1 - ph * 0.65) + a * 0.3;
    const size = (0.03 * (1 - ph) + 0.006) * fade;
    d = min(d, sphere(x - cos(an) * rad, y - 0.04 - h, z - sin(an) * rad, size + 0.001));
  }
  return d;
}

export function color(x, y, z, k) {
  // Pale pink at the heart, violet at the edges, and never still: the rings
  // shimmer along their length, the motes twinkle.
  const t = k.s0;
  const rr = length2(x, z);
  const edge = clamp(abs(rr - 0.52) * 6 + y * 0.6, 0, 1);
  const c = mix(rgb(246, 168, 236), rgb(158, 98, 255), edge * 0.65);
  const shimmer = 0.62 + 0.38 * sin(t * 4.1 + atan2(z, x) * 7 + y * 18 + hash(k.seed) * 6);
  return glow(c, clamp(shimmer, 0.25, 1));
}
