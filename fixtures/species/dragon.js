// A dragon body, as the builder might write it: four legs, a long neck and
// tail, two wings. Roles: raise = wing up, reach = wing wraps forward
// (a hug), lean, head, crouch, spread = wings spread wide.
export const meta = { name: "dragon", bounds: [5.5, 4.4, 6.5], tags: ["body"],
  props: { mass: 3000, alive: 1, heat: 40 },
  body: { height: 3.6, eye: 3.3, radius: 1.6, reach: 3.5, grip: [0, 2.6, 4.6], seat: [0, 2.3, 0.2],
          roles: ["raise", "reach", "lean", "head", "crouch", "spread"], gait: "quad", flies: true, arms: false,
          look: { hue: [0, 1], horns: [0, 1] } } };

export function sdf(x, y, z, k) {
  const crouch = clamp(k.s6, 0, 1);
  const by = 1.7 - crouch * 0.9;
  const sw = sin(k.f) * 0.35 * (1 - crouch);
  const body = ellipsoid(x, y - by, z, 1.0, 0.85, 2.0);
  const legs = min(
    capsule(x, y, z, -0.7, by - 0.4, 1.2, -0.75, 0.2, 1.3 + sw, 0.28),
    capsule(x, y, z, 0.7, by - 0.4, 1.2, 0.75, 0.2, 1.3 - sw, 0.28),
    capsule(x, y, z, -0.7, by - 0.4, -1.2, -0.75, 0.2, -1.2 - sw, 0.3),
    capsule(x, y, z, 0.7, by - 0.4, -1.2, 0.75, 0.2, -1.2 + sw, 0.3)
  );
  const hd = clamp(k.s5, -0.5, 1);
  const lean = clamp(k.s4, -0.3, 0.6);
  const hy = by + 1.5 - hd * 1.4 - lean * 0.8;
  const hz = 3.2 + lean * 0.6;
  const neck = capsule(x, y, z, 0, by + 0.4, 1.6, 0, hy, hz, 0.35);
  const head = ellipsoid(x, y - hy, z - hz - 0.4, 0.4, 0.35, 0.7);
  const horns = clamp(k.b, 0, 1) * 0.5;
  const horn = min(capsule(x, y, z, 0.2, hy + 0.2, hz, 0.3, hy + 0.4 + horns, hz - 0.4, 0.07), capsule(x, y, z, -0.2, hy + 0.2, hz, -0.3, hy + 0.4 + horns, hz - 0.4, 0.07));
  const tail = capsule(x, y, z, 0, by, -1.8, sin(k.f * 0.5) * 0.6 + clamp(k.s7, -1, 1) * 0.4, 0.5, -5.8, 0.25);
  // Wings: hinged at the shoulders, raised (s0/s1), wrapped forward (s2/s3), spread (s7).
  const sp = clamp(abs(k.s7), 0, 1);
  const wl = rotZ(x + 0.9, y - by - 0.6, z - 0.6, 0.5 + clamp(k.s0, 0, 1) * 0.9 - sp * 0.4);
  const ql = rotY(wl.x, wl.y, wl.z, -clamp(k.s2, 0, 1) * 1.2);
  const wingL = roundBox(ql.x + 1.6 + sp * 0.8, ql.y, ql.z, 1.6 + sp * 0.8, 0.05, 1.1, 0.04);
  const wr = rotZ(x - 0.9, y - by - 0.6, z - 0.6, -0.5 - clamp(k.s1, 0, 1) * 0.9 + sp * 0.4);
  const qr = rotY(wr.x, wr.y, wr.z, clamp(k.s3, 0, 1) * 1.2);
  const wingR = roundBox(qr.x - 1.6 - sp * 0.8, qr.y, qr.z, 1.6 + sp * 0.8, 0.05, 1.1, 0.04);
  return min(smoothUnion(smoothUnion(body, legs, 0.2), smoothUnion(neck, head, 0.15), 0.2), horn, smoothUnion(tail, body, 0.3), wingL, wingR);
}

export function color(x, y, z, k) {
  const scales = hsv(k.a, 0.55, 0.45);
  if (abs(y - 0.12) < 0.0 || (y < 1.2 && abs(x) < 0.5 && abs(z) < 1.6)) {
    return mix(scales, rgb(230, 200, 150), 0.5);
  }
  return mix(scales, hsv(k.a, 0.6, 0.25), noise3(x * 2, y * 2, z * 2) * 0.5);
}
