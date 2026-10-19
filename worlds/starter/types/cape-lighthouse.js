// The lighthouse on Lantern Point: a tall white round tower banded in black,
// a gallery and a glass lantern room with its great lamp, standing at the
// end of the keeper's house, two floors of white clapboard with dark green
// shutters and a red roof. The tower looks out to sea (+z); the house's
// door faces the path at its back (-z). Inside the tower a stair spirals
// up as a ladder to the lantern.
export const meta = {
  name: "cape lighthouse",
  bounds: [6.4, 20.2, 5.2],
  tags: ["building", "landmark", "lighthouse"],
  props: { mass: 100000, light: 1 },
  sound: { hard: 1, dry: 0.5, ring: 0.3 },
  anchors: [
    { kind: "door", at: [-2.2, 0, -4.2], face: 180, size: [1.1, 2.2] },
    { kind: "light", at: [3.0, 18.4, 2.0] },
    { kind: "light", at: [-2.2, 2.4, -4.6] },
    { kind: "bed", at: [-4.0, 0.05, 0.0] },
  ],
};

export function sdf(x, y, z, k) {
  // The keeper's house.
  const hx = x + 2.2;
  const house = box(hx, y - 3.2, z + 0.6, 3.9, 3.2, 3.6);
  const room = box(hx, y - 1.6, z + 0.6, 3.6, 1.45, 3.3);
  const door = box(hx, y - 1.1, z + 4.2, 0.55, 1.1, 0.5);
  const roof = max(box(hx, y - 7.0, z + 0.6, 4.2, 0.9, 3.9), (y - 7.8 + abs(z + 0.6) * 0.42) * 0.9);
  // The tower.
  const tx = x - 3.0;
  const tz = z - 2.0;
  const tower = cappedCone(tx, y - 8.0, tz, 8.0, 1.9, 1.35);
  const gallery = cylinder(tx, y - 16.1, tz, 1.95, 0.12);
  const rail = max(abs(length2(tx, tz) - 1.9) - 0.03, abs(y - 16.6) - 0.45);
  const lantern = cylinder(tx, y - 17.3, tz, 1.0, 1.1);
  const dome = max(length3(tx, y - 18.4, tz) - 1.1, 18.4 - y);
  const vane = cylinder(tx, y - 19.7, tz, 0.06, 0.3);
  return min(min(max(house, -min(room, door)), roof), min(min(tower, gallery), min(rail, min(min(lantern, dome), vane))));
}

export function color(x, y, z, k) {
  const tx = x - 3.0;
  const tz = z - 2.0;
  const n = noise3(x * 1.4, y * 1.4, z * 1.4);
  if (length2(tx, tz) < 2.05 && y > 7.2 - 0.0 && x > 0.9) {
    if (y > 18.35) return rgb(40, 52, 50);
    if (y > 16.25) return glow(rgb(255, 246, 200));
    if (y > 15.9) return rgb(40, 52, 50);
    return fract(y / 4.0) < 0.25 ? rgb(34, 36, 38) : rgb(246, 244, 238);
  }
  if (length2(tx, tz) < 2.0 && x > 1.0) return fract(y / 4.0) < 0.25 ? rgb(34, 36, 38) : rgb(246, 244, 238);
  if (y > 6.35) return mix(rgb(160, 40, 34), rgb(184, 54, 44), n);
  const hx = x + 2.2;
  if (abs(z + 0.6) > 3.55 && y > 0.4) {
    const wx = hx - 2.4 * floor(hx / 2.4 + 0.5);
    const fy = y - 3.2 * floor(y / 3.2);
    if (abs(wx) < 0.45 && abs(fy - 1.7) < 0.65 && abs(hx) > 0.8) return glow(rgb(46, 44, 40), 0.3);
    if (abs(abs(wx) - 0.72) < 0.26 && abs(fy - 1.7) < 0.66 && abs(hx) > 0.8) return rgb(46, 112, 74);
  }
  if (y < 0.45) return rgb(170, 160, 142);
  return rgb(240, 238, 230) * (fract(y * 4.0) < 0.12 ? 0.86 : 1.0) * (0.94 + 0.06 * n);
}
