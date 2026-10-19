// A harbor seal, lying along +z with its belly near y = 0: a plump, spotted
// grey body tapering to hind flippers that sweep side to side as it swims
// (k.f), a round head with big dark eyes and whiskers, two front flippers.
// Look: a = shade. Roles: s4 = lean (dive or rise), s5 = head (nod),
// s7 = spread (the flippers' sweep, wider).
export const meta = { name: "seal", bounds: [0.62, 0.8, 0.95], tags: ["body"],
  props: { mass: 90, alive: 1, burns: 0.05, wet: 1 },
  body: { height: 0.45, eye: 0.36, radius: 0.32, reach: 0.4, grip: [0, 0.3, 0.85],
          roles: ["lean", "head", "spread"], gait: "slither", flies: false, arms: false,
          look: { shade: [0.25, 0.75] } } };

export function sdf(x, y, z, k) {
  const lean = clamp(k.s4, -0.3, 0.6);
  const p = rotX(x, y - 0.25, z, lean);
  const sweep = (0.08 + clamp(k.s7, -1, 1) * 0.03) * sin(k.f - p.z * 2.6);
  const w = clamp(-p.z, 0, 0.9);
  const px = p.x - sweep * w;
  const nod = clamp(k.s5, -1, 1) * 0.3 * step(0.35, p.z);
  const h = rotX(p.x, p.y, p.z - 0.4, nod);
  const body = smoothUnion(ellipsoid(px, p.y, p.z - 0.05, 0.27, 0.23, 0.55), ellipsoid(px, p.y + 0.02, p.z + 0.45, 0.13, 0.11, 0.28), 0.12);
  const head = sphere(h.x, h.y - 0.08, h.z - 0.22, 0.16);
  const snout = ellipsoid(h.x, h.y - 0.04, h.z - 0.38, 0.09, 0.07, 0.08);
  const fl = triangle(abs(p.x), p.y, p.z, 0.2, -0.12, 0.25, 0.42, -0.2, 0.08, 0.22, -0.16, 0.05, 0.02);
  const hind = triangle(abs(px), p.y, p.z, 0.02, 0, -0.62, 0.22, 0.0, -0.9, 0.03, 0, -0.85, 0.02);
  return min(smoothUnion(smoothUnion(body, head, 0.1), snout, 0.04), min(fl, hind));
}

export function color(x, y, z, k) {
  const v = 0.3 + clamp(k.a, 0, 1) * 0.3;
  if (z > 0.62 && y > 0.32 && abs(abs(x) - 0.07) < 0.035) return rgb(16, 16, 18);
  const spot = noise3(x * 14, y * 14, z * 14) > 0.68 ? 0.6 : 1.0;
  if (y < 0.1) return rgb(176, 172, 160) * spot;
  return hsv(0.1, 0.12, v) * spot;
}
