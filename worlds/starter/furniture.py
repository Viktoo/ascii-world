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


def pumpkin_js(v, cx, cy, cz, R, squash=1.35, ribs=5):
    """JS lines for one ribbed pumpkin centred at (cx, cy, cz), radius R:
    lobes round it, flattened, a dimple on top; its distance is `v`."""
    return f"""  const {v}x = x - ({cx});
  const {v}y = y - ({cy});
  const {v}z = z - ({cz});
  const {v}a = atan2({v}z, {v}x);
  const {v}r = {R} * (0.9 + 0.1 * abs(cos({v}a * {ribs / 2:.1f})));
  const {v}b = (length3({v}x, {v}y * {squash}, {v}z) - {v}r) * 0.72;
  const {v} = smoothSubtract({v}b, sphere({v}x, {v}y - {R / squash + 0.02:.3f}, {v}z, {R * 0.28:.3f}), {R * 0.12:.3f});
  const {v}s = capsule({v}x, {v}y, {v}z, 0, {R / squash - 0.04:.3f}, 0, {R * 0.12:.3f}, {R / squash + R * 0.32:.3f}, {R * 0.05:.3f}, {R * 0.09:.3f});"""


def pumpkin_col(v, cx, cy, cz, R, squash=1.35, ribs=5, base="rgb(222, 108, 24)"):
    """JS colour for the pumpkin `v`: stalk, grooves darker, the lobes lit."""
    return f"""  {{
    const px = x - ({cx});
    const py = y - ({cy});
    const pz = z - ({cz});
    if (length3(px, py * {squash}, pz) < {R * 1.08:.3f} || (py > {R / squash - 0.06:.3f} && length2(px, pz) < {R * 0.2:.3f})) {{
      if (py > {R / squash - 0.05:.3f} && length2(px, pz) < {R * 0.16:.3f}) return mix(rgb(96, 104, 54), rgb(130, 120, 70), noise3(px * 40, py * 40, pz * 40));
      const g = abs(cos(atan2(pz, px) * {ribs / 2:.1f}));
      return {base} * (0.62 + 0.38 * g) * (0.88 + 0.12 * noise3(px * 20, py * 20, pz * 20)) * (0.85 + 0.15 * clamp(py / {R:.3f} + 0.6, 0, 1));
    }}
  }}"""


def fish_rows(px, pz, y0, wx=0.075, wz=0.13, L=0.1):
    """JS lines: a layer of fish side by side, one to each cell of a grid
    (px, pz the cell size), lying along z at height y0; distance `fish`."""
    return f"""  const fcx = x - {px} * floor(x / {px} + 0.5);
  const fcz = z - {pz} * floor(z / {pz} + 0.5) + (fract(floor(x / {px} + 0.5) * 0.5) - 0.25) * 0.06;
  const fbody = ellipsoid(fcx, y - {y0}, fcz, {wx * 0.42:.3f}, {wx * 0.3:.3f}, {L:.3f});
  const ftail = triangle(fcx, y - {y0}, fcz, 0, 0, -{L * 0.85:.3f}, -{wx * 0.4:.3f}, 0, -{L * 1.25:.3f}, {wx * 0.4:.3f}, 0, -{L * 1.25:.3f}, {wx * 0.06:.3f});
  const fish = min(fbody, ftail);"""


def fish_col(px, pz, y0, wx=0.075, L=0.1):
    """JS colour of those fish: a blue-grey back, a silver side, a white
    belly, a dark eye."""
    return f"""  {{
    const fcx = x - {px} * floor(x / {px} + 0.5);
    const fcz = z - {pz} * floor(z / {pz} + 0.5) + (fract(floor(x / {px} + 0.5) * 0.5) - 0.25) * 0.06;
    if (abs(y - {y0}) < {wx * 0.34:.3f} && abs(fcx) < {wx * 0.5:.3f}) {{
      if (fcz > {L * 0.7:.3f} && abs(y - {y0} - {wx * 0.08:.3f}) < {wx * 0.1:.3f} && abs(fcx) > {wx * 0.25:.3f}) return rgb(20, 20, 24);
      if (y - {y0} > {wx * 0.12:.3f}) return mix(rgb(60, 80, 100), rgb(90, 110, 120), hash(floor(x / {px} + 0.5), 3));
      if (y - {y0} > -{wx * 0.1:.3f}) return mix(rgb(196, 204, 210), rgb(230, 234, 236), noise3(x * 60, y * 60, z * 60));
      return rgb(240, 240, 236);
    }}
  }}"""


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

