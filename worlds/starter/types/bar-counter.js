// The tavern's bar: a long counter of dark polished wood, three brass beer
// taps at one end, mugs stacked on it, a jar of pickled eggs and a bowl of
// peanuts.
export const meta = {
  name: "bar counter",
  bounds: [1.6, 1.4, 0.4],
  tags: ["furniture", "counter", "shop"],
  props: { mass: 200, heat: 30 },
};

export function sdf(x, y, z, k) {
  const body = box(x, y - 0.52, z, 1.55, 0.52, 0.32);
  const top = box(x, y - 1.06, z, 1.6, 0.025, 0.38);
  const machine = roundBox(x + 1.05, y - 1.3, z + 0.05, 0.35, 0.22, 0.22, 0.05);
  const cups = max(cylinder(x - 0.12 * floor(x / 0.12 + 0.5), y - 1.12, z, 0.04, 0.04), abs(x + 0.2) - 0.35);
  const dome = max(sphere(x + 0.5, y - 1.1, z, 0.2), 1.08 - y);
  return min(min(body, top), min(machine, min(cups, dome)));
}

export function color(x, y, z, k) {
  if (abs(x - 1.05) < 0.4 && y > 1.08) return rgb(196, 160, 70);
  if (y > 1.08 && abs(x + 0.5) < 0.22) return rgb(210, 190, 140);
  if (y > 1.08) return rgb(226, 170, 60);
  if (y > 1.03) return rgb(70, 46, 30);
  return mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6));
}
