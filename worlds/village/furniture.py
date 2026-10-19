"""The things inside: furniture, a shop's fittings and goods, things to pick
up, and things to ride. Each a small type, cheap to draw."""
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
ALL = []


def t(slug, desc, name, bounds, tags, props, sdf, color, anchors=None, extra=None):
    meta = [f'  name: "{name}",', f"  bounds: [{bounds[0]}, {bounds[1]}, {bounds[2]}],", f"  tags: {tags},", f"  props: {props},"]
    if anchors:
        meta.append("  anchors: [\n    " + ",\n    ".join(anchors) + ",\n  ],")
    if extra:
        meta.append(extra)
    src = "// " + desc.replace("\n", "\n// ") + "\nexport const meta = {\n" + "\n".join(meta) + "\n};\n\nexport function sdf(x, y, z, k) {\n" + sdf.strip("\n") + "\n}\n\nexport function color(x, y, z, k) {\n" + color.strip("\n") + "\n}\n"
    slug = "-".join(w for w in re.split(r"[^a-z0-9]+", name.lower()) if w)
    with open(os.path.join(HERE, "types", slug + ".js"), "w") as f:
        f.write(src)
    ALL.append(slug)


WOOD = "mix(rgb(120, 82, 52), rgb(150, 108, 68), noise3(x * 6, y * 18, z * 6))"
DARK = "mix(rgb(84, 56, 38), rgb(104, 72, 48), noise3(x * 6, y * 18, z * 6))"

# ------------------------------------------------------------- the home
t("double-bed", "A double bed: a wooden frame and headboard, a thick mattress under a white\nsheet and a coloured bedspread, two pillows. The headboard is at -z.",
  "double bed", (0.85, 1.15, 1.08), '["furniture", "bed"]', "{ mass: 60, burns: 0.6 }",
  """  const frame = box(x, y - 0.2, z, 0.82, 0.2, 1.02);
  const mattress = roundBox(x, y - 0.46, z + 0.04, 0.78, 0.1, 0.96, 0.05);
  const head = box(x, y - 0.55, z + 1.03, 0.82, 0.55, 0.04);
  const pillows = roundBox(abs(x) - 0.38, y - 0.6, z + 0.75, 0.3, 0.06, 0.16, 0.05);
  return min(min(frame, mattress), min(head, pillows));""",
  f"""  if (y < 0.36 || z < -1.0) return {WOOD};
  if (y > 0.54 && z < -0.55) return rgb(244, 242, 236);
  const c = floor(hash(k.seed) * 4);
  const bed = c < 1 ? rgb(170, 40, 50) : c < 2 ? rgb(40, 90, 150) : c < 3 ? rgb(220, 180, 60) : rgb(60, 130, 90);
  return fract((x + z) * 3.0) < 0.5 ? bed : bed * 0.85;""",
  ['{ kind: "bed", at: [-0.35, 0.56, 0] }', '{ kind: "bed", at: [0.35, 0.56, 0] }'])

t("single-bed", "A narrow single bed: a painted iron frame, a mattress with a patterned blanket\nand one pillow. The headboard is at -z.",
  "single bed", (0.5, 0.75, 1.02), '["furniture", "bed"]', "{ mass: 30, burns: 0.5 }",
  """  const frame = max(box(x, y - 0.35, z, 0.48, 0.35, 1.0), -box(x, y - 0.42, z, 0.44, 0.33, 0.96));
  const mattress = roundBox(x, y - 0.42, z, 0.44, 0.08, 0.96, 0.04);
  const head = box(x, y - 0.6, z + 0.98, 0.48, 0.6, 0.025);
  const pillow = roundBox(x, y - 0.54, z + 0.72, 0.34, 0.06, 0.16, 0.05);
  return min(min(frame, mattress), min(head, pillow));""",
  """  if (y > 0.48 && z < -0.55) return rgb(244, 242, 236);
  if (abs(x) > 0.44 || z < -0.97 || y < 0.33) return rgb(60, 84, 110);
  return step(0.5, fract(x * 4.0)) + step(0.5, fract(z * 4.0)) == 1 ? rgb(220, 120, 60) : rgb(240, 220, 170);""",
  ['{ kind: "bed", at: [0, 0.5, 0] }'])

t("bunk-beds", "Bunk beds for the children of the house: two beds one over the other in a\nwooden frame, a ladder at the foot, blankets in bright colours.",
  "bunk beds", (0.53, 1.75, 1.08), '["furniture", "bed"]', "{ mass: 60, burns: 0.6 }",
  """  const posts = box(abs(x) - 0.47, y - 0.85, abs(z) - 0.97, 0.04, 0.85, 0.04);
  const low = roundBox(x, y - 0.38, z, 0.44, 0.09, 0.96, 0.04);
  const high = roundBox(x, y - 1.32, z, 0.44, 0.09, 0.96, 0.04);
  const rails = min(box(x, y - 0.27, z, 0.48, 0.04, 1.0), box(x, y - 1.2, z, 0.48, 0.04, 1.0));
  const guard = box(x - 0.46, y - 1.52, z + 0.25, 0.02, 0.1, 0.7);
  const rungs = max(box(x + 0.46, y - 0.3 * floor(y / 0.3) - 0.15, z - 0.95, 0.02, 0.02, 0.2), max(y - 1.3, -y));
  return min(min(posts, min(low, high)), min(rails, min(guard, rungs)));""",
  f"""  if (abs(abs(x) - 0.47) < 0.06 || abs(y - 0.27) < 0.05 || abs(y - 1.2) < 0.05 || abs(z - 0.95) < 0.05) return {WOOD};
  return y > 1.0 ? rgb(60, 120, 200) : rgb(230, 80, 70);""",
  ['{ kind: "bed", at: [0, 0.48, 0] }', '{ kind: "bed", at: [0, 1.42, 0] }'])

t("wardrobe", "A tall wardrobe of dark walnut, two doors with brass handles, carved crest.",
  "wardrobe", (0.62, 2.05, 0.32), '["furniture"]', "{ mass: 90, burns: 0.6 }",
  """  return min(box(x, y - 0.98, z, 0.6, 0.98, 0.3), box(x, y - 2.0, z, 0.62, 0.04, 0.32));""",
  f"""  if (z > 0.29 && abs(abs(x) - 0.06) < 0.025 && abs(y - 1.0) < 0.12) return rgb(200, 160, 60);
  if (z > 0.29 && abs(x) < 0.012) return rgb(40, 28, 20);
  return {DARK};""")

t("kitchen", "A home's kitchen along a wall: a counter of white tiles with a sink and a\ngas stove with a pot of sauce on it, a shelf of jars and copper pans above,\nstrings of garlic and peppers. The cook stands at its front (+z).",
  "kitchen", (1.25, 1.95, 0.32), '["furniture", "kitchen"]', "{ mass: 200, burns: 0.3, heat: 60 }",
  """  const counter = box(x, y - 0.45, z, 1.22, 0.45, 0.3);
  const sink = box(x + 0.6, y - 0.86, z, 0.28, 0.1, 0.2);
  const pot = cylinder(x - 0.55, y - 1.0, z + 0.02, 0.14, 0.11);
  const shelf = box(x, y - 1.55, z + 0.12, 1.15, 0.025, 0.16);
  const jars = max(cylinder(x - 0.3 * floor(x / 0.3 + 0.5), y - 1.66, z + 0.12, 0.06, 0.09), abs(x) - 1.1);
  const pans = sphere(x + 0.3 * floor(-x / 0.3 + 0.5) * 0.0 - 0.2, y - 1.3, z + 0.25, 0.1);
  return min(min(max(counter, -sink), pot), min(shelf, min(jars, pans)));""",
  """  if (y > 1.5) return y > 1.6 ? hsv(fract(floor(x / 0.3) * 0.27), 0.5, 0.8) : rgb(130, 90, 56);
  if (y > 0.89 && abs(x - 0.6) < 0.3) return rgb(190, 196, 200);
  if (y > 0.89 && abs(x + 0.55) < 0.16) return y > 1.08 ? rgb(190, 40, 30) : rgb(70, 70, 76);
  if (y > 1.1) return rgb(196, 120, 70);
  if (z > 0.28 && y < 0.82) return fract(x * 1.6) < 0.04 ? rgb(60, 60, 60) : rgb(70, 140, 170);
  if (y > 0.86) return (fract(x * 6.0) < 0.06 || fract(z * 6.0) < 0.06) ? rgb(180, 180, 180) : rgb(240, 238, 232);
  return rgb(240, 238, 232);""",
  ['{ kind: "light", at: [0, 1.8, 0.2] }'])