t("dining-table", "The family table: a long wooden table under a red-and-white checked cloth,\nfour chairs round it, laid with bowls of stew, a loaf of brown bread\nand a jug of cider. Four can sit and eat.",
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
  f"""  if (y > 0.84 && length2(x, z) < 0.13) return rgb(200, 150, 70);
  if (y > 0.76 && abs(z - 0.22) < 0.07 && abs(x) < 0.15) return rgb(200, 150, 80);
  if (y > 0.79 && abs(abs(z) - 0.2) < 0.1 && abs(abs(x) - 0.4) < 0.1) return noise3(x * 20, y * 20, z * 20) > 0.6 ? rgb(120, 70, 40) : rgb(170, 110, 60);
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
t("shop-counter", "A shop counter of varnished wood with a worn top, an old brass till and a\nset of scales on it. The shopkeeper stands behind it (-z); customers at its\nfront (+z).",
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

shelves("bread-shelves", "bread shelves", "The baker's shelves behind the counter, stacked with round loaves, brown\nbread, apple pies and cinnamon buns.",
        """    return mix(rgb(196, 130, 60), rgb(236, 190, 110), h) * (0.85 + 0.15 * noise3(x * 20, y * 20, z * 20));""")
shelves("grocery-shelves", "grocery shelves", "The general store's shelves: jars of maple syrup and jam, sacks of flour and\noats, tins of coffee, candles, lamp oil, rope and twine.",
        """    if (h < 0.25) return rgb(200, 40, 30);
    if (h < 0.5) return rgb(236, 200, 110);
    if (h < 0.75) return rgb(110, 130, 40);
    return rgb(40, 60, 120);""")
shelves("liquor-shelves", "liquor shelves", "Shelves of bottles behind the tavern's bar: amber whiskey and dark rum,\njugs of hard cider, and a row of pewter tankards.",
        """    if (h < 0.4) return rgb(70, 20, 30);
    if (h < 0.65) return rgb(230, 220, 120);
    return rgb(40, 100, 50);""")
shelves("book-shelves", "book shelves", "The bookseller's shelves, full of books new and old, sea charts of the coast\nand almanacs.",
        """    return hsv(h, 0.55, 0.5 + 0.3 * hash(k.seed, i));""")
t("bar-counter", "The tavern's bar: a long counter of dark polished wood, three brass beer\ntaps at one end, mugs stacked on it, a jar of pickled eggs and a bowl of\npeanuts.",
  "bar counter", (1.6, 1.4, 0.4), '["furniture", "counter", "shop"]', "{ mass: 200, heat: 30 }",
  """  const body = box(x, y - 0.52, z, 1.55, 0.52, 0.32);
  const top = box(x, y - 1.06, z, 1.6, 0.025, 0.38);
  const machine = roundBox(x + 1.05, y - 1.3, z + 0.05, 0.35, 0.22, 0.22, 0.05);
  const cups = max(cylinder(x - 0.12 * floor(x / 0.12 + 0.5), y - 1.12, z, 0.04, 0.04), abs(x + 0.2) - 0.35);
  const dome = max(sphere(x + 0.5, y - 1.1, z, 0.2), 1.08 - y);
  return min(min(body, top), min(machine, min(cups, dome)));""",
  f"""  if (abs(x - 1.05) < 0.4 && y > 1.08) return rgb(196, 160, 70);
  if (y > 1.08 && abs(x + 0.5) < 0.22) return rgb(210, 190, 140);
  if (y > 1.08) return rgb(226, 170, 60);
  if (y > 1.03) return rgb(70, 46, 30);
  return {WOOD};""")

t("tavern-table", "A heavy tavern table of scrubbed pine, four chairs round it, laid with\na pot of clam chowder, foaming mugs of beer and a basket of bread.",
  "tavern table", (0.95, 1.0, 0.95), '["furniture", "table"]', "{ mass: 40, burns: 0.6 }",
  """  const top = box(x, y - 0.74, z, 0.45, 0.025, 0.45);
  const leg = cylinder(x, y - 0.37, z, 0.05, 0.37);
  const pot = cylinder(x, y - 0.775, z, 0.25, 0.012);
  const carafe = cappedCone(x + 0.3, y - 0.86, z - 0.3, 0.1, 0.07, 0.03);
  const ax = abs(x) - 0.68;
  const az = abs(z) - 0.68;
  const seats = min(box(ax, y - 0.45, z, 0.18, 0.025, 0.18), box(x, y - 0.45, az, 0.18, 0.025, 0.18));
  const backs = min(box(abs(x) - 0.86, y - 0.7, z, 0.02, 0.24, 0.18), box(x, y - 0.7, abs(z) - 0.86, 0.18, 0.24, 0.02));
  return min(min(top, leg), min(min(pot, carafe), min(seats, backs)));""",
  f"""  if (y > 0.76 && length2(x, z) < 0.25) return noise3(x * 25, y * 25, z * 25) > 0.7 ? rgb(200, 180, 140) : rgb(236, 226, 196);
  if (y > 0.78 && length2(x - 0.3, z + 0.3) < 0.1) return y > 0.93 ? rgb(250, 246, 236) : rgb(214, 150, 40);
    return {WOOD};""",
  ['{ kind: "seat", at: [0.68, 0.47, 0], face: 270 }', '{ kind: "seat", at: [-0.68, 0.47, 0], face: 90 }',
   '{ kind: "seat", at: [0, 0.47, 0.68], face: 180 }', '{ kind: "seat", at: [0, 0.47, -0.68], face: 0 }'])

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

t("boat-on-trestles", "A lobster boat half built on trestles in the boat shop: ribs and fresh\nplanks of oak, half of it painted white, a pot of paint and brushes\nbeside it.",
  "boat on trestles", (0.95, 1.5, 2.6), '["furniture", "prop"]', "{ mass: 400, burns: 0.6 }",
  """  const tr = box(abs(x) - 0.0, y - 0.35, abs(z) - 1.6, 0.8, 0.35, 0.08);
  const t2 = clamp(z / 2.5, -1, 1);
  const w = 0.85 * sqrt(max(1 - t2 * t2, 0.02));
  const hull = max(max(length2(x / max(w, 0.05), (y - 1.15) / 0.5) - 1, y - 1.25), abs(z) - 2.5) * min(w, 0.4);
  const inner = max(length2(x / max(w - 0.06, 0.03), (y - 1.2) / 0.44) - 1, abs(z) - 2.4) * 0.4;
  return min(tr, max(hull, -inner));""",
  f"""  if (y < 0.72) return {DARK};
  if (z > 0.0 && y > 0.75) return rgb(240, 238, 230);
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

item("newspaper", "newspaper", "This week's Cove Gazette, folded: the tides, the fishing, the harvest supper on the green.", (0.17, 0.02, 0.12), "{ mass: 0.2, burns: 0.95 }",
     """  return box(x, y - 0.01, z, 0.16, 0.01, 0.11);""",
     """  return fract(z * 18.0) < 0.4 ? rgb(70, 70, 70) : rgb(236, 232, 220);""")
item("broom", "broom", "A broom of yellow straw on a long wooden handle, for sweeping the step.", (0.15, 0.7, 0.06), "{ mass: 0.8, burns: 0.8 }",
     """  return min(cylinder(x, y - 0.22, z, 0.014, 0.48), max(cappedCone(x, y + 0.42, z, 0.18, 0.15, 0.04) * 0.0 + box(x, y + 0.5, z, 0.13 - (y + 0.5) * 0.25, 0.18, 0.04), -0.0 - 1.0));""",
     """  if (y < -0.32) return rgb(220, 190, 90);
  return rgb(150, 110, 70);""", tags='["item", "tool"]', extra='  tool: { grip: [0, 0.5, 0], tip: [0, -0.65, 0], motions: ["swing"], sharp: 0 },')
item("bucket", "bucket", "A zinc bucket of water with a wire handle.", (0.15, 0.42, 0.16), "{ mass: 6, wet: 1 }",
     """  return min(max(cappedCone(x, y - 0.13, z, 0.13, 0.11, 0.14), -cappedCone(x, y - 0.15, z, 0.12, 0.1, 0.13)), max(abs(length2(x, y - 0.26) - 0.13) - 0.006, abs(z) - 0.006));""",
     """  if (y > 0.2 && length2(x, z) < 0.13) return rgb(90, 140, 170);
  return rgb(170, 176, 180);""", tags='["item", "tool"]', extra='  tool: { grip: [0, 0.38, 0], tip: [0, 0.2, 0.15], motions: ["pour"], sharp: 0 },')
item("watering-can", "watering can", "A green tin watering can with a long spout, for the flower boxes.", (0.12, 0.38, 0.32), "{ mass: 3, wet: 0.8 }",
     """  return min(min(cylinder(x, y - 0.11, z, 0.1, 0.11), cone(x, y, z, 0, 0.05, 0.08, 0, 0.22, 0.28, 0.025, 0.015)), max(abs(length2(z + 0.02, y - 0.2) - 0.08) - 0.01, abs(x) - 0.01));""",
     """  return rgb(60, 130, 80);""", tags='["item", "tool"]', extra='  tool: { grip: [0, 0.28, -0.02], tip: [0, 0.22, 0.28], motions: ["pour"], sharp: 0 },')
# ------------------------------------------------------------- in the street
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

# ------------------------------------------------------------- Maple Cove
item("mug-of-beer", "mug of beer", "A heavy glass mug of the tavern's amber ale, a head of foam on top.", (0.07, 0.17, 0.07), "{ mass: 0.8, edible: 0.3, fragile: 0.6 }",
     """  return min(cylinder(x, y - 0.075, z, 0.05, 0.075), max(abs(length2(x - 0.065, y - 0.08) - 0.035) - 0.008, abs(z) - 0.01));""",
     """  if (y > 0.13) return rgb(250, 246, 232);
  return glow(rgb(214, 150, 40), 0.15);""", tags='["item", "food", "drink"]')
item("basket-of-apples", "basket of apples", "A bushel basket heaped with red and gold apples from the orchards.", (0.25, 0.32, 0.25), "{ mass: 8, edible: 0.8, burns: 0.6 }",
     """  const basket = max(cappedCone(x, y - 0.14, z, 0.14, 0.17, 0.23), -cappedCone(x, y - 0.17, z, 0.14, 0.16, 0.22));
  const heap = max(ellipsoid(x, y - 0.26, z, 0.21, 0.07, 0.21) + (noise3(x * 18, y * 18, z * 18) - 0.5) * 0.05, -y + 0.2);
  return min(basket, heap);""",
     """  if (y > 0.24) return noise3(x * 20, y * 20, z * 20) > 0.55 ? rgb(226, 180, 50) : rgb(190, 30, 30);
  return fract(y * 14.0) < 0.3 ? rgb(150, 110, 64) : rgb(196, 156, 100);""", tags='["item", "food"]')
item("jar-of-honey", "jar of honey", "A glass jar of golden wildflower honey from the hives at Pine Hollow, a cloth tied over its lid.", (0.06, 0.13, 0.06), "{ mass: 0.6, edible: 0.6, fragile: 0.7 }",
     """  return min(cylinder(x, y - 0.055, z, 0.05, 0.055), cylinder(x, y - 0.12, z, 0.058, 0.012));""",
     """  if (y > 0.105) return rgb(220, 70, 60);
  return glow(rgb(226, 150, 30), 0.2);""", tags='["item", "food"]')
item("apple-pie", "apple pie", "A golden apple pie with a lattice top, still warm from the bakery's oven.", (0.15, 0.05, 0.15), "{ mass: 1.0, edible: 0.9, burns: 0.4 }",
     """  return min(cappedCone(x, y - 0.025, z, 0.025, 0.12, 0.14), ellipsoid(x, y - 0.045, z, 0.13, 0.02, 0.13));""",
     """  if (y > 0.05) return (fract(x * 12.0) < 0.3 || fract(z * 12.0) < 0.3) ? rgb(220, 160, 80) : rgb(150, 70, 30);
  return rgb(200, 140, 70);""", tags='["item", "food"]')
item("carving", "carving", "A little deer whittled from birch wood, smooth and pale, small enough to hold in one hand.", (0.05, 0.11, 0.07), "{ mass: 0.1, burns: 0.9 }",
     """  const b = ellipsoid(x, y - 0.06, z, 0.025, 0.022, 0.045);
  const legs = box(abs(x) - 0.012, y - 0.02, abs(z) - 0.03, 0.006, 0.02, 0.006);
  const head = sphere(x, y - 0.09, z - 0.05, 0.016);
  return min(min(b, legs), head);""",
     """  return mix(rgb(220, 200, 160), rgb(236, 220, 186), noise3(x * 40, y * 40, z * 40));""", tags='["item", "toy"]')
item("pumpkin", "pumpkin", "A big ribbed orange pumpkin from the garden, deep grooves between its\nlobes, a dimple on top and its curled green stalk still on.", (0.27, 0.42, 0.27), "{ mass: 7, edible: 0.6, burns: 0.3 }",
     pumpkin_js("p", 0, 0.18, 0, 0.25) + """
  return min(p, ps);""",
     pumpkin_col("p", 0, 0.18, 0, 0.25) + """
  return rgb(222, 108, 24);""", tags='["item", "food"]')
item("hand-lantern", "hand lantern", "A tin hand lantern with a glass chimney, its wick burning warm.", (0.08, 0.3, 0.08), "{ mass: 1.2, fragile: 0.5, light: 1, heat: 30 }",
     """  return min(min(cylinder(x, y - 0.03, z, 0.07, 0.03), cylinder(x, y - 0.13, z, 0.045, 0.07)), max(abs(length2(x, y - 0.24) - 0.05) - 0.006, abs(z) - 0.006));""",
     """  if (abs(y - 0.13) < 0.07) return glow(rgb(255, 200, 110), 0.9);
  return rgb(80, 80, 76);""", tags='["item", "tool"]')

# ------------------------------------------------------------- outdoors
t("pumpkins", "A heap of pumpkins and squash by a door or a porch step: a big ribbed orange\npumpkin and a smaller one on it, a little white pumpkin and a warty green\ngourd, their stalks still on.",
  "pumpkins", (0.95, 0.62, 0.62), '["prop", "decoration"]', "{ mass: 40, edible: 0.6, burns: 0.3 }",
  pumpkin_js("a", -0.38, 0.2, 0, 0.27) + pumpkin_js("b", 0.18, 0.15, 0.12, 0.2) + pumpkin_js("c", 0.62, 0.1, -0.18, 0.13, 1.5, 4) + pumpkin_js("d", -0.3, 0.47, 0.02, 0.13) + """
  const gourd = (length3(x - 0.2, (y - 0.09) * 1.0, (z + 0.32) * 0.7) - 0.09 + noise3(x * 30, y * 30, z * 30) * 0.02) * 0.6;
  return min(min(min(a, as), min(b, bs)), min(min(min(c, cs), min(d, ds)), gourd));""",
  pumpkin_col("d", -0.3, 0.47, 0.02, 0.13, base="rgb(236, 140, 36)") + pumpkin_col("a", -0.38, 0.2, 0, 0.27) + pumpkin_col("b", 0.18, 0.15, 0.12, 0.2, base="rgb(214, 96, 22)") + pumpkin_col("c", 0.62, 0.1, -0.18, 0.13, 1.5, 4, base="rgb(240, 236, 220)") + """
  if (z < -0.2) return mix(rgb(70, 110, 50), rgb(200, 190, 90), step(0.6, noise3(x * 25, y * 25, z * 25)));
  return rgb(222, 108, 24);""")
t("picket-fence", "A length of white picket fence, 2 m, round a cottage garden.",
  "picket fence", (1.05, 1.0, 0.08), '["prop", "fence"]', "{ mass: 40, burns: 0.6 }",
  """  const px = x - 0.16 * floor(x / 0.16 + 0.5);
  const pickets = max(max(box(px, y - 0.45, z, 0.04, 0.45, 0.015), (y - 0.86) + abs(px) * 1.5), abs(x) - 1.0);
  const rails = box(x, abs(y - 0.5) - 0.22, z + 0.03, 1.0, 0.03, 0.015);
  const posts = box(abs(x) - 0.98, y - 0.5, z, 0.04, 0.5, 0.04);
  return min(min(pickets, rails), posts);""",
  """  return rgb(244, 242, 236) * (0.94 + 0.06 * noise3(x * 6, y * 6, z * 6));""")
t("park-bench", "A green-painted wooden bench of slats on iron legs, for sitting on the\ngreen or the landing and watching the sun go down.",
  "park bench", (0.9, 0.85, 0.35), '["bench", "furniture", "prop"]', "{ mass: 40, burns: 0.5 }",
  """  const sz = z - 0.1 * floor(z / 0.1 + 0.5);
  const seat = max(box(x, y - 0.45, z, 0.85, 0.025, 0.22), abs(sz) - 0.035);
  const back = box(x, y - 0.68 - 0.0, z + 0.25 - (y - 0.45) * 0.2, 0.85, 0.2, 0.02);
  const legs = box(abs(x) - 0.75, y - 0.23, z, 0.03, 0.23, 0.22);
  return min(min(seat, back), legs);""",
  """  if (y < 0.42) return rgb(40, 40, 40);
  return rgb(56, 100, 66);""",
  ['{ kind: "seat", at: [-0.45, 0.46, -0.05], face: 0 }', '{ kind: "seat", at: [0.45, 0.46, -0.05], face: 0 }'])
t("rocking-chair", "A painted wooden rocking chair on a porch, a folded quilt over its arm.",
  "rocking chair", (0.35, 1.1, 0.45), '["furniture", "seat"]', "{ mass: 12, burns: 0.6 }",
  """  const seat = box(x, y - 0.45, z, 0.25, 0.03, 0.22);
  const back = box(x, y - 0.8, z + 0.24 - (y - 0.45) * 0.2, 0.25, 0.35, 0.02);
  const rock = max(abs(length2(z, y - 1.4) - 1.38) - 0.02, max(abs(abs(x) - 0.23) - 0.02, abs(z) - 0.4));
  const legs = box(abs(x) - 0.23, y - 0.25, abs(z) - 0.18, 0.02, 0.22, 0.02);
  return min(min(seat, back), min(rock, legs));""",
  """  if (y > 0.48 && y < 0.55 && x > 0.15) return rgb(170, 50, 40);
  return rgb(40, 70, 56);""",
  ['{ kind: "seat", at: [0, 0.47, -0.05], face: 0 }'])
t("water-pump", "The village pump on the green: a cast-iron hand pump with a long handle\nover a granite trough, cold, sweet well water for anyone.",
  "water pump", (0.5, 1.4, 0.75), '["prop", "well", "source"]', "{ mass: 300, wet: 1 }",
  """  const col = cylinder(x, y - 0.6, z, 0.09, 0.6);
  const head = cylinder(x, y - 1.25, z, 0.11, 0.07);
  const spout = capsule(x, y, z, 0, 0.95, 0, 0, 0.9, 0.22, 0.035);
  const handle = capsule(x, y, z, 0, 1.25, 0, 0, 1.38, -0.45, 0.025);
  const trough = max(box(x, y - 0.25, z - 0.5, 0.45, 0.25, 0.25), -box(x, y - 0.35, z - 0.5, 0.38, 0.2, 0.18));
  return min(min(col, head), min(min(spout, handle), trough));""",
  """  if (abs(z - 0.5) < 0.26 && y < 0.5) return y > 0.4 && abs(x) < 0.38 && abs(z - 0.5) < 0.18 ? rgb(70, 110, 130) : rgb(150, 148, 140);
  return rgb(40, 50, 46);""",
  extra='  sound: { hard: 1, dry: 0.3, ring: 0.5 },')
t("gazebo", "The gazebo on the green: an eight-sided bandstand of white posts and\nrailings on a raised floor, two steps up, under a pointed red roof with a\nweathervane; benches round the inside where people sit and the fiddler\nplays. Its steps face +z.",
  "gazebo", (3.6, 5.6, 3.6), '["building", "landmark", "shelter"]', "{ mass: 8000, burns: 0.6 }",
  """  // Fold round the eight sides.
  const a = atan2(x, z);
  const sec = floor(a / 0.7854 + 0.5) * 0.7854;
  const u = x * sin(sec) + z * cos(sec);
  const v = x * cos(sec) - z * sin(sec);
  const r = max(abs(x), abs(z)) * 0.0 + u;
  const floor0 = max(u - 3.0, abs(y - 0.25) - 0.25);
  const step1 = max(max(abs(x) - 0.8, z - 3.6), max(-z + 2.8, abs(y - 0.12) - 0.12));
  const posts = box(u - 2.85, y - 1.75, abs(v) - 1.17, 0.08, 1.25, 0.08);
  const rail = max(max(box(u - 2.85, y - 1.35, v, 0.04, 0.04, 1.18), -box(x, y - 1.3, z - 2.9, 0.8, 1.0, 0.6)), 0.0 - 1.0);
  const roof = max(max(u - 3.35 + (y - 3.0) * 1.3, 3.0 - y), y - 5.2);
  const vane = capsule(x, y, z, 0, 5.0, 0, 0, 5.55, 0, 0.025);
  const benches = max(box(u - 2.55, y - 0.95, v, 0.18, 0.04, 1.0), z - 2.4);
  return min(min(min(floor0, step1), min(posts, rail)), min(min(roof, vane), benches));""",
  """  if (y > 2.95 && y < 5.0) return mix(rgb(176, 44, 38), rgb(196, 60, 46), noise3(x * 4, y * 4, z * 4)) * (fract(y * 4.0) < 0.15 ? 0.85 : 1.0);
  if (y > 5.0) return rgb(40, 40, 40);
  if (y < 0.51) return fract(x * 4.0) < 0.06 ? rgb(130, 110, 90) : rgb(176, 156, 126);
  return rgb(246, 244, 238);""",
  ['{ kind: "seat", at: [-2.4, 1.0, 0], face: 90 }', '{ kind: "seat", at: [2.4, 1.0, 0], face: 270 }', '{ kind: "seat", at: [0, 1.0, -2.4], face: 0 }',
   '{ kind: "seat", at: [-1.7, 1.0, -1.7], face: 45 }', '{ kind: "seat", at: [1.7, 1.0, -1.7], face: 315 }', '{ kind: "light", at: [0, 2.9, 0] }'])
t("apple-barrel", "An open wooden barrel on the store's porch, heaped with red apples for sale.",
  "apple barrel", (0.38, 0.95, 0.38), '["prop", "shop", "source"]', "{ mass: 60, edible: 0.8, burns: 0.5 }",
  """  const barrel = max(length2(x, z) - 0.33 - 0.04 * (1.0 - abs(y - 0.42) / 0.42), abs(y - 0.42) - 0.42);
  const heap = ellipsoid(x, y - 0.84, z, 0.3, 0.1, 0.3) + (noise3(x * 16, y * 16, z * 16) - 0.5) * 0.06;
  return min(barrel, heap);""",
  f"""  if (y > 0.8) return rgb(190, 30, 30);
  if (abs(abs(y - 0.42) - 0.28) < 0.03) return rgb(60, 60, 62);
  return {WOOD};""")

# ------------------------------------------------------------- the pier
t("pier-section", "Six metres of the wooden pier: a deck of grey weathered planks on stout\ntarred piles driven into the cove's floor, a rail along each side. Its\ndeck is 7.8 m over its foot; it runs along z.",
  "pier section", (1.75, 8.9, 3.02), '["walkway", "pier"]', "{ mass: 20000, burns: 0.5 }",
  """  const deck = box(x, y - 7.7, z, 1.6, 0.1, 3.0);
  const piles = box(abs(x) - 1.45, y - 3.9, z - 3.0 * floor(z / 3.0 + 0.5), 0.14, 3.9, 0.14);
  const cap = box(x, y - 7.45, z - 3.0 * floor(z / 3.0 + 0.5), 1.62, 0.12, 0.12);
  const rail = box(abs(x) - 1.55, y - 8.75, z, 0.05, 0.05, 3.0);
  const posts = box(abs(x) - 1.55, y - 8.3, z - 1.5 * floor(z / 1.5 + 0.5), 0.05, 0.5, 0.05);
  return min(min(deck, piles), min(cap, min(rail, posts)));""",
  """  if (y < 7.3) return y < 6.2 ? rgb(40, 34, 30) : rgb(70, 60, 50);
  if (y < 7.85) return (fract(z * 4.0) < 0.07 ? 0.7 : 1.0) * mix(rgb(150, 140, 126), rgb(176, 166, 150), noise3(x * 2, y * 4, z * 9));
  return mix(rgb(160, 150, 136), rgb(186, 176, 160), noise3(x * 6, y * 6, z * 6));""")
t("pier-head", "The head of the wooden pier: a broad square deck on piles, open on its\nseaward side where a ladder goes down to the boats, a bench to sit on and\nwatch the sunset, a lamp on a post. Its deck is 7.8 m over its foot; the\npier comes in at -z.",
  "pier head", (4.1, 10.0, 4.1), '["walkway", "pier"]', "{ mass: 40000, burns: 0.5 }",
  """  const deck = box(x, y - 7.7, z, 4.0, 0.1, 4.0);
  const px = x - 2.6 * floor(x / 2.6 + 0.5);
  const pz = z - 2.6 * floor(z / 2.6 + 0.5);
  const piles = max(box(px, y - 3.9, pz, 0.15, 3.9, 0.15), max(abs(x) - 4.0, abs(z) - 4.0));
  const sides = box(abs(x) - 3.95, y - 8.75, z, 0.05, 0.05, 4.0);
  const back = max(box(x, y - 8.75, z + 3.95, 4.0, 0.05, 0.05), -box(x, y - 8.7, z + 3.95, 1.7, 0.2, 0.2));
  const posts = max(box(abs(x) - 3.95, y - 8.3, z - 2.0 * floor(z / 2.0 + 0.5), 0.05, 0.5, 0.05), abs(z) - 4.0);
  const ladder = min(box(abs(x - 2.0) - 0.25, y - 6.4, z - 4.08, 0.04, 1.4, 0.04), max(box(x - 2.0, y - 0.3 * floor(y / 0.3) - 0.15, z - 4.08, 0.25, 0.02, 0.03), max(y - 7.6, 5.0 - y)));
  const lamp = min(cylinder(x + 3.7, y - 8.6, z - 3.7, 0.06, 0.8), sphere(x + 3.7, y - 9.5, z - 3.7, 0.18));
  return min(min(min(deck, piles), min(sides, back)), min(posts, min(ladder, max(lamp, y - 9.98))));""",
  """  if (length2(x + 3.7, z - 3.7) < 0.2 && y > 9.35) return glow(rgb(255, 214, 140), 0.8);
  if (length2(x + 3.7, z - 3.7) < 0.1) return rgb(36, 36, 36);
  if (y < 7.3) return y < 6.2 ? rgb(40, 34, 30) : rgb(70, 60, 50);
  if (y < 7.85) return (fract(x * 4.0) < 0.07 ? 0.7 : 1.0) * mix(rgb(150, 140, 126), rgb(176, 166, 150), noise3(x * 9, y * 4, z * 2));
  return mix(rgb(160, 150, 136), rgb(186, 176, 160), noise3(x * 6, y * 6, z * 6));""",
  ['{ kind: "light", at: [-3.7, 9.5, 3.7] }', '{ kind: "stairs", at: [2.0, 7.8, 3.6], to: [2.0, 5.0, 4.4], face: 0 }'])
t("dory", "A wooden dory, the fishermen's rowing boat: 4.5 m, flat-bottomed with high\nflared sides, buff inside, painted green, red or white outside, two thwarts\nand a pair of oars. Its front is +z.",
  "dory", (0.85, 0.75, 2.3), '["boat", "vehicle", "water"]', "{ mass: 200, burns: 0.4, fuel: 0.5 }",
  """  const t = clamp(z / 2.25, -1, 1);
  const w = 0.78 * sqrt(max(1 - t * t * t * t, 0.02)) * (0.55 + 0.45 * clamp((y + 0.2) / 0.6, 0, 1));
  const lift = 0.25 * t * t;
  const hull = max(max(abs(x) - w, abs(z) - 2.25), max(-0.2 - y, y - 0.4 - lift));
  const inside = max(max(abs(x) - w + 0.05, abs(z) - 2.15), -0.12 - y);
  const thwarts = box(x, y - 0.18, z - 1.1 * floor(z / 1.1 + 0.5), 0.7, 0.025, 0.12);
  return min(max(hull, -inside), max(thwarts, abs(z) - 1.5));""",
  """  const c = floor(hash(k.seed) * 3);
  let hue = rgb(70, 120, 80);
  if (c == 1) hue = rgb(170, 52, 40);
  if (c == 2) hue = rgb(240, 238, 230);
  const t = clamp(z / 2.25, -1, 1);
  const w = 0.78 * sqrt(max(1 - t * t * t * t, 0.02)) * (0.55 + 0.45 * clamp((y + 0.2) / 0.6, 0, 1));
  if (abs(x) < w - 0.03 && y > -0.13) return rgb(214, 196, 150);
  return y < -0.1 ? rgb(140, 50, 40) : hue;""",
  ['{ kind: "seat", at: [0, 0.2, -1.1], face: 0 }', '{ kind: "seat", at: [0, 0.2, 0], face: 0 }'], extra='  drive: { speed: 2.5, on: "water", by: "oars" },')

# ------------------------------------------------------------- the smithy
t("smithy", "The smithy: an open-fronted shed of dark boards under a slate-grey roof,\nthe forge glowing in its stone hearth at the back, an anvil on a stump, a\nquench tub and tongs and horseshoes hung on the wall. It opens to +z.",
  "smithy", (4.0, 5.2, 3.4), '["building", "workshop"]', "{ mass: 30000, burns: 0.3, light: 1, heat: 60 }",
  """  const back = box(x, y - 1.6, z + 3.0, 3.8, 1.6, 0.12);
  const sides = box(abs(x) - 3.7, y - 1.6, z, 0.12, 1.6, 3.0);
  const posts = box(abs(x) - 3.7, y - 1.6, z - 3.0, 0.12, 1.6, 0.12);
  const roof = max(box(x, y - 3.6 + z * 0.3, z, 4.0, 0.08, 3.4), 0.0 - 1.0);
  const forge = max(box(x + 1.6, y - 0.5, z + 2.2, 1.0, 0.5, 0.7), -box(x + 1.6, y - 0.95, z + 2.1, 0.6, 0.15, 0.4));
  const hood = cappedCone(x + 1.6, y - 2.3, z + 2.3, 0.8, 0.8, 0.25);
  const flue = box(x + 1.6, y - 3.8, z + 2.4, 0.25, 1.2, 0.25);
  const stump = cylinder(x - 0.6, y - 0.3, z + 0.6, 0.3, 0.3);
  const anvil = min(box(x - 0.6, y - 0.72, z + 0.6, 0.12, 0.12, 0.3), box(x - 0.6, y - 0.9, z + 0.6, 0.16, 0.06, 0.4));
  const tub = max(cylinder(x - 2.6, y - 0.35, z + 2.0, 0.45, 0.35), -cylinder(x - 2.6, y - 0.45, z + 2.0, 0.4, 0.35));
  return min(min(min(back, sides), min(posts, roof)), min(min(forge, min(hood, flue)), min(min(stump, anvil), tub)));""",
  """  if (abs(x + 1.6) < 0.62 && abs(z + 2.1) < 0.42 && y > 0.78 && y < 1.02) return glow(rgb(255, 120, 40), 1.0);
  if (y > 3.0 + 0.0 - z * 0.3 && abs(x + 1.6) > 0.3) return rgb(80, 86, 92);
  if (abs(x + 1.6) < 1.05 && z > 1.4 && y < 1.05) return mix(rgb(120, 116, 110), rgb(150, 144, 136), noise3(x * 5, y * 5, z * 5));
  if (abs(x - 0.6 - 0.0) < 0.2 && abs(z - 0.6) < 0.45 && y > 0.6 && abs(x + 0.6) < 0.3) return rgb(50, 52, 56);
  if (y > 1.4 && abs(x + 1.6) < 0.85 && z > 1.5) return rgb(40, 40, 42);
  if (length2(x - 2.6, z - 2.0) < 0.5) return y > 0.6 ? rgb(60, 90, 110) : rgb(110, 80, 50);
  return mix(rgb(70, 56, 44), rgb(90, 72, 56), noise3(x * 3, y * 12, z * 3));""",
  ['{ kind: "light", at: [1.6, 1.0, 2.1] }'])

# ------------------------------------------------------------- the beach
item("beach-ball", "beach ball", "A big beach ball striped red, yellow, blue and white, light and bouncy.", (0.25, 0.5, 0.25), "{ mass: 0.15, toy: 1, bounce: 0.85 }",
     """  return sphere(x, y - 0.24, z, 0.24);""",
     """  const a = atan2(z, x);
  const s = floor(fract(a / 6.2832 + 0.5) * 6.0);
  if (abs(y - 0.24) > 0.22) return rgb(250, 250, 246);
  if (s < 1 || s == 3) return rgb(220, 40, 40);
  if (s == 1 || s == 4) return rgb(250, 210, 40);
  return rgb(40, 100, 200);""", tags='["item", "toy", "ball"]')
t("adirondack-chair", "A wooden Adirondack chair, wide-armed and low, its slatted back leaning far\nback, painted barn red, sea green or white, for sitting on the sand and\nwatching the sun go down. It faces +z.",
  "adirondack chair", (0.42, 1.0, 0.55), '["furniture", "seat"]', "{ mass: 15, burns: 0.6 }",
  """  const seat = max(box(x, y - 0.36 + z * 0.15, z, 0.3, 0.025, 0.28), 0.0 - 1.0);
  const sx = x - 0.12 * floor(x / 0.12 + 0.5);
  const back = max(max(box(sx, y - 0.62, z + 0.3 + (y - 0.35) * 0.5, 0.045, 0.32, 0.02), abs(x) - 0.3), (y - 0.95) + abs(x) * 0.4);
  const arms = box(abs(x) - 0.36, y - 0.55, z - 0.02, 0.06, 0.02, 0.32);
  const legs = box(abs(x) - 0.33, y - 0.27, abs(z) - 0.26, 0.025, 0.27, 0.025);
  return min(min(seat, back), min(arms, legs));""",
  """  const c = floor(hash(k.seed) * 3.0);
  const n = 0.92 + 0.08 * noise3(x * 8, y * 8, z * 8);
  if (c < 1) return rgb(160, 50, 40) * n;
  if (c < 2) return rgb(80, 150, 130) * n;
  return rgb(240, 238, 230) * n;""",
  ['{ kind: "seat", at: [0, 0.38, -0.05], face: 0 }'])
t("picnic-blanket", "A red-and-white checked picnic blanket spread on the sand, a wicker basket\non it with a loaf, apples and a thermos of cider.",
  "picnic blanket", (1.0, 0.35, 0.8), '["prop", "seat", "nonsolid"]', "{ mass: 4, burns: 0.8 }",
  """  const cloth = box(x, y - 0.01, z, 0.95, 0.01, 0.75);
  const basket = box(x - 0.5, y - 0.14, z + 0.35, 0.22, 0.13, 0.15);
  const handle = max(abs(length2(x - 0.5, y - 0.27) - 0.15) - 0.015, abs(z - 0.35) - 0.015);
  const apples = min(sphere(x + 0.3, y - 0.05, z - 0.2, 0.045), sphere(x + 0.2, y - 0.05, z - 0.3, 0.045));
  return min(min(cloth, basket), min(max(handle, 0.27 - y), apples));""",
  """  if (y > 0.03 && abs(x + 0.25) < 0.1) return rgb(196, 30, 30);
  if (y > 0.03) return y > 0.26 ? rgb(150, 110, 64) : (fract(y * 20.0) < 0.4 ? rgb(150, 110, 64) : rgb(196, 156, 100));
  return step(0.5, fract(x * 5.0)) + step(0.5, fract(z * 5.0)) == 1 ? rgb(200, 40, 40) : rgb(246, 242, 236);""",
  ['{ kind: "seat", at: [0.3, 0.05, 0.2], face: 0 }', '{ kind: "seat", at: [-0.3, 0.05, -0.2], face: 0 }'])
t("driftwood-log", "A long driftwood log, bleached silver-grey by the sea and smooth from\nyears of waves, washed up on the sand: a seat for three by the fire ring.",
  "driftwood log", (1.6, 0.5, 0.35), '["prop", "seat", "wood"]', "{ mass: 120, burns: 0.8, fuel: 0.8 }",
  """  const log = capsule(x, y, z, -1.4, 0.24, 0.0, 1.4, 0.2, 0.05, 0.22 - abs(x) * 0.03);
  const knot = capsule(x, y, z, 0.6, 0.3, 0.05, 0.85, 0.45, 0.25, 0.05);
  return min(log, knot);""",
  """  return mix(rgb(170, 164, 152), rgb(206, 200, 188), noise3(x * 3, y * 20, z * 20));""",
  ['{ kind: "seat", at: [-0.8, 0.44, 0], face: 0 }', '{ kind: "seat", at: [0, 0.44, 0], face: 0 }', '{ kind: "seat", at: [0.8, 0.44, 0], face: 0 }'])
t("quay-ladder", "A wooden ladder fixed to the granite face of the landing, its rungs green\nwith weed below the tide line, its rails rising over the edge for a hand\nto hold: the way up out of the water. It faces +z, out to sea.",
  "quay ladder", (0.32, 4.0, 0.12), '["ladder", "prop"]', "{ mass: 40, burns: 0.5 }",
  """  const rails = box(abs(x) - 0.25, y - 1.95, z, 0.04, 1.95, 0.04);
  const ry = y - 0.3 * floor(y / 0.3) - 0.15;
  const rungs = max(max(box(x, ry, z, 0.25, 0.022, 0.025), y - 3.0), 0.05 - y);
  return min(rails, rungs);""",
  """  if (y < 1.3) return mix(rgb(54, 76, 44), rgb(80, 96, 56), noise3(x * 8, y * 8, z * 8));
  return mix(rgb(120, 100, 76), rgb(146, 124, 96), noise3(x * 8, y * 20, z * 8));""",
  ['{ kind: "stairs", at: [0, 0, 0.5], to: [0, 3.0, -0.5], face: 180 }'])

# ------------------------------------------------------------- autumn
t("leaf-pile", "A heap of raked-up autumn leaves, red, orange, gold and brown, the kind\nchildren jump into; a rake leaning on it.",
  "leaf pile", (1.3, 0.75, 1.2), '["prop", "leaves", "nonsolid"]', "{ mass: 15, burns: 0.95, fuel: 0.3 }",
  """  const heap = ellipsoid(x, y - 0.05, z, 1.2, 0.6, 1.05) + (noise3(x * 3, y * 3, z * 3) - 0.5) * 0.25;
  const rake = capsule(x, y, z, 1.0, 0.0, 0.4, 0.55, 1.35, 0.1, 0.02);
  return min(max(heap * 0.8, -y), max(rake, y - 0.74));""",
  """  if (length2(x - 0.8, z - 0.3) < 0.35 && y > 0.3) return rgb(150, 110, 70);
  const g = hash(floor(x * 9.0), floor(y * 9.0) * 13 + floor(z * 9.0));
  return hsv(0.0 + g * 0.12, 0.8, 0.45 + 0.35 * noise3(x * 12, y * 12, z * 12));""")
t("harvest-table", "A long trestle table for the harvest supper on the green, laid with a\nwhite cloth, pumpkins and gourds down its middle, apple pies, jugs of\ncider and loaves of bread, benches along both sides. Long along x.",
  "harvest table", (2.6, 1.0, 0.95), '["furniture", "table"]', "{ mass: 120, burns: 0.6 }",
  """  const top = box(x, y - 0.76, z, 2.5, 0.03, 0.45);
  const legs = box(abs(x) - 2.2, y - 0.38, z, 0.06, 0.38, 0.38);
  const benches = box(x, y - 0.44, abs(z) - 0.75, 2.4, 0.03, 0.15);
  const blegs = box(abs(x) - 2.1, y - 0.22, abs(z) - 0.75, 0.05, 0.22, 0.12);
  const px = x - 1.1 * floor(x / 1.1 + 0.5);
  const pumpkins = max(length3(px, (y - 0.88) * 1.3, z) - 0.11, abs(x) - 2.3);
  const pies = max(cylinder(px - 0.45, y - 0.8, z - 0.24, 0.13, 0.02), abs(x) - 2.3);
  const jugs = max(cappedCone(px + 0.45, y - 0.88, z + 0.22, 0.09, 0.06, 0.04), abs(x) - 2.3);
  return min(min(min(top, legs), min(benches, blegs)), min(pumpkins, min(pies, jugs)));""",
  f"""  if (y > 0.79 && length2(x - 1.1 * floor(x / 1.1 + 0.5), z) < 0.13) return rgb(222, 110, 24);
  if (y > 0.79 && z > 0.1) return rgb(200, 140, 70);
  if (y > 0.79 && z < -0.1) return rgb(196, 150, 90);
  if (abs(y - 0.76) < 0.035 && abs(z) < 0.46) return rgb(246, 244, 236);
  return {WOOD};""",
  ['{ kind: "seat", at: [-1.6, 0.46, 0.75], face: 180 }', '{ kind: "seat", at: [0, 0.46, 0.75], face: 180 }', '{ kind: "seat", at: [1.6, 0.46, 0.75], face: 180 }',
   '{ kind: "seat", at: [-1.6, 0.46, -0.75], face: 0 }', '{ kind: "seat", at: [0, 0.46, -0.75], face: 0 }', '{ kind: "seat", at: [1.6, 0.46, -0.75], face: 0 }'])
t("scarecrow", "A scarecrow on a pole over the hay: a straw-stuffed plaid shirt and patched\noveralls, a burlap face with a stitched smile, a battered straw hat.",
  "scarecrow", (0.8, 2.1, 0.25), '["prop", "decoration"]', "{ mass: 10, burns: 0.95 }",
  """  const pole = box(x, y - 0.9, z, 0.04, 0.9, 0.04);
  const arms = capsule(x, y, z, -0.7, 1.45, 0, 0.7, 1.45, 0, 0.07);
  const body = roundBox(x, y - 1.2, z, 0.22, 0.32, 0.12, 0.08);
  const head = sphere(x, y - 1.7, z, 0.15);
  const hat = min(cylinder(x, y - 1.82, z, 0.3, 0.015), cylinder(x, y - 1.9, z, 0.14, 0.08));
  return min(min(pole, arms), min(min(body, head), hat));""",
  """  if (y > 1.8) return rgb(214, 186, 110);
  if (y > 1.55) return abs(y - 1.66) < 0.012 && z > 0.12 ? rgb(60, 40, 30) : rgb(196, 170, 120);
  if (y > 1.33) return step(0.5, fract(x * 6.0)) + step(0.5, fract(y * 6.0)) == 1 ? rgb(170, 40, 34) : rgb(60, 70, 120);
  if (y > 0.88) return rgb(70, 96, 140);
  return rgb(120, 100, 70);""")
t("fallen-leaves", "A drift of fallen leaves on the ground, red maple, gold birch and brown\noak, blown into a loose patch.",
  "fallen leaves", (1.7, 0.06, 1.7), '["prop", "leaves", "nonsolid"]', "{ mass: 1, burns: 0.95 }",
  """  const r = length2(x, z) - 1.0 - (noise3(x * 1.4, k.seed * 0.01, z * 1.4) - 0.5) * 1.2;
  return max(box(x, y - 0.015, z, 1.65, 0.015, 1.65), r);""",
  """  const g = hash(floor(x * 8.0), floor(z * 8.0), k.seed);
  if (g < 0.15) return rgb(110, 76, 40);
  return hsv(g * 0.13, 0.8, 0.5 + 0.3 * hash(floor(z * 8.0), floor(x * 8.0)));""")
t("easel", "An artist's easel set up on the sand facing the cove, a half-finished\npainting on it: the sunset over the water, the sky and sun done in reds and\ngolds, the sea and the pier still bare pencil on white canvas. A folding\nstool before it with a palette of paint blobs and a jar of brushes. The\npainting faces +z, where the painter sits; beyond it, the view.",
  "easel", (0.55, 1.75, 0.95), '["furniture", "art", "seat"]', "{ mass: 8, burns: 0.8 }",
  """  const legs = min(capsule(abs(x), y, z, 0.32, 0.0, 0.1, 0.05, 1.7, 0.0, 0.022), capsule(x, y, z, 0, 0.0, -0.45, 0, 1.6, -0.05, 0.02));
  const ledge = box(x, y - 0.72, z - 0.06, 0.38, 0.02, 0.05);
  const canvas = box(x, y - 1.08, z - 0.02 + (y - 1.08) * 0.08, 0.36, 0.34, 0.012);
  const stool = min(cylinder(x, y - 0.46, z - 0.8, 0.18, 0.025), box(abs(x) - 0.13, y - 0.22, abs(z - 0.8) - 0.0, 0.02, 0.22, 0.12));
  const palette = cylinder(x - 0.18, y - 0.495, z - 0.76, 0.1, 0.008);
  const jar = cappedCone(x + 0.4, y - 0.08, z - 0.5, 0.08, 0.05, 0.045);
  const brushes = capsule(x, y, z, -0.4, 0.1, 0.5, -0.38, 0.32, 0.52, 0.006);
  return min(min(min(legs, ledge), min(canvas, stool)), min(palette, min(jar, max(brushes, -1.0))));""",
  f"""  // The painting: the upper part done (sky, sun, its path on the water),
  // the lower still pencil lines on white.
  if (abs(x) < 0.355 && abs(y - 1.08) < 0.335 && z > 0.0 && abs(z - 0.02 - (y - 1.08) * 0.08) < 0.02) {{
    const u = x / 0.355;
    const v = (y - 1.08) / 0.335;
    if (v > -0.1) {{
      const sun = length2(u + 0.15, (v - 0.15) * 1.4);
      if (sun < 0.16) return rgb(255, 236, 170);
      return mix(rgb(250, 170, 80), rgb(200, 90, 110), clamp(v, 0, 1)) * (0.92 + 0.08 * noise3(x * 60, y * 60, 1.0));
    }}
    if (v > -0.3 && u < 0.2) return abs(u + 0.15) < 0.08 ? rgb(250, 200, 110) : rgb(60, 80, 120);
    if (abs(v + 0.35) < 0.012 || (abs(v + 0.6) < 0.01 && u > 0.1) || (abs(u - 0.5) < 0.01 && v < -0.35)) return rgb(120, 120, 120);
    return rgb(246, 244, 238);
  }}
  if (length2(x - 0.18, z - 0.76) < 0.1 && y > 0.49) return hsv(floor(atan2(z - 0.76, x - 0.18) * 1.6) * 0.17, 0.7, 0.9);
  if (abs(z - 0.8) < 0.2 && y > 0.43 && y < 0.49) return rgb(60, 110, 90);
  if (length2(x + 0.4, z - 0.5) < 0.09 && y < 0.17) return rgb(200, 220, 230);
  return {WOOD};""",
  ['{ kind: "seat", at: [0, 0.48, 0.8], face: 180 }'])


# ------------------------------------------------------------- detail pass
t("kitchen", "A cottage kitchen along a wall: cream-painted cupboards with brass knobs\nunder a butcher-block counter, a deep white sink, a black cast-iron stove\nwith a kettle and a pot of stew on it, a shelf of jars, crocks and blue\nplates above, copper pans hung under it. The cook stands at its front (+z).",
  "kitchen", (1.25, 1.95, 0.32), '["furniture", "kitchen", "oven"]', "{ mass: 200, burns: 0.3, heat: 60 }",
  """  const counter = box(x - 0.35, y - 0.45, z, 0.88, 0.45, 0.3);
  const top = box(x - 0.35, y - 0.915, z + 0.01, 0.9, 0.035, 0.31);
  const sink = box(x - 0.75, y - 0.86, z + 0.02, 0.26, 0.1, 0.2);
  const stove = roundBox(x - 0.89, y - 0.42, z, 0.36, 0.42, 0.28, 0.03);
  const flue = cylinder(x - 0.89, y - 1.5, z + 0.18, 0.05, 0.65);
  const kettle = min(ellipsoid(x - 0.78, y - 0.95, z - 0.02, 0.09, 0.07, 0.09), capsule(x, y, z, 0.86, 0.94, -0.02, 0.95, 1.0, -0.02, 0.012));
  const pot = cylinder(x - 1.02, y - 0.94, z + 0.05, 0.1, 0.09);
  const knobs = max(sphere(x - 0.3 * floor(x / 0.3 + 0.5) - 0.1, y - 0.62, z - 0.31, 0.018), x - 0.5);
  const shelf = box(x - 0.2, y - 1.55, z + 0.14, 0.95, 0.02, 0.14);
  const jars = max(cylinder(x - 0.22 * floor(x / 0.22 + 0.5), y - 1.65, z + 0.13, 0.055 + 0.02 * hash(floor(x / 0.22 + 0.5)), 0.08), abs(x + 0.2) - 0.9);
  const pans = max(cylinder(x - 0.3 * floor(x / 0.3 + 0.5), y - 1.3, z + 0.27, 0.09, 0.012), abs(x + 0.3) - 0.5);
  return min(min(min(max(min(counter, top), -sink), stove), min(flue, min(kettle, pot))), min(min(knobs, shelf), min(jars, pans)));""",
  """  if (x > 0.5 && y < 1.05 && y > 0.86) return abs(x - 1.02) < 0.11 && y > 0.98 ? rgb(130, 70, 40) : rgb(50, 50, 54);
  if (x > 0.5 && y < 0.86) return abs(y - 0.42) < 0.2 && z > 0.27 && abs(x - 0.89) < 0.22 ? glow(rgb(255, 120, 40), 0.4) : rgb(36, 36, 40);
  if (abs(x - 0.89) < 0.06 && y > 0.86) return rgb(40, 40, 44);
  if (y > 1.55) return hsv(fract(floor(x / 0.22 + 0.5) * 0.31), 0.45, 0.75);
  if (y > 1.52) return rgb(130, 90, 56);
  if (y > 1.2 && y < 1.4) return rgb(196, 110, 60);
  if (y > 0.88 && y < 0.96 && abs(x + 0.75) < 0.27 && abs(z) < 0.22) return rgb(236, 238, 240);
  if (y > 0.88 && y < 0.96) return mix(rgb(176, 130, 84), rgb(200, 156, 104), noise3(x * 3, y * 30, z * 30));
  if (abs(y - 0.62) < 0.03 && z > 0.29) return rgb(200, 170, 70);
  if (z > 0.28 && (fract((x + 0.35) / 0.44) < 0.04 || abs(y - 0.7) < 0.015)) return rgb(200, 190, 166);
  return rgb(236, 226, 196);""",
  ['{ kind: "light", at: [0, 1.8, 0.2] }'])

t("beer-barrels", "Three oak barrels on their sides on a rack, iron hoops round them and a\nbrass spigot in each head, chalked AMBER, STOUT and CIDER: the tavern's\nale, its dark stout and the autumn cider.",
  "beer barrels", (1.3, 1.2, 0.52), '["furniture", "prop"]', "{ mass: 300, burns: 0.5, fuel: 0.4 }",
  """  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  const bulge = 0.38 + 0.05 * (1.0 - (z / 0.45) * (z / 0.45));
  const barrels = max(max(length2(bx, y - 0.62) - bulge, abs(z) - 0.45), abs(x) - 1.25);
  const heads = max(max(length2(bx, y - 0.62) - 0.34, abs(z - 0.43) - 0.02), abs(x) - 1.25);
  const spigots = max(min(capsule(bx, y, z, 0, 0.5, 0.44, 0, 0.5, 0.56, 0.025), capsule(bx, y, z, 0, 0.5, 0.56, 0, 0.44, 0.58, 0.02)), abs(x) - 1.25);
  const rack = min(box(x, y - 0.1, abs(z) - 0.3, 1.25, 0.1, 0.06), box(abs(x - 0.85 * floor(x / 0.85 + 0.5)) - 0.3, y - 0.2, z, 0.06, 0.06, 0.4));
  return min(min(barrels, heads), min(spigots, rack));""",
  f"""  const bx = x - 0.85 * floor(x / 0.85 + 0.5);
  const r = length2(bx, y - 0.62);
  if (z > 0.43 && r < 0.35) {{
    if (abs(y - 0.5) < 0.04 && abs(bx) < 0.04) return rgb(200, 160, 60);
    if (abs(y - 0.72) < 0.05 && abs(bx) < 0.2) return rgb(236, 232, 220);
    return (fract(bx * 9.0) < 0.08 ? 0.7 : 1.0) * mix(rgb(150, 108, 66), rgb(176, 130, 80), noise3(bx * 8, y * 8, 1.0));
  }}
  if (z > 0.44 && y < 0.6) return rgb(200, 160, 60);
  if (abs(abs(z) - 0.12) < 0.03 || abs(abs(z) - 0.38) < 0.025) return rgb(60, 60, 62);
  return (fract(atan2(y - 0.62, bx) * 3.2) < 0.06 ? 0.75 : 1.0) * {WOOD};""")

item("book", "book", "A cloth-bound book of stories, its corners worn, gold letters on its spine\nand a red ribbon marking the page.", (0.12, 0.035, 0.09), "{ mass: 0.5, burns: 0.9 }",
     """  const cover = max(box(x, y - 0.0275, z, 0.11, 0.0275, 0.085), -box(x + 0.012, y - 0.0275, z, 0.1, 0.02, 0.08));
  const pages = box(x + 0.006, y - 0.0275, z, 0.1, 0.019, 0.078);
  const spine = capsule(x, y, z, -0.108, 0.0275, -0.085, -0.108, 0.0275, 0.085, 0.026);
  const ribbon = box(x - 0.04, y - 0.01, z - 0.09, 0.006, 0.01, 0.012);
  return min(min(cover, pages), min(max(spine, x + 0.098), ribbon));""",
     """  if (abs(x - 0.04 + 0.08) < 0.008 && z > 0.07) return rgb(180, 30, 30);
  if (x > -0.1 && abs(y - 0.0275) < 0.019 && (abs(z) > 0.076 || x > 0.1)) return fract(y * 300.0) < 0.5 ? rgb(236, 228, 206) : rgb(214, 204, 180);
  if (x < -0.09 && abs(abs(z) - 0.04) < 0.008) return rgb(210, 176, 80);
  return hsv(hash(k.seed), 0.55, 0.42) * (0.9 + 0.1 * noise3(x * 50, y * 50, z * 50));""")

item("bunch-of-flowers", "bunch of flowers", "A bunch of garden flowers wrapped in brown paper and tied with string: rust\nand gold mums, purple asters, a few sprigs of goldenrod.", (0.12, 0.32, 0.12), "{ mass: 0.3, alive: 1, burns: 0.6 }",
     """  const paper = max(max(cappedCone(x, y - 0.11, z, 0.11, 0.025, 0.08), -cappedCone(x, y - 0.13, z, 0.1, 0.015, 0.07)), y - 0.22);
  let d = paper;
  for (let i = 0; i < 7; i++) {
    const a = i * 2.39996;
    const r = 0.015 + (i % 3) * 0.025;
    const hx = cos(a) * r;
    const hz = sin(a) * r;
    const hy = 0.24 + hash(i, 2) * 0.05;
    d = min(d, capsule(x, y, z, 0, 0.04, 0, hx, hy, hz, 0.005));
    d = min(d, ellipsoid(x - hx, y - hy, z - hz, 0.032, 0.022, 0.032));
  }
  return d;""",
     """  if (y < 0.215) return abs(y - 0.12) < 0.006 ? rgb(230, 220, 200) : mix(rgb(176, 140, 96), rgb(196, 160, 112), noise3(x * 40, y * 40, z * 40));
  const a = atan2(z, x);
  const i = floor(fract(a / 6.2832 * 2.0 + y * 3.0) * 4.0);
  if (y < 0.235 && length2(x, z) < 0.05) return rgb(70, 110, 50);
  const petal = 0.85 + 0.15 * step(0.5, fract(atan2(z, x) * 4.0));
  if (i < 1) return rgb(196, 90, 30) * petal;
  if (i < 2) return rgb(236, 180, 40) * petal;
  if (i < 3) return rgb(140, 80, 190) * petal;
  return rgb(170, 40, 50) * petal;""")

t("fish-counter", "The fish counter in the fish shed: a sloping bed of crushed ice on a stand\nof weathered boards, laid with rows of silver mackerel and cod, two red\nlobsters and a heap of clams at one end, a slate with the day's prices.",
  "fish counter", (1.15, 1.25, 0.45), '["furniture", "counter", "shop"]', "{ mass: 140, wet: 0.5 }",
  """  const stand = box(x, y - 0.4, z, 1.1, 0.4, 0.4);
  const tray = max(box(x, y - 0.84, z, 1.12, 0.06, 0.42), -box(x, y - 0.9, z, 1.06, 0.06, 0.36));
  const ice = box(x, y - 0.83, z, 1.06, 0.04, 0.36) + (noise3(x * 30, y * 30, z * 30) - 0.5) * 0.01;
""" + fish_rows(0.14, 0.32, 0.9) + """
  const fishes = max(fish, max(max(x - 0.55, -x - 1.0), abs(z) - 0.34));
  const lob = ellipsoid(x - 0.68, y - 0.9, abs(z) - 0.16, 0.06, 0.04, 0.14);
  const clams = max(ellipsoid(x - 0.95, y - 0.88, z, 0.12, 0.06, 0.3) + (noise3(x * 30, y * 30, z * 30) - 0.5) * 0.03, -y + 0.86);
  const slate = box(x + 0.9, y - 1.05, z + 0.38, 0.18, 0.13, 0.01);
  return min(min(min(stand, tray), min(ice, fishes)), min(min(lob, clams), slate));""",
  fish_col(0.14, 0.32, 0.9) + """
  if (abs(x + 0.9) < 0.18 && y > 0.92) return abs(y - 1.05) < 0.08 && fract(x * 30.0) < 0.5 && fract(y * 40.0) < 0.4 ? rgb(230, 230, 220) : rgb(50, 54, 58);
  if (abs(x - 0.68) < 0.09 && y > 0.86) return rgb(200, 50, 36);
  if (x > 0.83 && y > 0.86) return noise3(x * 40, y * 40, z * 40) > 0.5 ? rgb(170, 160, 150) : rgb(110, 104, 100);
  if (y > 0.8 && abs(x) < 1.07 && abs(z) < 0.37) return mix(rgb(220, 232, 236), rgb(250, 252, 252), noise3(x * 40, y * 40, z * 40));
  if (y > 0.78) return rgb(120, 116, 110);
  return (fract(x * 4.0) < 0.05 ? 0.75 : 1.0) * mix(rgb(140, 132, 120), rgb(168, 160, 146), noise3(x * 3, y * 20, z * 3));""")

t("fish-crates", "A stack of wooden fish crates on the landing, slatted and weathered, the\nboat's name stencilled on their ends, the top one full of silver fish on ice.",
  "fish crates", (0.75, 0.9, 0.52), '["prop"]', "{ mass: 40, burns: 0.6 }",
  """  const s = hash(k.seed) * 0.2;
  const a = max(box(x + 0.36, y - 0.15, z, 0.34, 0.15, 0.24), -box(x + 0.36, y - 0.2, z, 0.31, 0.12, 0.21));
  const b = max(box(x + 0.34, y - 0.45, z + 0.03, 0.34, 0.15, 0.24), -box(x + 0.34, y - 0.5, z + 0.03, 0.31, 0.12, 0.21));
  const c = max(box(x - 0.36, y - 0.15, z - 0.06 + s, 0.34, 0.15, 0.24), -box(x - 0.36, y - 0.2, z - 0.06 + s, 0.31, 0.12, 0.21));
  const ice = box(x + 0.34, y - 0.52, z + 0.03, 0.31, 0.04, 0.21);
  const iceb = box(x + 0.36, y - 0.22, z, 0.31, 0.04, 0.21);
""" + fish_rows(0.1, 0.4, 0.58, 0.07, 0.09) + """
  const fishes = max(fish, box(x + 0.34, y - 0.58, z + 0.03, 0.3, 0.06, 0.2));
  return min(min(min(a, b), min(c, ice)), min(iceb, fishes));""",
  fish_col(0.1, 0.4, 0.58, 0.07, 0.09) + """
  if (y > 0.47 && y < 0.57 && x > 0.02 && abs(z - 0.03) < 0.215) return mix(rgb(220, 232, 236), rgb(250, 252, 252), noise3(x * 40, y * 40, z * 40));
  if (abs(z) > 0.23 && abs(y - 0.15 - 0.3 * floor(y / 0.3)) < 0.04 && fract(x * 12.0) < 0.6) return rgb(60, 70, 100);
  if (fract(y / 0.3 * 3.0) < 0.12) return rgb(90, 70, 50);
  return mix(rgb(150, 132, 104), rgb(180, 160, 128), noise3(x * 6, y * 20, z * 6));""")

item("fiddle", "fiddle", "A well-worn fiddle of honey-coloured maple: a waisted body with two f-holes,\na dark fingerboard and four strings over its bridge, a scroll at the head,\nits bow beside it. For the dances at the tavern.", (0.13, 0.07, 0.34), "{ mass: 0.5, fragile: 0.6, burns: 0.8 }",
     """  const lower = ellipsoid(x, y - 0.035, z + 0.1, 0.105, 0.032, 0.095);
  const upper = ellipsoid(x, y - 0.035, z - 0.05, 0.085, 0.032, 0.08);
  const waist = ellipsoid(abs(x) - 0.105, y - 0.035, z - 0.025, 0.035, 0.05, 0.04);
  const body = smoothSubtract(smoothUnion(lower, upper, 0.03), waist, 0.01);
  const neck = box(x, y - 0.055, z + 0.19, 0.016, 0.012, 0.08);
  const scroll = max(abs(length2(x, z + 0.3) - 0.018) - 0.008, abs(y - 0.06) - 0.015);
  const pegs = box(x, y - 0.06, z + 0.265, 0.03, 0.006, 0.006);
  const bridge = box(x, y - 0.075, z - 0.06 + 0.08, 0.04, 0.008, 0.004);
  const bow = capsule(x, y, z, 0.12, 0.012, -0.32, 0.12, 0.012, 0.3, 0.006);
  return min(min(min(body, neck), min(scroll, pegs)), min(bridge, bow));""",
     """  if (x > 0.108) return abs(z) > 0.28 ? rgb(40, 30, 26) : rgb(236, 230, 210);
  if (z < -0.11 || (y > 0.064 && abs(x) < 0.018 && z < 0.07)) return z < -0.26 ? rgb(150, 70, 30) : rgb(30, 26, 24);
  if (y > 0.06 && abs(x) < 0.02 && abs(fract(x * 80.0) - 0.5) < 0.12) return rgb(220, 220, 210);
  if (y > 0.06 && abs(abs(x) - 0.045) < 0.006 && abs(z - 0.03) < 0.035) return rgb(30, 20, 16);
  if (abs(z - 0.02) < 0.006 && y > 0.065) return rgb(236, 214, 170);
  return mix(rgb(150, 70, 26), rgb(206, 120, 44), clamp(1.0 - length2(x * 1.4, z - 0.03) * 6.0, 0, 1)) * (0.92 + 0.08 * noise3(x * 30, y * 30, z * 8));""", tags='["item", "instrument"]')

t("hay-bale", "A round bale of golden hay, its straw wound round and round in a tight\nspiral, loose wisps at its edges and bound with twine; a pumpkin or two\nset against it.",
  "hay bale", (0.75, 1.2, 0.88), '["prop"]', "{ mass: 250, burns: 0.95, fuel: 0.5 }",
  """  // The bale lies on its side, its round faces to ±z.
  const body = max(length2(x, y - 0.6) - 0.6, abs(z) - 0.55) + noise3(x * 9, y * 9, z * 9) * 0.035;
""" + pumpkin_js("p", 0.32, 0.17, 0.72 - 0.05, 0.18) + """
  return min(body * 0.85, min(p, ps));""",
  pumpkin_col("p", 0.32, 0.17, 0.67, 0.18) + """
  const r = length2(x, y - 0.6);
  if (abs(z) > 0.53) return (fract(r * 22.0 + atan2(y - 0.6, x) * 0.16) < 0.35 ? 0.78 : 1.0) * mix(rgb(196, 160, 80), rgb(228, 198, 116), noise3(x * 20, y * 20, z * 20));
  if (abs(abs(z) - 0.25) < 0.015) return rgb(170, 140, 90);
  return (fract(atan2(y - 0.6, x) * 9.0 + noise3(x * 4, y * 4, z * 30) * 0.5) < 0.3 ? 0.82 : 1.0) * mix(rgb(196, 160, 80), rgb(226, 194, 112), noise3(x * 30, y * 6, z * 30));""")

t("tree-stump", "An old stump left from a felled maple, its top sawn flat, grey and ringed\nwith the years, its bark rough, roots spreading into the ground with moss\nbetween them; a handy seat.",
  "tree stump", (0.6, 0.55, 0.6), '["prop", "seat", "wood"]', "{ mass: 300, burns: 0.5 }",
  """  const a = atan2(z, x);
  const rough = 0.3 + 0.02 * sin(a * 9.0) + noise3(x * 8, y * 4, z * 8) * 0.03;
  const trunk = max(length2(x, z) - rough - max(0.12 - y, 0.0) * 0.8, max(y - 0.5 + noise3(x * 5, 0, z * 5) * 0.02, -y));
  let roots = 10.0;
  for (let i = 0; i < 5; i++) {
    const ra = i * 1.2566 + 0.3;
    roots = min(roots, capsule(x, y, z, cos(ra) * 0.2, 0.12, sin(ra) * 0.2, cos(ra) * 0.55, -0.02, sin(ra) * 0.55, 0.06));
  }
  return min(trunk * 0.8, roots);""",
  """  if (y > 0.47) return (fract(length2(x, z) * 28.0 + noise3(x * 6, 0, z * 6) * 0.4) < 0.25 ? rgb(140, 120, 92) : rgb(190, 172, 140)) * (length2(x, z) > 0.28 ? 0.6 : 1.0);
  if (y < 0.05 && noise3(x * 10, y * 10, z * 10) > 0.62) return mix(rgb(70, 100, 46), rgb(100, 130, 60), noise3(x * 30, y * 30, z * 30));
  return (fract(atan2(z, x) * 4.0 + noise3(x * 3, y * 6, z * 3)) < 0.2 ? 0.7 : 1.0) * mix(rgb(70, 58, 46), rgb(100, 84, 66), noise3(x * 8, y * 8, z * 8));""",
  ['{ kind: "seat", at: [0, 0.5, 0], face: 0 }'])

t("seashells", "Seashells strewn on the sand: blue-black mussel shells, ribbed scallops\nin white, pink and apricot, a spiralled whelk, a round moon snail, a\nbleached sand dollar, washed up along the tide line.",
  "seashells", (0.62, 0.07, 0.62), '["prop", "shell", "nonsolid"]', "{ mass: 0.3, fragile: 0.5 }",
  """  let d = 10.0;
  for (let i = 0; i < 6; i++) {
    const a = i * 2.39996 + hash(k.seed, i) * 1.3;
    const r = 0.08 + hash(k.seed, i, 2) * 0.48;
    const q = rotY(x - cos(a) * r, y, z - sin(a) * r, a * 3.0);
    const kind = i % 3;
    let sh = 10.0;
    if (kind == 0) sh = max(ellipsoid(q.x, q.y - 0.006, q.z, 0.05, 0.022, 0.048), max(-q.y, q.z * 0.6 - 0.03));
    if (kind == 1) sh = ellipsoid(q.x, q.y - 0.008, q.z, 0.022, 0.014, 0.045);
    if (kind == 2) sh = min(cappedCone(q.x, q.y - 0.018, q.z, 0.018, 0.024, 0.004), sphere(q.x, q.y - 0.012, q.z, 0.02));
    d = min(d, sh);
  }
  return d;""",
  """  const cell = floor(hash(floor(x * 5.0 + 50.0), floor(z * 5.0 + 50.0), k.seed) * 4.0);
  const ribs = fract(atan2(z, x) * 5.0 + length2(x, z) * 30.0) < 0.3 ? 0.82 : 1.0;
  if (y < 0.004) return rgb(214, 200, 166);
  if (cell < 1) return mix(rgb(30, 34, 60), rgb(70, 90, 130), noise3(x * 60, y * 60, z * 60));
  if (cell < 2) return rgb(244, 214, 200) * ribs;
  if (cell < 3) return mix(rgb(236, 190, 140), rgb(250, 236, 220), fract(length2(x, z) * 60.0)) * ribs;
  return rgb(244, 240, 228) * ribs;""")
item("basket-of-fish", "basket of fish", "A woven basket of the morning's catch: silver mackerel and a cod or two\nlaid head to tail, a handful of seaweed over them.", (0.28, 0.24, 0.28), "{ mass: 6, edible: 0.7, wet: 0.6, burns: 0.4 }",
     """  const basket = max(cappedCone(x, y - 0.1, z, 0.1, 0.2, 0.26), -cappedCone(x, y - 0.14, z, 0.1, 0.17, 0.23));
  const rim = max(abs(length2(x, z) - 0.255) - 0.018, abs(y - 0.2) - 0.018);
""" + fish_rows(0.07, 0.22, 0.18, 0.08, 0.1) + """
  const fishes = max(fish, length2(x, z) - 0.22);
  const weed = max(ellipsoid(x + 0.08, y - 0.2, z - 0.05, 0.08, 0.02, 0.06), -y + 0.18);
  return min(min(basket, rim), min(fishes, weed));""",
     fish_col(0.07, 0.22, 0.18, 0.08, 0.1) + """
  if (y > 0.19 && length2(x + 0.08, z - 0.05) < 0.08) return rgb(60, 90, 40);
  if (abs(length2(x, z) - 0.255) < 0.025 && y > 0.18) return rgb(120, 86, 50);
  return (fract(y * 30.0) < 0.4 != fract(atan2(z, x) * 6.0) < 0.5) ? rgb(130, 96, 56) : rgb(176, 136, 80);""".replace("(fract(y * 30.0) < 0.4 != fract(atan2(z, x) * 6.0) < 0.5)", "step(0.4, fract(y * 30.0)) + step(0.5, fract(atan2(z, x) * 6.0)) == 1"), tags='["item", "food"]')
item("fishing-net", "fishing net", "A fisherman's net bundled up for carrying: folds of green mesh with a line\nof orange cork floats along one edge and a coil of rope tied round it.", (0.42, 0.24, 0.32), "{ mass: 3, burns: 0.7 }",
     """  const heap = (ellipsoid(x, y - 0.04, z, 0.38, 0.17, 0.28) + (noise3(x * 6, y * 6, z * 6) - 0.5) * 0.08) * 0.8;
  const fx = x - 0.12 * floor(x / 0.12 + 0.5);
  const floats = max(ellipsoid(fx, y - 0.14 + x * 0.1, z - 0.2 + abs(x) * 0.15, 0.04, 0.03, 0.03), abs(x) - 0.34);
  const rope = max(abs(length2(x - 0.1, y - 0.04) - 0.15) - 0.014, abs(z) - 0.022);
  return min(max(heap, -y), min(floats, max(rope, -y)));""",
     """  if (abs(length2(x - 0.1, y - 0.04) - 0.15) < 0.02 && abs(z) < 0.03) return fract(atan2(y - 0.05, x - 0.1) * 6.0) < 0.5 ? rgb(196, 170, 120) : rgb(170, 146, 100);
  if (abs(z - 0.2 + abs(x) * 0.15) < 0.04 && y > 0.1) return rgb(240, 130, 40);
  const m = step(0.82, fract(x * 18.0 + z * 18.0)) + step(0.82, fract(x * 18.0 - z * 18.0));
  return m > 0.0 ? rgb(90, 140, 100) : rgb(36, 70, 46) * (0.8 + 0.2 * noise3(x * 10, y * 10, z * 10));""", tags='["item", "tool"]')
t("pots-of-mums", "Clay pots of autumn mums by a doorway or on porch steps, two or three\ntogether: domes thick with small flowers in rust, gold and wine red over\ndark green leaves.",
  "pots of mums", (0.62, 0.66, 0.36), '["flower", "prop"]', "{ mass: 12, alive: 1, fragile: 0.5 }",
  """  const px = x - 0.42 * floor(x / 0.42 + 0.5);
  const pots = max(max(cappedCone(px, y - 0.15, z, 0.15, 0.1, 0.15), -cappedCone(px, y - 0.2, z, 0.12, 0.09, 0.13)), abs(x) - 0.58);
  const rim = max(max(abs(length2(px, z) - 0.15) - 0.02, abs(y - 0.29) - 0.025), abs(x) - 0.6);
  // The dome of blooms: small bumps, one flower to each cell.
  const fx = px - 0.05 * floor(px / 0.05 + 0.5);
  const fz = z - 0.05 * floor(z / 0.05 + 0.5);
  const dome = ellipsoid(px, y - 0.36, z, 0.2, 0.16, 0.2);
  const blooms = max(dome - 0.012 + length2(fx, fz) * 0.35, -dome - 0.04);
  return min(min(pots, rim), max(min(dome, blooms), abs(x) - 0.6));""",
  """  if (y < 0.31) return abs(y - 0.29) < 0.03 ? rgb(170, 84, 50) : mix(rgb(166, 82, 50), rgb(196, 104, 62), noise3(x * 8, y * 8, z * 8));
  const px = x - 0.42 * floor(x / 0.42 + 0.5);
  const fx = px - 0.05 * floor(px / 0.05 + 0.5);
  const fz = z - 0.05 * floor(z / 0.05 + 0.5);
  const pot = floor(x / 0.42 + 0.5 + k.seed);
  let c = rgb(200, 90, 30);
  if (fract(pot / 3.0) > 0.3) c = rgb(236, 176, 40);
  if (fract(pot / 3.0) > 0.6) c = rgb(150, 30, 50);
  if (y < 0.4 && length2(px, z) > 0.17) return rgb(46, 80, 40);
  const r = length2(fx, fz);
  if (r < 0.006) return c * 0.6;
  return c * (0.75 + 0.25 * step(0.5, fract(atan2(fz, fx) * 1.9))) * (0.85 + 0.15 * (1.0 - r / 0.035));""")
