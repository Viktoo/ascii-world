// A rock crab lying flat with its front to +z: a broad, rounded reddish-brown
// shell, two stalked eyes, a big claw on each side held up before it, and
// four legs a side that scuttle (k.f). Look: a = shade.
// Roles: s5 = head (claws raise), s7 = spread
// (claws open wide).
export const meta = { name: "crab", bounds: [0.34, 0.2, 0.3], tags: ["body"],
  props: { mass: 0.6, alive: 1, burns: 0.05, wet: 0.5 },
  body: { height: 0.12, eye: 0.11, radius: 0.18, reach: 0.25, grip: [0, 0.06, 0.24],
          roles: ["head", "spread"], gait: "quad", flies: false, arms: false,
          look: { shade: [0.3, 0.8] } } };

export function sdf(x, y, z, k) {
  const shell = ellipsoid(x, y - 0.075, z, 0.15, 0.045, 0.11);
  const ax = abs(x);
  const side = x > 0 ? 1.0 : -1.0;
  // Legs: four a side, each a bent capsule, rocking with the walk.
  let legs = 10.0;
  for (let i = 0; i < 4; i++) {
    const lz = -0.06 + i * 0.04;
    const ph = sin(k.f + i * 1.6 + side * 1.57) * 0.03;
    legs = min(legs, min(capsule(ax, y, z, 0.12, 0.07, lz, 0.22, 0.08, lz + ph, 0.012), capsule(ax, y, z, 0.22, 0.08, lz + ph, 0.27, 0.0, lz + ph * 1.5, 0.01)));
  }
  const lift = clamp(k.s5, 0, 1) * 0.05;
  const open = clamp(k.s7, 0, 1) * 0.03;
  const arm = capsule(ax, y, z, 0.1, 0.07, 0.08, 0.16, 0.08 + lift, 0.17, 0.016);
  const claw = min(ellipsoid(ax - 0.16, y - 0.09 - lift - open, z - 0.22, 0.04, 0.02, 0.05), ellipsoid(ax - 0.165, y - 0.06 - lift + open, z - 0.22, 0.035, 0.015, 0.045));
  const eyes = capsule(ax, y, z, 0.03, 0.1, 0.09, 0.035, 0.14, 0.1, 0.008);
  return min(min(shell, legs), min(min(arm, claw), eyes));
}

export function color(x, y, z, k) {
  const v = 0.42 + clamp(k.a, 0, 1) * 0.25;
  if (y > 0.125 && z > 0.08) return rgb(14, 14, 16);
  if (y < 0.05 && length2(x, z) < 0.13) return rgb(230, 210, 180);
  return hsv(0.03, 0.62, v) * (0.9 + 0.1 * noise3(x * 30, y * 30, z * 30));
}
