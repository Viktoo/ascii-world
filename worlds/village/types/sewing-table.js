// A tailor's table: a black sewing machine with gold lettering, a pincushion,
// scissors, a tape measure, and a dressmaker's dummy in a half-made dress.
export const meta = {
  name: "sewing table",
  bounds: [0.95, 1.6, 0.4],
  tags: ["furniture", "prop"],
  props: { mass: 50, burns: 0.6 },
};

export function sdf(x, y, z, k) {
  const top = box(x + 0.25, y - 0.75, z, 0.6, 0.03, 0.35);
  const legs = box(abs(x + 0.25) - 0.55, y - 0.37, z, 0.03, 0.37, 0.3);
  const machine = min(box(x + 0.3, y - 0.88, z, 0.25, 0.1, 0.1), box(x + 0.48, y - 1.0, z, 0.07, 0.2, 0.08));
  const dummy = min(ellipsoid(x + 0.7, y - 1.25, z, 0.18, 0.3, 0.13), cylinder(x + 0.7, y - 0.5, z, 0.02, 0.5));
  const dress = max(cappedCone(x + 0.7, y - 0.9, z, 0.32, 0.32, 0.16), 0.6 - y);
  return min(min(top, legs), min(machine, min(dummy, dress)));
}

export function color(x, y, z, k) {
  if (x < -0.4) return y < 1.0 ? hsv(fract(hash(k.seed) * 0.8), 0.55, 0.8) : rgb(220, 200, 170);
  if (y > 0.78 && y < 1.2) return rgb(30, 30, 32);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
