"""The lettered and walled types: the town wall, the gate, the watchtower."""
import os

from glyphs import block

HERE = os.path.dirname(os.path.abspath(__file__))


def write(name, src):
    with open(os.path.join(HERE, "types", name + ".js"), "w") as f:
        f.write(src.lstrip())


STONE = """
  const n = noise3(x * 1.6, y * 1.6, z * 1.6);
  const course = fract(y * 2.2) < 0.08 ? 0.86 : 1;
  const joint = fract(x * 1.1 + floor(y * 2.2) * 0.5) < 0.05 ? 0.88 : 1;
  const stone = mix(rgb(196, 182, 152), rgb(222, 208, 178), n) * course * joint;"""


def wall():
    write("town-wall", f"""
// A length of Marinella's old town wall: 8 m of warm limestone 1.6 m
// thick, 5.5 m high with a walk behind square merlons, its footing going
// deep into the hillside so it can stand across a terrace's step.
export const meta = {{
  name: "town wall",
  bounds: [4.05, 7.0, 0.85],
  tags: ["structure", "wall"],
  props: {{ mass: 100000 }},
  sound: {{ hard: 1, dry: 0.4, ring: 0.1 }},
}};

export function sdf(x, y, z, k) {{
  const body = box(x, y - 0.75, z, 4.0, 5.25, 0.8);
  const mx = x - 2.0 * floor(x / 2.0) - 1.0;
  const merlons = max(box(mx, y - 6.4, z - 0.55, 0.55, 0.4, 0.25), abs(x) - 4.0);
  return min(body, merlons);
}}

export function color(x, y, z, k) {{{STONE}
  if (y > 5.95) return stone * 1.05;
  return stone;
}}
""")


def gate():
    text = block([("MARINELLA", 0, 6.15, 0.95, {"stroke": 0.15})])
    write("town-gate", f"""
// Marinella's land gate in the town wall: a square gatehouse of limestone
// with merlons along its top, a round arch through it wide enough for a
// cart, and MARINELLA carved and painted in deep blue over the arch on its
// outer face (+z, toward the road); a lantern hangs in the arch.
export const meta = {{
  name: "town gate",
  bounds: [5.6, 9.6, 2.3],
  tags: ["building", "landmark", "gate"],
  props: {{ mass: 100000, light: 1 }},
  sound: {{ hard: 1, dry: 0.4, ring: 0.3 }},
  anchors: [{{ kind: "light", at: [0, 4.0, 0] }}],
}};

export function sdf(x, y, z, k) {{
  const body = box(x, y - 2.8, z, 5.5, 6.0, 2.2);
  const arch = min(box(x, y - 1.4, z, 1.9, 1.6, 2.6), max(length2(x, y - 3.0) - 1.9, abs(z) - 2.6));
  const mx = x - 1.8 * floor(x / 1.8) - 0.9;
  const merlons = max(box(mx, y - 9.15, abs(z) - 1.95, 0.5, 0.35, 0.25), abs(x) - 5.5);
  const mz = z - 1.8 * floor(z / 1.8) - 0.9;
  const merlons2 = max(box(abs(x) - 5.25, y - 9.15, mz, 0.25, 0.35, 0.5), abs(z) - 2.2);
  const lamp = min(cappedCone(x, y - 3.9, z, 0.22, 0.14, 0.2), cylinder(x, y - 4.5, z, 0.02, 0.4));
  return min(min(max(body, -arch), lamp), min(merlons, merlons2));
}}

export function color(x, y, z, k) {{
  if (y > 3.6 && y < 4.15 && length2(x, z) < 0.3) return glow(rgb(255, 210, 140));{STONE}
  if (z > 2.15 && abs(x) < 4.9 && abs(y - 6.6) < 0.95) {{
    const tx = -x;
    const ty = y;
{text}
    if (lit0 > 0) return rgb(30, 64, 140);
    if (abs(abs(x) - 4.85) < 0.08 || abs(abs(y - 6.6) - 0.9) < 0.08) return rgb(30, 64, 140);
    return rgb(240, 234, 220);
  }}
  if (length2(x, y - 3.0) < 2.35 && length2(x, y - 3.0) > 1.9 && y > 2.9) return rgb(238, 232, 218);
  return stone;
}}
""")


def tower():
    write("watchtower", f"""
// The watchtower at the corner of Marinella's wall, 15 m of limestone:
// a door at its foot (+z, toward the village), and inside a wooden ladder
// up the back wall through an opening in the floor of the lookout, where
// a parapet runs round under four corner posts holding a little tiled
// roof. From there the watch sees the bay, the cape and the road.
export const meta = {{
  name: "watchtower",
  bounds: [2.75, 16.6, 2.75],
  tags: ["building", "landmark", "lookout"],
  props: {{ mass: 100000 }},
  sound: {{ hard: 1, dry: 0.4, ring: 0.2 }},
  anchors: [
    {{ kind: "door", at: [0, 0, 2.5], face: 0, size: [1.1, 2.2] }},
    {{ kind: "stairs", at: [0, 0, -1.2], to: [0, 12.15, 0.2], face: 180 }},
  ],
}};

export function sdf(x, y, z, k) {{
  const shaft = box(x, y - 6.0, z, 2.5, 6.0, 2.5);
  const inside = box(x, y - 5.9, z, 2.1, 5.9, 2.1);
  const door = box(x, y - 1.1, z - 2.5, 0.55, 1.1, 0.6);
  const slits = min(box(x, y - 5.5, z, 0.12, 0.6, 3.0), box(x, y - 8.5, z, 3.0, 0.6, 0.12));
  const walls = max(shaft, -min(min(inside, door), slits));
  // The lookout's floor, with the ladder's opening at the back.
  const floor0 = max(box(x, y - 12.05, z, 2.7, 0.1, 2.7), -box(x, y - 12.05, z + 1.7, 0.55, 0.3, 0.45));
  const parapet = max(box(x, y - 12.65, z, 2.7, 0.55, 2.7), -box(x, y - 12.8, z, 2.45, 0.8, 2.45));
  const posts = box(abs(x) - 2.5, y - 13.9, abs(z) - 2.5, 0.16, 1.8, 0.16);
  const roof = max((max(abs(x), abs(z)) - 3.0 + (y - 15.6) * 1.3) * 0.6, max(15.6 - y, y - 16.6));
  const eave = box(x, y - 15.6, z, 3.0, 0.08, 3.0);
  // The ladder: two rails and rungs 0.3 m apart up the back wall.
  const rails = box(abs(x) - 0.3, y - 6.1, z + 1.95, 0.04, 6.1, 0.04);
  const rungs = max(box(x, y - 0.3 * floor(y / 0.3) - 0.15, z + 1.95, 0.3, 0.025, 0.025), y - 12.1);
  return min(min(min(walls, floor0), min(parapet, posts)), min(min(roof, eave) , min(rails, rungs)));
}}

export function color(x, y, z, k) {{
  if (y > 15.5) return mix(rgb(176, 78, 44), rgb(206, 104, 60), noise3(x * 2, y * 2, z * 2));
  if (y > 13.2 || (abs(x) < 2.0 && abs(z + 1.95) < 0.1 && y < 12.1)) return rgb(120, 86, 54);{STONE}
  return stone;
}}
""")


if __name__ == "__main__":
    wall()
    gate()
    tower()
