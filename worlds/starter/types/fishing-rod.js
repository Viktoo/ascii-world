// A fishing rod: a long tapered rod with a reel near the grip, line strung
// to its tip. Held by the grip; its tip whips out over the water.
export const meta = {
  name: "fishing rod",
  bounds: [0.08, 0.08, 1.25],
  tags: ["item", "tool"],
  props: { mass: 0.4, burns: 0.4 },
  tool: { grip: [0, 0, -0.95], tip: [0, 0, 1.2], motions: ["swing"], sharp: 0 },
};

export function sdf(x, y, z, k) {
  const rod = cone(x, y, z, 0, 0, -1.15, 0, 0, 1.2, 0.018, 0.004);
  const reel = max(length2(x + 0.03, y + 0.05) - 0.04, abs(z + 0.75) - 0.025);
  return min(rod, reel);
}

export function color(x, y, z, k) {
  if (z < -0.7) return rgb(40, 40, 44);
  return mix(rgb(30, 60, 120), rgb(200, 200, 200), step(0.0, z - 1.05));
}