t("dining-table", "The family table: a long wooden table under a red-and-white checked cloth,\nfour chairs round it, laid with plates of pasta, a bowl of salad, a loaf\nand a jug of wine. Four can sit and eat.",
  "dining table", (1.0, 1.0, 0.95), '["furniture", "table"]', "{ mass: 50, burns: 0.6 }",
  """  const top = box(x, y - 0.74, z, 0.75, 0.025, 0.45);
  const legs = box(abs(x) - 0.68, y - 0.36, abs(z) - 0.38, 0.03, 0.36, 0.03);
  const plates = max(cylinder(abs(x) - 0.4, y - 0.78, abs(z) - 0.2, 0.13, 0.015), -0.0 + 0.0);
  const food = ellipsoid(abs(x) - 0.4, y - 0.8, abs(z) - 0.2, 0.08, 0.035, 0.08);
  const jug = cappedCone(x, y - 0.88, z, 0.12, 0.08, 0.05);
  const bread = ellipsoid(x - 0.0, y - 0.8, z - 0.22, 0.14, 0.05, 0.06);
  const cx = abs(x) - 0.38;
  const seats = box(cx, y - 0.45, abs(z) - 0.68, 0.2, 0.025, 0.19);
  const backs = box(cx, y - 0.72, abs(z) - 0.86, 0.2, 0.25, 0.02);
  const clegs = box(abs(cx) - 0.17, y - 0.22, abs(abs(z) - 0.68) - 0.16, 0.02, 0.22, 0.02);
  return min(min(min(top, legs), min(plates, food)), min(min(jug, bread), min(seats, min(backs, clegs))));""",
  f"""  if (y > 0.84 && length2(x, z) < 0.13) return rgb(150, 40, 50);
  if (y > 0.76 && abs(z - 0.22) < 0.07 && abs(x) < 0.15) return rgb(200, 150, 80);
  if (y > 0.79 && abs(abs(z) - 0.2) < 0.1 && abs(abs(x) - 0.4) < 0.1) return noise3(x * 20, y * 20, z * 20) > 0.6 ? rgb(200, 50, 30) : rgb(240, 200, 110);
  if (y > 0.762 && abs(abs(z) - 0.2) < 0.14 && abs(abs(x) - 0.4) < 0.14) return rgb(246, 246, 240);
  if (abs(y - 0.74) < 0.03 && abs(z) < 0.46) return step(0.5, fract(x * 5.0)) + step(0.5, fract(z * 5.0)) == 1 ? rgb(200, 40, 40) : rgb(246, 242, 236);
  return {WOOD};""",
  ['{ kind: "seat", at: [-0.38, 0.47, 0.66], face: 180 }', '{ kind: "seat", at: [0.38, 0.47, 0.66], face: 180 }',
   '{ kind: "seat", at: [-0.38, 0.47, -0.66], face: 0 }', '{ kind: "seat", at: [0.38, 0.47, -0.66], face: 0 }'])

t("sofa", "A soft three-seat sofa in faded velvet (green, rose, mustard or blue) with\nround arms and cushions, a crocheted cover over its back. Faces +z.",
  "sofa", (1.05, 0.85, 0.46), '["furniture", "seat"]', "{ mass: 60, burns: 0.8 }",
  """  const seat = roundBox(x, y - 0.25, z + 0.02, 0.95, 0.15, 0.4, 0.08);
  const back = roundBox(x, y - 0.5, z + 0.34, 0.95, 0.28, 0.1, 0.08);
  const arms = roundBox(abs(x) - 0.92, y - 0.32, z + 0.02, 0.1, 0.18, 0.42, 0.08);
  return min(min(seat, back), arms);""",
  """  const c = floor(hash(k.seed) * 4);
  const cloth = c < 1 ? rgb(70, 120, 80) : c < 2 ? rgb(190, 110, 120) : c < 3 ? rgb(200, 160, 60) : rgb(60, 90, 150);
  if (y > 0.72 && abs(x) < 0.5) return rgb(244, 238, 222);
  return cloth * (0.9 + 0.1 * noise3(x * 10, y * 10, z * 10));""",
  ['{ kind: "seat", at: [-0.6, 0.4, 0.05], face: 0 }', '{ kind: "seat", at: [0, 0.4, 0.05], face: 0 }', '{ kind: "seat", at: [0.6, 0.4, 0.05], face: 0 }'])

t("armchair", "An old armchair with a lace cloth on its back and a cushion, a reading\nlamp's spot beside it. Faces +z.",
  "armchair", (0.46, 0.9, 0.46), '["furniture", "seat"]', "{ mass: 25, burns: 0.8 }",
  """  return min(min(roundBox(x, y - 0.25, z, 0.38, 0.15, 0.38, 0.07), roundBox(x, y - 0.55, z + 0.32, 0.38, 0.3, 0.08, 0.07)), roundBox(abs(x) - 0.36, y - 0.35, z, 0.08, 0.15, 0.38, 0.06));""",
  """  if (y > 0.75 && abs(x) < 0.2) return rgb(246, 242, 232);
  return hsv(fract(hash(k.seed) * 0.3 + 0.02), 0.45, 0.6) * (0.9 + 0.1 * noise3(x * 9, y * 9, z * 9));""",
  ['{ kind: "seat", at: [0, 0.4, 0.02], face: 0 }'])

t("bookshelf", "A tall bookshelf of five shelves crammed with books in every colour, a few\nfigurines and a framed photograph among them.",
  "bookshelf", (0.6, 2.0, 0.18), '["furniture"]', "{ mass: 60, burns: 0.8 }",
  """  const outer = box(x, y - 0.98, z, 0.58, 0.98, 0.16);
  const holes = box(x, y - 0.4 * floor(y / 0.4) - 0.21, z - 0.03, 0.54, 0.17, 0.15);
  const books = box(x, y - 0.4 * floor(y / 0.4) - 0.17, z, 0.53, 0.13 + 0.03 * hash(floor(x * 25.0), floor(y / 0.4)), 0.12);
  return min(max(outer, -max(holes, y - 1.9)), max(books, max(y - 1.9, 0.05 - y)));""",
  f"""  if (abs(z) < 0.13 && abs(x) < 0.54 && y < 1.9 && fract(y / 0.4) > 0.06) return hsv(hash(floor(x * 25.0), floor(y / 0.4)), 0.55, 0.45 + 0.3 * hash(floor(y / 0.4), floor(x * 25.0)));
  return {DARK};""")

t("bathroom", "A small bathroom against a wall: a white toilet, a washbasin under a round\nmirror and a towel on a ring, blue tiles behind.",
  "bathroom", (0.85, 1.6, 0.4), '["furniture", "bathroom"]', "{ mass: 120, wet: 0.2 }",
  """  const bowl = roundBox(x + 0.4, y - 0.22, z - 0.05, 0.18, 0.22, 0.24, 0.08);
  const tank = box(x + 0.4, y - 0.62, z + 0.28, 0.2, 0.2, 0.08);
  const basin = roundBox(x - 0.35, y - 0.8, z - 0.05, 0.26, 0.07, 0.2, 0.05);
  const pedestal = cylinder(x - 0.35, y - 0.4, z + 0.05, 0.08, 0.4);
  const mirror = cylinder(x + 0.35, z - 0.36, y - 1.35, 0.25, 0.015);
  const tiles = box(x, y - 0.75, z + 0.38, 0.85, 0.75, 0.02);
  return min(min(bowl, tank), min(min(basin, pedestal), min(box(x + 0.35, y - 1.35, z - 0.36, 0.25, 0.25, 0.015) * 0.0 + length2(x + 0.35, y - 1.35) * 0.0 + max(length2(x + 0.35, y - 1.35) - 0.25, abs(z - 0.355) - 0.015), tiles)));""",
  """  if (z > 0.34 && length2(x + 0.35, y - 1.35) < 0.25) return glow(rgb(200, 220, 230), 0.15);
  if (z > 0.35) return (fract(x * 6.0) < 0.06 || fract(y * 6.0) < 0.06) ? rgb(220, 230, 236) : rgb(80, 140, 190);
  return rgb(246, 246, 244);""")

