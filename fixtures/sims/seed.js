// An acorn: left on the ground long enough, it becomes a sapling.
export const meta = { name: "acorn", bounds: [0.04, 0.05, 0.04], tags: ["item", "seed"], props: { mass: 0.01, alive: 1, burns: 0.5 }, spawns: ["sapling"] };

export function sdf(x, y, z, k) {
  return ellipsoid(x, y, z, 0.03, 0.04, 0.03);
}

export function color(x, y, z, k) {
  return y > 0.015 ? rgb(90, 70, 40) : rgb(150, 110, 60);
}

export function tick(s, w, k) {
  if (w.ground > 0 && w.held == 0) {
    s.s0 = s.s0 + w.dt;
  }
  if (s.s0 > 20) {
    transform(0);
  }
}
