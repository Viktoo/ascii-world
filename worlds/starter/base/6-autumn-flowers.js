// A clump of autumn wildflowers, each flower on its own stem with a leaf or
// two, and a low tuft of leaves at the foot: purple New England asters, golden goldenrod plumes, black-eyed
// Susans or white heath asters, a whole patch of a field in one kind with a
// few strays. Each head is a ring of separate petals round a domed centre,
// tipped a little to one side.
export const meta = {
  name: "autumn wildflowers",
  bounds: [0.7, 1.3, 0.7],
  tags: ["flower", "nonsolid"],
  props: { mass: 0.2, burns: 0.8, alive: 1, fuel: 0.2 },
};

export function sdf(x, y, z, k) {
  let d = 10;
  // The kind: the stand's, or a stray's; most white stands are asters.
  let f = k.a;
  if (hash(k.seed, 11) < 0.15) f = hash(k.seed, 12);
  if (f >= 0.85 && hash(k.seed, 13) < 0.7) f = 0.15;
  const gold = f > 0.3 && f < 0.6;
  const susan = f >= 0.6 && f < 0.85;
  // Black-eyed Susans stand tall over the rest.
  const tall = hash(k.seed, 9) * 0.2 + (gold ? 0.25 : 0.0) + (susan ? 0.32 : 0.0);
  // White asters grow thinly, three to a clump.
  const heads = f >= 0.85 ? 3 : 6;
  // Asters have many narrow rays; Susans and white asters fewer, broader.
  const rays = f < 0.3 ? 16 : 11;
  const sector = 6.2832 / rays;
  // A low tuft of leaves where the stems come up.
  d = min(d, (ellipsoid(x, y - 0.05, z, 0.16, 0.075, 0.16) + (noise3(x * 30, y * 30, z * 30) - 0.5) * 0.03) * 0.8);
  for (let i = 0; i < 6; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 0.9;
    const r = 0.06 + hash(k.seed, i, 2) * 0.5;
    const h = (i < heads ? 0.4 : -9) + tall + hash(k.seed, i, 3) * 0.3;
    const tx = cos(a) * r;
    const tz = sin(a) * r;
    // The stem bows a little outwards; a leaf part way up.
    d = min(d, capsule(x, y, z, tx * 0.6, 0, tz * 0.6, tx, h, tz, 0.013));
    const lh = h * (0.35 + 0.2 * hash(k.seed, i, 4));
    const la = a + 1.6;
    d = min(d, ellipsoid(x - tx * 0.8 - cos(la) * 0.045, y - lh - 0.02, z - tz * 0.8 - sin(la) * 0.045, 0.05, 0.006, 0.016) * 0.7);
    const hx = x - tx;
    const hy = y - h;
    const hz = z - tz;
    {
      if (gold) {
        // Goldenrod: a plume of tiny flowers arching over at the top.
        d = min(d, (cappedCone(hx - 0.03, hy + 0.02, hz, 0.14, 0.035, 0.008) + (noise3(x * 50, y * 50, z * 50) - 0.5) * 0.012) * 0.8);
      } else {
        // Tip the head towards its own side so it shows its face.
        const q = rotZ(hx, hy, hz, 0.35 + hash(k.seed, i, 5) * 0.5);
        const p = rotY(q.x, q.y, q.z, a);
        const pr = length2(p.x, p.z);
        const pa = atan2(p.z, p.x);
        const s = mod(pa + sector * 0.5, sector) - sector * 0.5;
        const u = cos(s) * pr;
        const v = sin(s) * pr;
        // Petals curve up a touch towards their tips.
        const pl = f < 0.3 ? 0.05 : 0.055;
        const pw = f < 0.3 ? 0.007 : 0.012;
        const petal = ellipsoid(u - 0.022 - pl * 0.5, p.y - pr * pr * 1.2, v, pl * 0.55, 0.0035, pw) * 0.6;
        const eye = ellipsoid(p.x, p.y - 0.006, p.z, 0.02, 0.014, 0.02);
        d = min(d, min(petal, eye));
      }
    }
  }
  return d;
}

export function color(x, y, z, k) {
  let f = k.a;
  if (hash(k.seed, 11) < 0.15) f = hash(k.seed, 12);
  if (f >= 0.85 && hash(k.seed, 13) < 0.7) f = 0.15;
  const n = noise3(x * 30, y * 30, z * 30);
  const tall = hash(k.seed, 9) * 0.2 + (f > 0.3 && f < 0.6 ? 0.25 : 0.0) + (f >= 0.6 && f < 0.85 ? 0.32 : 0.0);
  // Below the blooms, the green stems and leaves.
  if (y < 0.37 + tall) return mix(rgb(56, 96, 40), rgb(104, 128, 52), clamp(y + n * 0.3, 0, 1));
  if (f >= 0.3 && f < 0.6) return mix(rgb(206, 156, 22), rgb(252, 216, 66), noise3(x * 70, y * 70, z * 70));
  // Near a head's middle, its centre; further out, the petals, paler at the tip.
  let c = 1;
  let best = 1;
  for (let i = 0; i < 6; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 0.9;
    const r = 0.06 + hash(k.seed, i, 2) * 0.5;
    const h = 0.4 + tall + hash(k.seed, i, 3) * 0.3;
    const dd = length3(x - cos(a) * r, y - h, z - sin(a) * r);
    if (dd < best) best = dd;
  }
  c = best;
  const tip = smoothstep(0.03, 0.075, c);
  if (f < 0.3) return c < 0.022 ? rgb(240, 190, 50) : mix(rgb(112, 64, 186), rgb(168, 128, 226), tip);
  if (f < 0.85) return c < 0.022 ? rgb(64, 36, 22) : mix(rgb(236, 150, 20), rgb(252, 196, 46), tip);
  return c < 0.022 ? rgb(242, 206, 70) : mix(rgb(246, 244, 236), rgb(255, 255, 252), tip);
}