t("bathtub", "A white bathtub on clawed iron feet, a shower hose and a striped curtain.",
  "bathtub", (0.45, 1.95, 0.85), '["furniture", "bathroom"]', "{ mass: 120, wet: 0.3 }",
  """  const tub = max(roundBox(x, y - 0.35, z, 0.38, 0.2, 0.8, 0.12), -roundBox(x, y - 0.5, z, 0.32, 0.2, 0.74, 0.1));
  const feet = sphere(abs(x) - 0.3, y - 0.07, abs(z) - 0.68, 0.07);
  const pole = capsule(x + 0.42, y, z, 0, 1.9, -0.8, 0, 1.9, 0.8, 0.015);
  const curtain = max(box(x + 0.42, y - 1.3, z + 0.55, 0.01, 0.6, 0.25), -0.0 + 0.0);
  return min(min(tub, feet), min(pole, curtain));""",
  """  if (x < -0.38 && y > 0.6) return fract(z * 5.0) < 0.5 ? rgb(90, 160, 200) : rgb(246, 246, 246);
  if (y < 0.15) return rgb(60, 60, 64);
  return rgb(248, 248, 246);""")

t("desk", "A writing desk with a drawer, a chair pushed under, a green-shaded lamp, an\nopen ledger and a pen. Someone sits at its front (+z).",
  "desk", (0.7, 1.2, 0.72), '["furniture", "table"]', "{ mass: 40, burns: 0.6, light: 1 }",
  """  const top = box(x, y - 0.74, z, 0.65, 0.03, 0.32);
  const sides = box(abs(x) - 0.6, y - 0.36, z, 0.04, 0.36, 0.3);
  const lamp = min(cylinder(x + 0.45, y - 0.9, z + 0.1, 0.02, 0.15), cappedCone(x + 0.45, y - 1.08, z + 0.1, 0.06, 0.12, 0.04));
  const book = box(x, y - 0.78, z - 0.05, 0.2, 0.012, 0.14);
  const seat = box(x, y - 0.45, z - 0.5, 0.2, 0.025, 0.2);
  const back = box(x, y - 0.7, z - 0.68, 0.2, 0.22, 0.02);
  return min(min(top, sides), min(min(lamp, book), min(seat, back)));""",
  f"""  if (y > 1.0 && abs(x + 0.45) < 0.13) return glow(rgb(60, 140, 80), 0.4);
  if (y > 0.77 && abs(x) < 0.2) return rgb(244, 238, 220);
  return {WOOD};""",
  ['{ kind: "seat", at: [0, 0.47, -0.5], face: 0 }', '{ kind: "light", at: [-0.45, 1.0, 0.1] }'])

t("chest-of-drawers", "A chest of drawers with brass pulls, on it a lace runner, framed family\nphotographs and a vase of flowers.",
  "chest of drawers", (0.6, 1.25, 0.26), '["furniture"]', "{ mass: 50, burns: 0.6 }",
  """  const body = box(x, y - 0.42, z, 0.56, 0.42, 0.24);
  const frames = box(abs(x) - 0.3, y - 0.98 + 0.0, z + 0.05, 0.08, 0.1, 0.01);
  const vase = min(cappedCone(x, y - 0.95, z, 0.1, 0.06, 0.04), sphere(x, y - 1.1, z, 0.1));
  return min(body, min(max(frames, -0.0 + 0.0), vase));""",
  f"""  if (y > 1.02 && abs(x) < 0.11) return hsv(fract(hash(k.seed) + 0.9), 0.7, 0.9);
  if (y > 0.86 && abs(x) < 0.1) return rgb(60, 110, 160);
  if (y > 0.86) return rgb(210, 190, 150);
  if (z > 0.22 && abs(fract(y / 0.27) - 0.5) < 0.08 && abs(abs(x) - 0.25) < 0.05) return rgb(200, 160, 60);
  return {DARK};""")

t("floor-lamp", "A standing lamp with a fringed cream shade, warm light at night.",
  "floor lamp", (0.25, 1.7, 0.25), '["furniture", "lamp"]', "{ mass: 6, light: 1, fragile: 0.4 }",
  """  return min(min(cylinder(x, y - 0.03, z, 0.18, 0.03), cylinder(x, y - 0.8, z, 0.02, 0.8)), cappedCone(x, y - 1.5, z, 0.16, 0.24, 0.14));""",
  """  if (y > 1.33) return glow(rgb(255, 226, 170), 0.8);
  return rgb(60, 50, 40);""",
  ['{ kind: "light", at: [0, 1.5, 0] }'])

# ------------------------------------------------------------- the shop
t("shop-counter", "A shop counter of varnished wood with a marble top, an old brass till and a\nset of scales on it. The shopkeeper stands behind it (-z); customers at its\nfront (+z).",
  "shop counter", (1.2, 1.2, 0.35), '["furniture", "counter", "shop"]', "{ mass: 120, burns: 0.4 }",
  """  const body = box(x, y - 0.48, z, 1.15, 0.48, 0.3);
  const top = box(x, y - 0.98, z, 1.2, 0.025, 0.34);
  const till = box(x + 0.75, y - 1.12, z + 0.05, 0.18, 0.12, 0.15);
  const scales = min(cylinder(x - 0.6, y - 1.06, z, 0.12, 0.05), cylinder(x - 0.6, y - 1.18, z, 0.03, 0.08));
  return min(min(body, top), min(till, scales));""",
  f"""  if (y > 1.0 && abs(x - 0.75) < 0.2) return rgb(196, 160, 60);
  if (y > 1.0) return rgb(190, 196, 200);
  if (y > 0.95) return rgb(236, 232, 226);
  return {WOOD};""")

def shelves(slug, name, desc, goods):
    t(slug, desc, name, (1.0, 2.15, 0.25), '["furniture", "shop"]', "{ mass: 80, burns: 0.5 }",
      """  const frame = max(box(x, y - 1.05, z, 0.95, 1.05, 0.22), -box(x, y - 1.12, z - 0.04, 0.91, 0.98, 0.22));
  const sy = y - 0.5 * floor(y / 0.5) - 0.25;
  const boards = max(box(x, sy + 0.23, z, 0.92, 0.02, 0.2), max(y - 2.05, 0.1 - y));
  const gx = x - 0.2 * floor(x / 0.2 + 0.5);
  const goods = max(max(box(gx, sy + 0.08, z, 0.08, 0.13, 0.15), abs(x) - 0.88), max(y - 2.0, 0.2 - y));
  return min(frame, min(boards, goods));""",
      f"""  const sy = y - 0.5 * floor(y / 0.5);
  if (abs(z) < 0.19 && abs(x) < 0.9 && sy > 0.05 && y > 0.2 && y < 2.0) {{
    const i = floor(x / 0.2 + 0.5) + floor(y / 0.5) * 7;
    const h = hash(i, k.seed);
{goods}
  }}
  return {WOOD};""")

shelves("bread-shelves", "bread shelves", "The baker's shelves behind the counter, stacked with round loaves, long\nbaguettes, focaccia and twisted grissini.",
        """    return mix(rgb(196, 130, 60), rgb(236, 190, 110), h) * (0.85 + 0.15 * noise3(x * 20, y * 20, z * 20));""")
