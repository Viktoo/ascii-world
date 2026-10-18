// Built-in walkway: one straight span of a way off the ground (a rope
// bridge between trees, a boardwalk on a cliff, a stone way between
// towers), laid by the engine end to end between what holds it up. The
// deck runs along z, its top at y = 0 in the middle, rising `rise` over its
// length (in steps where it is steep), with rails along its sides.
// k.a: half-length (0.5–8 m), k.b: half-width (0.3–1.5 m), k.c: rise over
// the span (0 = 8 m down … 0.5 level … 1 = 8 m up), k.d: 0 rope and planks …
// 0.5 timber … 1 stone, k.e: shade.
export const meta = {
  name: "walkway",
  bounds: [1.65, 5.3, 8.1],
  tags: ["builtin", "walkway", "bridge", "structure"],
  props: { mass: 3000 },
  sound: { hard: 0.6, dry: 0.7, ring: 0.05 },
};

export function sdf(x, y, z, k) {
  const L = 0.5 + clamp(k.a, 0, 1) * 7.5;
  const W = 0.3 + clamp(k.b, 0, 1) * 1.2;
  const rise = (clamp(k.c, 0, 1) - 0.5) * 16;
  const s = rise / (2 * L);
  const u = clamp(z, -L, L);
  let top = u * s;
  // How much a distance measured up and down overstates the true one: on a
  // slope by its slant; in steps (steep), more.
  let q = 1 / sqrt(1 + s * s);
  // Steep: in steps of 0.2 m.
  if (abs(s) > 0.35) {
    const tread = 0.2 / abs(s);
    top = (floor(u / tread) + 0.5) * tread * s;
    q = 0.5;
  }
  const rope = k.d < 0.33;
  const stone = k.d > 0.66;
  const th = rope ? 0.06 : (stone ? 0.35 : 0.1);
  // Rope bridges: planks with gaps between them.
  const gap = rope ? max(abs(fract(z * 2.2) - 0.5) - 0.38, 0) : 0;
  const deck = max(max(abs(x) - W, abs(z) - L), max(y - top, top - th - y) * q) + gap;
  const rail_h = top + (stone ? 0.55 : 0.95);
  let rails = 1e9;
  if (rope) {
    // A rope along each side, posts at its ends, hangers between.
    const r1 = length2(abs(x) - W, (y - rail_h) * q) - 0.025;
    const along = max(r1, abs(z) - L);
    const hang = max(length2(abs(x) - W, fract(z * 1.25) / 1.25 - 0.4) - 0.012, max(y - rail_h, top - y));
    const post = max(length2(abs(x) - W, abs(z) - L + 0.08) - 0.06, max(y - rail_h - 0.1, top - th - y));
    rails = min(min(along, max(hang, abs(z) - L)), post);
  } else if (stone) {
    rails = max(max(abs(abs(x) - W + 0.15) - 0.15, abs(z) - L), max(y - rail_h, top - th - y) * q);
  } else {
    const rail = max(max(abs(abs(x) - W + 0.04) - 0.04, abs(z) - L), (abs(y - rail_h + 0.04) - 0.05) * q);
    const posts = max(max(abs(abs(x) - W + 0.05) - 0.05, abs(fract(z * 0.6 + 0.5) / 0.6 - 0.833) - 0.05), max(y - rail_h, top - th - y) * q);
    rails = min(rail, max(posts, abs(z) - L));
  }
  return min(deck, rails);
}

export function color(x, y, z, k) {
  const n = noise3(x * 3, y * 3, z * 3);
  const shade = 0.75 + k.e * 0.4;
  if (k.d > 0.66) {
    const stone = mix(rgb(160, 152, 136), rgb(200, 192, 176), n) * shade;
    return mix(stone, stone * 0.75, step(0.93, fract(z * 1.4)));
  }
  const wood = mix(rgb(100, 72, 46), rgb(136, 100, 64), n) * shade;
  const plank = mix(wood, wood * 0.72, step(0.88, fract(z * 3.1)));
  if (k.d < 0.33) {
    const W = 0.3 + clamp(k.b, 0, 1) * 1.2;
    if (abs(abs(x) - W) < 0.04) return rgb(150, 128, 92) * shade;
  }
  return plank;
}
