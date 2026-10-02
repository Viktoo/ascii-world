// A wool cloak for the human body: hangs from the shoulders down the back.
export const meta = { name: "red cloak", bounds: [0.95, 2.4, 0.95], tags: ["layer"], fits: "figure",
  props: { mass: 2, burns: 0.8, fuel: 0.5 } };

export function sdf(x, y, z, k) {
  const h = clamp(k.a, 1.3, 2.0) / 1.75;
  const w = clamp(k.b, 0.7, 1.4);
  const hip = 0.9 - clamp(k.s6, 0, 1) * 0.38;
  const q = rotX(x / h, y / h - hip, z / h, -clamp(k.s4, -0.3, 0.6));
  const sway = sin(k.f) * 0.04;
  const back = roundBox(q.x, q.y + 0.9 - 1.0, q.z + (0.11 * w + 0.06) + sway, 0.27 * w, 0.48, 0.015, 0.01);
  return back * h;
}

export function color(x, y, z, k) {
  return rgb(150, 30, 35);
}