shelves("pharmacy-shelves", "pharmacy shelves", "The pharmacy's shelves: rows of white and brown bottles, boxes of medicine\nwith green crosses, jars of herbs and powders with neat labels.",
        """    if (h < 0.3) return rgb(120, 70, 30);
    if (h < 0.6) return rgb(246, 246, 244);
    return fract(y * 8.0) < 0.3 ? rgb(40, 140, 80) : rgb(236, 236, 230);""")
shelves("grocery-shelves", "grocery shelves", "The grocer's shelves: tins of tomatoes, bags of pasta, bottles of olive oil,\njars of olives and capers, packets of coffee.",
        """    if (h < 0.25) return rgb(200, 40, 30);
    if (h < 0.5) return rgb(236, 200, 110);
    if (h < 0.75) return rgb(110, 130, 40);
    return rgb(40, 60, 120);""")
shelves("wine-shelves", "wine shelves", "Shelves of wine and liqueur bottles behind the bar: dark reds, pale whites,\ngreen bottles of limoncello and amaro.",
        """    if (h < 0.4) return rgb(70, 20, 30);
    if (h < 0.65) return rgb(230, 220, 120);
    return rgb(40, 100, 50);""")
shelves("book-shelves", "book shelves", "The bookseller's shelves, full of books new and old, maps of the coast and\npostcards of the harbour.",
        """    return hsv(h, 0.55, 0.5 + 0.3 * hash(k.seed, i));""")
shelves("cloth-shelves", "cloth shelves", "The tailor's shelves, bolts of cloth in every colour, linen, wool, silk.",
        """    return hsv(h, 0.6, 0.75);""")

t("gelato-counter", "The gelato counter: a glass case of tubs heaped with ice cream in pistachio,\nstrawberry, lemon, chocolate, stracciatella and more, cones stacked on top.",
  "gelato counter", (1.1, 1.3, 0.4), '["furniture", "counter", "shop"]', "{ mass: 150, light: 1 }",
  """  const body = box(x, y - 0.45, z, 1.05, 0.45, 0.36);
  const glass = max(box(x, y - 1.0, z - 0.05, 1.05, 0.1, 0.3), (z - 0.1) + (y - 0.9) * 0.6);
  const tubs = max(ellipsoid(x - 0.3 * floor(x / 0.3 + 0.5), y - 0.95, z - 0.1 * 0.0, 0.13, 0.06, 0.25), abs(x) - 1.0);
  const cones = max(cappedCone(x - 0.8, y - 1.18, z + 0.2, 0.1, 0.02, 0.05), 0.0 - 0.0 - 1.0);
  return min(min(body, tubs), min(glass * 1.0, cones));""",
  """  if (y > 0.86 && y < 1.03 && abs(z) < 0.3) return hsv(fract(floor(x / 0.3 + 0.5) * 0.17 + 0.05), 0.45, 0.95);
  if (y > 1.05) return rgb(210, 160, 90);
  if (y > 0.9) return glow(rgb(220, 236, 240), 0.2);
  return rgb(240, 236, 228);""")

t("butcher-counter", "The butcher's counter: a white-tiled counter with a glass front full of\nsteaks, sausages and chops, a wooden block and cleaver on top, hams and\nsalami hanging from a rail above.",
  "butcher counter", (1.15, 2.1, 0.4), '["furniture", "counter", "shop"]', "{ mass: 160 }",
  """  const body = box(x, y - 0.45, z, 1.1, 0.45, 0.36);
  const block = box(x + 0.6, y - 1.0, z, 0.3, 0.08, 0.25);
  const meats = max(box(x - 0.2 * floor(x / 0.2 + 0.5), y - 0.94, z, 0.08, 0.04, 0.24), abs(x) - 0.4);
  const rail = capsule(x, y, z, -1.0, 2.0, -0.2, 1.0, 2.0, -0.2, 0.02);
  const hams = max(ellipsoid(x - 0.4 * floor(x / 0.4 + 0.5), y - 1.65, z + 0.2, 0.1, 0.22, 0.1), abs(x) - 1.0);
  return min(min(body, block), min(meats, min(rail, hams)));""",
  """  if (y > 1.4) return y > 1.95 ? rgb(120, 120, 120) : mix(rgb(150, 60, 40), rgb(190, 110, 80), noise3(x * 10, y * 10, z * 10));
  if (y > 0.98 && x > 0.3) return rgb(160, 120, 80);
  if (y > 0.89) return mix(rgb(170, 40, 40), rgb(220, 120, 110), noise3(x * 20, y * 20, z * 20));
  return (fract(x * 6.0) < 0.05 || fract(y * 6.0) < 0.05) ? rgb(200, 200, 200) : rgb(246, 246, 242);""")

t("fish-counter", "The fish counter of the pescheria: a sloping bed of crushed ice on a tiled\nstand, laid with silver sardines, red mullet, a sea bass, octopus and mussels,\nlemons cut among them.",
  "fish counter", (1.15, 1.0, 0.45), '["furniture", "counter", "shop"]', "{ mass: 140, wet: 0.5 }",
  """  const stand = box(x, y - 0.4, z, 1.1, 0.4, 0.4);
  const ice = max(box(x, y - 0.85, z, 1.1, 0.1, 0.4), (y - 0.9) - (z + 0.4) * 0.25);
  const fish = max(ellipsoid(x - 0.25 * floor(x / 0.25 + 0.5), y - 0.88 - (z + 0.4) * 0.25 * 0.0, z, 0.1, 0.04, 0.28), abs(x) - 1.0);
  return min(stand, min(ice, fish));""",
  """  if (y > 0.82) {
    const i = floor(x / 0.25 + 0.5);
    const h = hash(i, k.seed);
    if (noise3(x * 30, y * 30, z * 30) > 0.62) return rgb(250, 220, 40);
    if (abs(x - i * 0.25) < 0.1) return h < 0.4 ? rgb(196, 206, 214) : h < 0.7 ? rgb(214, 90, 70) : rgb(120, 70, 90);
    return rgb(236, 244, 248);
  }
  return (fract(x * 6.0) < 0.05 || fract(y * 6.0) < 0.05) ? rgb(200, 210, 210) : rgb(70, 140, 170);""")

t("bar-counter", "A bar's counter: polished wood with a zinc top, a chrome espresso machine\nhissing at one end, cups stacked on it, a bowl of oranges and pastries\nunder a glass dome.",
  "bar counter", (1.6, 1.4, 0.4), '["furniture", "counter", "shop"]', "{ mass: 200, heat: 30 }",
  """  const body = box(x, y - 0.52, z, 1.55, 0.52, 0.32);
  const top = box(x, y - 1.06, z, 1.6, 0.025, 0.38);
  const machine = roundBox(x + 1.05, y - 1.3, z + 0.05, 0.35, 0.22, 0.22, 0.05);
  const cups = max(cylinder(x - 0.12 * floor(x / 0.12 + 0.5), y - 1.12, z, 0.04, 0.04), abs(x + 0.2) - 0.35);
  const dome = max(sphere(x + 0.5, y - 1.1, z, 0.2), 1.08 - y);
  return min(min(body, top), min(machine, min(cups, dome)));""",
  f"""  if (abs(x - 1.05) < 0.4 && y > 1.08) return rgb(210, 214, 220);
  if (y > 1.08 && abs(x + 0.5) < 0.22) return glow(rgb(240, 200, 120), 0.2);
  if (y > 1.08) return rgb(246, 246, 240);
  if (y > 1.03) return rgb(180, 184, 186);
  return {WOOD};""")

t("wine-barrels", "Three oak wine barrels on their sides on a rack, each with a spigot, the\nosteria's house red and white.",
  "wine barrels", (1.3, 1.2, 0.5), '["furniture", "prop"]', "{ mass: 300, burns: 0.5, fuel: 0.4 }",
  """  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  const barrels = max(max(length2(bx, y - 0.62) - 0.38 - 0.05 * (1.0 - abs(z) / 0.45), abs(z) - 0.45), abs(x) - 1.25);
  const rack = box(x, y - 0.12, z, 1.25, 0.12, 0.4);
  return min(barrels, rack);""",
  f"""  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  if (abs(abs(z) - 0.32) < 0.03 || abs(z) > 0.43) return rgb(60, 60, 62);
  return {WOOD};""")

t("trattoria-table", "A trattoria table under a checked cloth, four rush chairs round it, laid\nwith a pizza on a board, glasses, a carafe of wine and a basket of bread.",
  "trattoria table", (0.95, 1.0, 0.95), '["furniture", "table"]', "{ mass: 40, burns: 0.6 }",
  """  const top = box(x, y - 0.74, z, 0.45, 0.025, 0.45);
  const leg = cylinder(x, y - 0.37, z, 0.05, 0.37);
  const pizza = cylinder(x, y - 0.775, z, 0.25, 0.012);
  const carafe = cappedCone(x + 0.3, y - 0.86, z - 0.3, 0.1, 0.07, 0.03);
  const ax = abs(x) - 0.68;
  const az = abs(z) - 0.68;
  const seats = min(box(ax, y - 0.45, z, 0.18, 0.025, 0.18), box(x, y - 0.45, az, 0.18, 0.025, 0.18));
  const backs = min(box(abs(x) - 0.86, y - 0.7, z, 0.02, 0.24, 0.18), box(x, y - 0.7, abs(z) - 0.86, 0.18, 0.24, 0.02));
  return min(min(top, leg), min(min(pizza, carafe), min(seats, backs)));""",
  f"""  if (y > 0.76 && length2(x, z) < 0.25) return noise3(x * 25, y * 25, z * 25) > 0.62 ? rgb(190, 40, 30) : rgb(240, 200, 120);
  if (y > 0.78 && length2(x - 0.3, z + 0.3) < 0.1) return rgb(130, 30, 40);
  if (abs(y - 0.74) < 0.03 && abs(x) < 0.46 && abs(z) < 0.46) return step(0.5, fract(x * 5.0)) + step(0.5, fract(z * 5.0)) == 1 ? rgb(200, 40, 40) : rgb(246, 242, 236);
  return {WOOD};""",
  ['{ kind: "seat", at: [0.68, 0.47, 0], face: 270 }', '{ kind: "seat", at: [-0.68, 0.47, 0], face: 90 }',
   '{ kind: "seat", at: [0, 0.47, 0.68], face: 180 }', '{ kind: "seat", at: [0, 0.47, -0.68], face: 0 }'])

t("barber-chair", "The barber's chair: red leather on chrome, a footrest, before a tall\nmirror with a shelf of combs, scissors and bottles of tonic. Faces +z, its\nback to the mirror at -z.",
  "barber chair", (0.7, 1.95, 0.6), '["furniture", "seat", "shop"]', "{ mass: 80 }",
  """  const base = cylinder(x, y - 0.15, z, 0.25, 0.15);
  const post = cylinder(x, y - 0.35, z, 0.06, 0.2);
  const seat = roundBox(x, y - 0.6, z + 0.05, 0.3, 0.08, 0.3, 0.05);
  const back = roundBox(x, y - 0.95, z - 0.25, 0.3, 0.32, 0.06, 0.05);
  const mirror = box(x, y - 1.4, z + 0.57, 0.65, 0.5, 0.02);
  const shelf = box(x, y - 0.85, z + 0.48, 0.65, 0.025, 0.1);
  return min(min(base, post), min(min(seat, back), min(mirror, shelf)));""",
  """  if (z < -0.5 && y > 0.88) return glow(rgb(200, 220, 230), 0.15);
  if (y < 0.5 || z < -0.4) return rgb(200, 204, 210);
  return rgb(170, 30, 36);""",
  ['{ kind: "seat", at: [0, 0.7, 0.05], face: 0 }'])

t("vet-table", "The vet's examining table: a steel top on a white stand, a cabinet of\nmedicines and bandages beside it, a weighing scale for dogs.",
  "vet table", (1.2, 1.0, 0.45), '["furniture", "shop"]', "{ mass: 90 }",
  """  const top = box(x, y - 0.85, z, 0.6, 0.03, 0.35);
  const stand = box(x, y - 0.42, z, 0.5, 0.42, 0.3);
  const cab = box(x - 0.95, y - 0.5, z, 0.25, 0.5, 0.25);
  return min(min(top, stand), cab);""",
  """  if (y > 0.8 && abs(x) < 0.6) return rgb(196, 200, 206);
  if (x < -0.7) return fract(y * 4.0) < 0.06 ? rgb(60, 60, 60) : rgb(236, 240, 240);
  return rgb(246, 246, 246);""")

t("reception-desk", "The hotel's reception desk: dark wood with a brass bell and a guest book on\nit, a board of room keys on hooks behind. The clerk stands behind it (-z).",
  "reception desk", (1.25, 2.0, 0.6), '["furniture", "counter"]', "{ mass: 150, burns: 0.4 }",
  """  const desk = box(x, y - 0.55, z, 1.2, 0.55, 0.3);
  const top = box(x, y - 1.11, z, 1.25, 0.025, 0.35);
  const bell = min(sphere(x - 0.6, y - 1.18, z, 0.06), cylinder(x - 0.6, y - 1.14, z, 0.07, 0.01));
  const book = box(x + 0.2, y - 1.15, z, 0.2, 0.02, 0.14);
  const keys = box(x, y - 1.6, z + 0.58, 0.6, 0.35, 0.02);
  return min(min(desk, top), min(min(bell, book), keys));""",
  f"""  if (z < -0.5) return fract(x * 6.0) < 0.15 && fract(y * 5.0) < 0.3 ? rgb(200, 160, 60) : rgb(90, 60, 40);
  if (y > 1.12 && x < -0.4) return rgb(210, 170, 70);
  if (y > 1.12) return rgb(140, 30, 40);
  return {DARK};""")

t("school-desk", "A double school desk with an inkwell and a slanted lid, a bench for two\nchildren. They sit at its back (-z), facing +z.",
  "school desk", (0.6, 0.75, 0.6), '["furniture", "table"]', "{ mass: 25, burns: 0.6 }",
  """  const lid = box(x, y - 0.68 - z * 0.15, z, 0.55, 0.02, 0.2);
  const sides = box(abs(x) - 0.52, y - 0.35, z, 0.03, 0.35, 0.22);
  const bench = box(x, y - 0.4, z + 0.45, 0.55, 0.025, 0.12);
  return min(min(lid, sides), bench);""",
  f"""  return {WOOD};""",
  ['{ kind: "seat", at: [-0.25, 0.42, -0.45], face: 0 }', '{ kind: "seat", at: [0.25, 0.42, -0.45], face: 0 }'])

t("blackboard", "A blackboard on an easel stand, chalked with the alphabet, a sum and a\ndrawing of a fish; a box of chalk on its ledge.",
  "blackboard", (0.85, 1.9, 0.4), '["furniture", "prop"]', "{ mass: 20, burns: 0.6 }",
  """  const board = box(x, y - 1.3, z, 0.8, 0.5, 0.025);
  const legs = capsule(abs(x) - 0.7, y, z, 0, 0, -0.35, 0, 1.85, 0, 0.025);
  return min(board, legs);""",
  """  if (abs(z) < 0.03 && abs(x) < 0.76 && abs(y - 1.3) < 0.46) return noise3(x * 30, y * 30, 1.0) > 0.78 ? rgb(230, 230, 220) : rgb(40, 60, 50);
  return rgb(120, 90, 60);""")

t("boat-on-trestles", "A fishing boat half built on trestles in Matteo's workshop: ribs and fresh\nplanks of chestnut, half of it painted blue, a pot of paint and brushes\nbeside it.",
  "boat on trestles", (0.95, 1.5, 2.6), '["furniture", "prop"]', "{ mass: 400, burns: 0.6 }",
  """  const tr = box(abs(x) - 0.0, y - 0.35, abs(z) - 1.6, 0.8, 0.35, 0.08);
  const t2 = clamp(z / 2.5, -1, 1);
  const w = 0.85 * sqrt(max(1 - t2 * t2, 0.02));
  const hull = max(max(length2(x / max(w, 0.05), (y - 1.15) / 0.5) - 1, y - 1.25), abs(z) - 2.5) * min(w, 0.4);
  const inner = max(length2(x / max(w - 0.06, 0.03), (y - 1.2) / 0.44) - 1, abs(z) - 2.4) * 0.4;
  return min(tr, max(hull, -inner));""",
  f"""  if (y < 0.72) return {DARK};
  if (z > 0.0 && y > 0.75) return rgb(40, 100, 180);
  return fract(y * 8.0) < 0.12 ? rgb(110, 76, 48) : rgb(196, 150, 100);""")

t("workbench", "A heavy workbench with a vice, planes, chisels and saws hung on a board\nabove it, wood shavings on the floor.",
  "workbench", (1.0, 1.7, 0.42), '["furniture", "prop"]', "{ mass: 80, burns: 0.6 }",
  """  const top = box(x, y - 0.85, z, 0.95, 0.05, 0.38);
  const legs = box(abs(x) - 0.85, y - 0.4, abs(z) - 0.3, 0.05, 0.4, 0.05);
  const vice = box(x + 0.85, y - 0.95, z + 0.36, 0.08, 0.06, 0.06);
  const board = box(x, y - 1.35, z - 0.4, 0.9, 0.35, 0.02);
  const tools = max(box(x - 0.25 * floor(x / 0.25 + 0.5), y - 1.35, z - 0.37, 0.03, 0.25, 0.01), abs(x) - 0.8);
  return min(min(top, legs), min(vice, min(board, tools)));""",
  f"""  if (z < -0.36 && y > 1.0) return abs(x - 0.25 * floor(x / 0.25 + 0.5)) < 0.035 ? rgb(150, 150, 156) : rgb(196, 170, 120);
  if (abs(x - 0.85) < 0.1 && y > 0.88) return rgb(60, 60, 64);
  return {WOOD};""")

t("sewing-table", "A tailor's table: a black sewing machine with gold lettering, a pincushion,\nscissors, a tape measure, and a dressmaker's dummy in a half-made dress.",
  "sewing table", (0.95, 1.6, 0.4), '["furniture", "prop"]', "{ mass: 50, burns: 0.6 }",
  """  const top = box(x + 0.25, y - 0.75, z, 0.6, 0.03, 0.35);
  const legs = box(abs(x + 0.25) - 0.55, y - 0.37, z, 0.03, 0.37, 0.3);
  const machine = min(box(x + 0.3, y - 0.88, z, 0.25, 0.1, 0.1), box(x + 0.48, y - 1.0, z, 0.07, 0.2, 0.08));
  const dummy = min(ellipsoid(x + 0.7, y - 1.25, z, 0.18, 0.3, 0.13), cylinder(x + 0.7, y - 0.5, z, 0.02, 0.5));
  const dress = max(cappedCone(x + 0.7, y - 0.9, z, 0.32, 0.32, 0.16), 0.6 - y);
  return min(min(top, legs), min(machine, min(dummy, dress)));""",
  f"""  if (x < -0.4) return y < 1.0 ? hsv(fract(hash(k.seed) * 0.8), 0.55, 0.8) : rgb(220, 200, 170);
  if (y > 0.78 && y < 1.2) return rgb(30, 30, 32);
  return {WOOD};""")

t("easel", "An artist's easel holding a half-finished painting of the harbour, a stool\nand a palette of blobbed paints beside it.",
  "easel", (0.45, 1.85, 0.45), '["furniture", "prop"]', "{ mass: 8, burns: 0.8 }",
  """  const legs = min(capsule(x, y, z, -0.3, 0, -0.1, 0, 1.8, 0.05, 0.02), min(capsule(x, y, z, 0.3, 0, -0.1, 0, 1.8, 0.05, 0.02), capsule(x, y, z, 0, 0, 0.4, 0, 1.6, 0.05, 0.02)));
  const canvas = box(x, y - 1.25, z + 0.02, 0.35, 0.28, 0.015);
  const ledge = box(x, y - 0.95, z + 0.05, 0.38, 0.02, 0.05);
  return min(legs, min(canvas, ledge));""",
  """  if (abs(z - 0.005) < 0.03 && abs(x) < 0.34 && abs(y - 1.25) < 0.27) {
    if (y < 1.12) return mix(rgb(40, 110, 180), rgb(90, 160, 210), noise3(x * 8, y * 8, 2.0));
    if (y < 1.3) return hsv(fract(floor(x * 12.0) * 0.13), 0.5, 0.9);
    return rgb(150, 200, 236);
  }
  return rgb(150, 110, 70);""")

t("telescope", "A brass telescope on a wooden tripod, pointed out to sea.",
  "telescope", (0.45, 1.6, 0.55), '["furniture", "prop"]', "{ mass: 8 }",
  """  const legs = min(capsule(x, y, z, -0.35, 0, -0.2, 0, 1.2, 0, 0.02), min(capsule(x, y, z, 0.35, 0, -0.2, 0, 1.2, 0, 0.02), capsule(x, y, z, 0, 0, 0.35, 0, 1.2, 0, 0.02)));
  const tube = cone(x, y, z, 0, 1.2, -0.3, 0, 1.45, 0.5, 0.06, 0.04);
  return min(legs, tube);""",
  """  if (y > 1.18) return rgb(200, 160, 70);
  return rgb(110, 76, 48);""")

t("upright-piano", "An upright piano of dark wood, its lid open on yellowed keys, a candlestick\nand sheet music on it, a stool before it (+z).",
  "upright piano", (0.75, 1.35, 0.6), '["furniture", "instrument"]', "{ mass: 200, burns: 0.5 }",
  """  const body = box(x, y - 0.65, z, 0.72, 0.65, 0.28);
  const keys = box(x, y - 0.72, z - 0.36, 0.65, 0.04, 0.1);
  const stool = cylinder(x, y - 0.25, z - 0.55, 0.16, 0.25);
  return min(min(body, keys), stool);""",
  f"""  if (abs(y - 0.74) < 0.03 && z > 0.26) return fract(x * 22.0) < 0.15 ? rgb(30, 30, 30) : rgb(240, 232, 210);
  return {DARK};""",
  ['{ kind: "seat", at: [0, 0.5, -0.55], face: 180 }'])

# ------------------------------------------------------------- things to pick up
def item(slug, name, desc, bounds, props, sdf, color, tags='["item"]', extra=None):
    t(slug, desc, name, bounds, tags, props, sdf, color, extra=extra)

item("pizza", "pizza", "A wood-fired pizza margherita on a board: blistered crust, tomato, melted\nmozzarella and basil leaves.", (0.2, 0.05, 0.2), "{ mass: 0.6, edible: 0.8, burns: 0.5 }",
     """  return min(cylinder(x, y - 0.02, z, 0.18, 0.02), cylinder(x, y - 0.045, z, 0.16, 0.008));""",
     """  const r = length2(x, z);
  if (r > 0.155) return rgb(214, 160, 90);
  const n = noise3(x * 40, 1.0, z * 40);
  if (n > 0.75) return rgb(50, 130, 50);
  if (n > 0.45) return rgb(246, 236, 210);
  return rgb(196, 40, 28);""", tags='["item", "food"]')
item("panino", "panino", "A panino: a crusty roll split and stuffed with mortadella, cheese and rocket.", (0.13, 0.06, 0.06), "{ mass: 0.25, edible: 0.6, burns: 0.5 }",
     """  return ellipsoid(x, y - 0.04, z, 0.12, 0.045, 0.055);""",
     """  if (abs(y - 0.04) < 0.008) return rgb(230, 150, 150);
  if (abs(y - 0.05) < 0.008) return rgb(70, 140, 50);
  return rgb(210, 150, 80);""", tags='["item", "food"]')
item("gelato", "gelato", "A gelato cone with two scoops, pistachio and strawberry, a little melting.", (0.05, 0.22, 0.05), "{ mass: 0.15, edible: 0.4 }",
     """  return min(cappedCone(x, y - 0.06, z, 0.05, 0.005, 0.03), min(sphere(x, y - 0.13, z, 0.035), sphere(x, y - 0.18, z, 0.03)));""",
     """  if (y < 0.11) return rgb(210, 160, 90);
  return y < 0.155 ? rgb(170, 210, 120) : rgb(250, 170, 180);""", tags='["item", "food"]')
item("wine-bottle", "bottle of wine", "A bottle of the house red, a straw-wrapped fiasco.", (0.08, 0.3, 0.08), "{ mass: 1.2, edible: 0.2, fragile: 0.8 }",
     """  return min(sphere(x, y - 0.08, z, 0.08), cylinder(x, y - 0.2, z, 0.02, 0.08));""",
     """  if (y < 0.11) return fract(y * 30.0) < 0.5 ? rgb(196, 160, 90) : rgb(170, 130, 70);
  return rgb(60, 20, 30);""", tags='["item", "food"]')
item("salami", "salami", "A whole salami in its string net, for slicing.", (0.04, 0.04, 0.17), "{ mass: 0.6, edible: 0.7 }",
     """  return capsule(x, y, z, 0, 0, -0.13, 0, 0, 0.13, 0.04);""",
     """  return fract(z * 20.0) < 0.15 ? rgb(240, 236, 226) : rgb(150, 70, 60);""", tags='["item", "food"]')
item("medicine-bottle", "bottle of medicine", "A brown glass bottle of syrup with a paper label and a cork.", (0.04, 0.08, 0.04), "{ mass: 0.2, fragile: 0.8 }",
     """  return min(cylinder(x, y - 0.05, z, 0.035, 0.05), cylinder(x, y - 0.12, z, 0.012, 0.03));""",
     """  if (abs(y - 0.05) < 0.02) return rgb(240, 236, 220);
  return rgb(120, 70, 30);""")
item("newspaper", "newspaper", "Today's newspaper, folded: the fishing forecast, the football, the feast of San Pietro.", (0.17, 0.02, 0.12), "{ mass: 0.2, burns: 0.95 }",
     """  return box(x, y - 0.01, z, 0.16, 0.01, 0.11);""",
     """  return fract(z * 18.0) < 0.4 ? rgb(70, 70, 70) : rgb(236, 232, 220);""")
item("book", "book", "A cloth-bound book of stories, its corners worn.", (0.12, 0.03, 0.09), "{ mass: 0.5, burns: 0.9 }",
     """  return box(x, y - 0.025, z, 0.11, 0.025, 0.085);""",
     """  if (abs(x) < 0.1 && abs(z) < 0.08 && abs(y - 0.025) < 0.018) return rgb(240, 232, 210);
  return hsv(hash(k.seed), 0.6, 0.5);""")
item("coffee-cup", "cup of espresso", "A small white cup of espresso on its saucer.", (0.06, 0.06, 0.06), "{ mass: 0.15, edible: 0.1, fragile: 0.7, heat: 60 }",
     """  return min(cylinder(x, y - 0.005, z, 0.055, 0.005), cappedCone(x, y - 0.035, z, 0.025, 0.025, 0.032));""",
     """  if (y > 0.058) return rgb(60, 36, 20);
  return rgb(246, 246, 244);""", tags='["item", "food"]')
item("bunch-of-flowers", "bunch of flowers", "A bunch of flowers wrapped in paper: roses, daisies and sprigs of lavender.", (0.1, 0.25, 0.1), "{ mass: 0.3, alive: 1, burns: 0.6 }",
     """  return min(cappedCone(x, y - 0.1, z, 0.1, 0.02, 0.07), sphere(x, y - 0.2, z, 0.09));""",
     """  if (y < 0.18) return rgb(236, 224, 200);
  return hsv(fract(noise3(x * 30, y * 30, z * 30) * 2.0), 0.6, 0.9);""")
item("broom", "broom", "A broom of yellow straw on a long wooden handle, for sweeping the step.", (0.15, 0.7, 0.06), "{ mass: 0.8, burns: 0.8 }",
     """  return min(cylinder(x, y - 0.22, z, 0.014, 0.48), max(cappedCone(x, y + 0.42, z, 0.18, 0.15, 0.04) * 0.0 + box(x, y + 0.5, z, 0.13 - (y + 0.5) * 0.25, 0.18, 0.04), -0.0 - 1.0));""",
     """  if (y < -0.32) return rgb(220, 190, 90);
  return rgb(150, 110, 70);""", tags='["item", "tool"]', extra='  tool: { grip: [0, 0.5, 0], tip: [0, -0.65, 0], motions: ["swing"], sharp: 0 },')
item("bucket", "bucket", "A zinc bucket of water with a wire handle.", (0.15, 0.42, 0.16), "{ mass: 6, wet: 1 }",
     """  return min(max(cappedCone(x, y - 0.13, z, 0.13, 0.11, 0.14), -cappedCone(x, y - 0.15, z, 0.12, 0.1, 0.13)), max(abs(length2(x, y - 0.26) - 0.13) - 0.006, abs(z) - 0.006));""",
     """  if (y > 0.2 && length2(x, z) < 0.13) return rgb(90, 140, 170);
  return rgb(170, 176, 180);""", tags='["item", "tool"]', extra='  tool: { grip: [0, 0.38, 0], tip: [0, 0.2, 0.15], motions: ["pour"], sharp: 0 },')
item("watering-can", "watering can", "A green tin watering can with a long spout, for the geraniums.", (0.12, 0.38, 0.32), "{ mass: 3, wet: 0.8 }",
     """  return min(min(cylinder(x, y - 0.11, z, 0.1, 0.11), cone(x, y, z, 0, 0.05, 0.08, 0, 0.22, 0.28, 0.025, 0.015)), max(abs(length2(z + 0.02, y - 0.2) - 0.08) - 0.01, abs(x) - 0.01));""",
     """  return rgb(60, 130, 80);""", tags='["item", "tool"]', extra='  tool: { grip: [0, 0.28, -0.02], tip: [0, 0.22, 0.28], motions: ["pour"], sharp: 0 },')
item("accordion", "accordion", "An accordion with mother-of-pearl buttons and red bellows.", (0.25, 0.42, 0.12), "{ mass: 6, fragile: 0.4 }",
     """  return min(box(x, y - 0.2, z, 0.08 + 0.0, 0.2, 0.1), min(box(abs(x) - 0.16, y - 0.2, z, 0.06, 0.2, 0.11), box(x, y - 0.2, z, 0.12, 0.19, 0.1)));""",
     """  if (abs(x) < 0.11) return fract(x * 40.0) < 0.5 ? rgb(170, 30, 40) : rgb(240, 230, 230);
  return fract(y * 12.0) < 0.15 ? rgb(240, 236, 230) : rgb(30, 30, 34);""", tags='["item", "instrument"]')

# ------------------------------------------------------------- in the street
t("drinking-fountain", "A nasone: a cast-iron drinking fountain shaped like a little column, its\ncurved spout always running with cool water into a grate.",
  "drinking fountain", (0.3, 1.0, 0.45), '["prop", "fountain"]', "{ mass: 200, wet: 1 }",
  """  const col = cappedCone(x, y - 0.45, z, 0.45, 0.17, 0.14);
  const top = sphere(x, y - 0.92, z, 0.12);
  const spout = cone(x, y, z, 0, 0.6, 0.12, 0, 0.55, 0.3, 0.03, 0.022);
  const water = cylinder(x, y - 0.27, z - 0.31, 0.012, 0.27);
  const grate = box(x, y - 0.01, z - 0.25, 0.22, 0.01, 0.18);
  return min(min(col, top), min(spout, min(water, grate)));""",
  """  if (abs(z - 0.31) < 0.03 && y < 0.54 && y > 0.03) return rgb(200, 230, 246);
  return rgb(44, 56, 50);""",
  extra='  sound: { hard: 1, dry: 0.2, ring: 0.5 },')

t("public-toilets", "The public toilets: a small whitewashed block with two doors, UOMINI and\nDONNE in blue letters, a washbasin outside. Doors face +z.",
  "public toilets", (2.3, 3.0, 1.8), '["building", "bathroom"]', "{ mass: 30000, wet: 0.2 }",
  """  const body = box(x, y - 1.35, z, 2.2, 1.35, 1.6);
  const rooms = min(box(x - 1.1, y - 1.2, z, 0.9, 1.15, 1.35), box(x + 1.1, y - 1.2, z, 0.9, 1.15, 1.35));
  const doors = min(box(x - 1.1, y - 1.05, z - 1.6, 0.45, 1.05, 0.4), box(x + 1.1, y - 1.05, z - 1.6, 0.45, 1.05, 0.4));
  const roof = box(x, y - 2.8, z, 2.35, 0.12, 1.75);
  const toilets = roundBox(abs(x) - 1.1, y - 0.22, z + 0.9, 0.18, 0.22, 0.25, 0.08);
  return min(min(max(body, -min(rooms, doors)), roof), toilets);""",
  """  if (z > 1.55 && y > 2.2 && y < 2.55 && abs(abs(x) - 1.1) < 0.6) return rgb(40, 70, 150);
  if (y > 2.65) return rgb(176, 78, 44);
  if (abs(z - 0.9) < 0.3 && y < 0.5 && abs(abs(x) - 1.1) < 0.2) return rgb(246, 246, 246);
  if (y < 0.3) return rgb(160, 150, 136);
  return rgb(244, 240, 232);""",
  ['{ kind: "door", at: [-1.1, 0, 1.6], face: 0, size: [0.9, 2.1] }', '{ kind: "door", at: [1.1, 0, 1.6], face: 0, size: [0.9, 2.1] }'])

t("flower-stand", "The florist's stand outside her shop: tiered wooden steps of zinc buckets\nfull of roses, sunflowers, lilies, carnations and lavender.",
  "flower stand", (1.0, 1.2, 0.6), '["prop", "shop"]', "{ mass: 60, alive: 1, burns: 0.6 }",
  """  const steps = min(box(x, y - 0.2, z - 0.25, 0.95, 0.2, 0.3), box(x, y - 0.5, z + 0.2, 0.95, 0.5, 0.25));
  const bx = x - 0.32 * floor(x / 0.32 + 0.5);
  const lowb = max(ellipsoid(bx, y - 0.65, z - 0.25, 0.14, 0.25, 0.14), abs(x) - 0.9);
  const highb = max(ellipsoid(bx, y - 1.05, z + 0.2, 0.14, 0.2, 0.14), abs(x) - 0.9);
  return min(steps, min(lowb, highb));""",
  f"""  if ((y > 0.4 && z < 0.0) || y > 1.0) {{
    if (noise3(x * 14, y * 14, z * 14) > 0.4) return hsv(fract(floor(x / 0.32 + 0.5) * 0.23 + floor(y * 2.0) * 0.4), 0.7, 0.95);
    return rgb(60, 120, 50);
  }}
  return {WOOD};""")

# ------------------------------------------------------------- to ride
t("bicycle", "A sturdy town bicycle with a basket on the front, a bell and a rack behind:\nblack, red or green paint. Ridden by pedalling; its front is +z.",
  "bicycle", (0.3, 1.05, 0.95), '["vehicle", "bicycle"]', "{ mass: 15 }",
  """  const wf = rotZ(x, y - 0.34, z - 0.55, 1.5708);
  const wb = rotZ(x, y - 0.34, z + 0.55, 1.5708);
  const wheels = min(torus(wf.x, wf.y, wf.z, 0.32, 0.025), torus(wb.x, wb.y, wb.z, 0.32, 0.025));
  const frame = min(min(capsule(x, y, z, 0, 0.34, -0.55, 0, 0.75, -0.1, 0.02), capsule(x, y, z, 0, 0.75, -0.1, 0, 0.82, 0.45, 0.02)), min(capsule(x, y, z, 0, 0.82, 0.45, 0, 0.34, 0.55, 0.02), capsule(x, y, z, 0, 0.34, -0.55, 0, 0.4, 0.05, 0.02)));
  const seat = roundBox(x, y - 0.85, z + 0.15, 0.07, 0.03, 0.12, 0.02);
  const bars = capsule(x, y, z, -0.25, 0.98, 0.5, 0.25, 0.98, 0.5, 0.015);
  const basket = box(x, y - 0.85, z - 0.75, 0.16, 0.1, 0.13);
  return min(min(wheels, frame), min(seat, min(bars, basket)));""",
  """  if (abs(y - 0.85) < 0.12 && z > 0.6) return rgb(190, 150, 90);
  if (y > 0.82 && abs(z - 0.15) < 0.14) return rgb(60, 40, 30);
  const c = floor(hash(k.seed) * 3);
  return c < 1 ? rgb(30, 30, 32) : c < 2 ? rgb(170, 30, 30) : rgb(40, 110, 70);""",
  ['{ kind: "seat", at: [0, 0.82, -0.15], face: 0 }'], extra='  drive: { speed: 5, on: "land", by: "legs" },')

t("vespa", "A Vespa scooter in mint green, cream or red: the round steel body over its\nlittle back wheel, a long saddle for two, a chrome headlamp on the handlebar\nand a spare wheel behind. Its front is +z.",
  "vespa", (0.4, 1.12, 0.95), '["vehicle", "scooter"]', "{ mass: 90, fuel: 0.4 }",
  """  const wf = rotZ(x, y - 0.18, z - 0.6, 1.5708);
  const wb = rotZ(x, y - 0.18, z + 0.6, 1.5708);
  const wheels = min(torus(wf.x, wf.y, wf.z, 0.14, 0.06), torus(wb.x, wb.y, wb.z, 0.14, 0.06));
  const body = ellipsoid(x, y - 0.42, z + 0.42, 0.28, 0.25, 0.42);
  const floorboard = box(x, y - 0.25, z - 0.15, 0.17, 0.04, 0.35);
  const shield = box(x, y - 0.55, z - 0.5, 0.2, 0.32, 0.05);
  const stem = capsule(x, y, z, 0, 0.3, 0.6, 0, 0.95, 0.52, 0.03);
  const bars = capsule(x, y, z, -0.3, 0.97, 0.5, 0.3, 0.97, 0.5, 0.022);
  const lamp = sphere(x, y - 0.98, z - 0.58, 0.07);
  const saddle = roundBox(x, y - 0.7, z + 0.3, 0.13, 0.05, 0.38, 0.04);
  return min(min(wheels, body), min(min(floorboard, shield), min(min(stem, bars), min(lamp, saddle))));""",
  """  if (y > 0.92 && z > 0.52) return glow(rgb(250, 246, 220), 0.6);
  if (abs(y - 0.7) < 0.06 && z < -0.05) return rgb(40, 34, 30);
  if (y < 0.32 && abs(z) > 0.45) return rgb(30, 30, 30);
  const c = floor(hash(k.seed) * 3);
  return c < 1 ? rgb(150, 210, 190) : c < 2 ? rgb(240, 230, 200) : rgb(200, 40, 40);""",
  ['{ kind: "seat", at: [0, 0.75, -0.05], face: 0 }', '{ kind: "seat", at: [0, 0.75, -0.45], face: 0 }'], extra='  drive: { speed: 12, on: "land", by: "engine" },')

t("skateboard", "A skateboard painted with a flame, red wheels. Ridden standing on it,\npushing with one foot; its front is +z.",
  "skateboard", (0.12, 0.08, 0.42), '["vehicle", "toy"]', "{ mass: 2.5, toy: 1 }",
  """  const deck = roundBox(x, y - 0.07, z, 0.1, 0.012, 0.4, 0.05);
  const wheels = cylinder(abs(z) - 0.28 + 0.0 * x, abs(x) - 0.08, y - 0.03, 0.025, 0.02);
  return min(deck, wheels);""",
  """  if (y < 0.055) return rgb(220, 40, 40);
  return fract(z * 3.0) < 0.5 ? rgb(250, 160, 40) : rgb(30, 30, 30);""",
  ['{ kind: "seat", at: [0, 0.09, 0], face: 0 }'], extra='  drive: { speed: 4, on: "land", by: "legs" },')
