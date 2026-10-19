"""The broadleaf trees of Maple Cove, written from one recipe: a trunk that
forks into limbs, a crown of separate leafy lobes on the limbs' ends with
sky between them, colour that wanders across the crown in patches and is
darker inside,.

    python3 worlds/starter/trees.py   (writes base/1-, 2-, 3-, 5-*.js)
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
NEWPART = '''  const ring = {ring};
  // Leaves on the ground: only near it is the ring's edge worth finding.
  const ground = y > 0.3 ? max(y - 0.024, ring) : max(cylinder(x, y - 0.012, z, {rmax:.2f} * s, 0.012), length2(x, z) - ({rin:.2f} + noise3(x * 0.9, 3.0, z * 0.9) * {rnoise:.2f}) * s);
  // Far from the crown, its bounding sphere is enough.
  const far = length3(x, y - {cy:.2f} * s, z) - {cr:.2f} * s;
  if (far > 0.5) return min(min(trunk, ground), far * 0.8);
  // Limbs forking from the trunk's top out to the lobes.
  let limbs = 10.0;
  for (let i = 0; i < {limbs}; i++) {{
    const a = turn + i * {step:.4f};
    limbs = min(limbs, capsule(x, y, z, 0, {lb:.2f} * s, 0, cos(a) * {lr:.2f} * s, {lt:.2f} * s, sin(a) * {lr:.2f} * s, {lw:.3f} * s));
  }}
  const wood = min(trunk, limbs);{extra_sdf}
'''


def tree(fname, name, desc, tag, bounds, trunk_h, trunk_r, limbs, lobes, top, lobe_r, spread, colour, bark, leaves=False, extra_sdf="", extra_color="", props="{ mass: 3500, burns: 0.5, alive: 1, fuel: 1 }"):
    """`limbs`: how many forks; `lobes`: how many leafy lobes round the
    crown (plus one on top at `top`); `spread`: how far out they reach;
    `colour`: JS returning the leaf colour from f (the stand's choice), n
    (fine noise) and m (a slow patch noise across the crown)."""
    cy = (trunk_h + top) / 2
    cr = max(spread + lobe_r * 1.25, (top - trunk_h) / 2 + lobe_r * 1.2)
    src = f"""// {desc.replace(chr(10), chr(10) + '// ')}
export const meta = {{
  name: "{name}",
  bounds: [{bounds[0]}, {bounds[1]}, {bounds[2]}],
  tags: ["{tag}"],
  props: {props},
}};

export function sdf(x, y, z, k) {{
  const s = 0.78 + hash(k.seed) * 0.26;
  const turn = hash(k.seed, 2) * 6.2832;
  const trunk = cappedCone(x, y - {trunk_h / 2} * s, z, {trunk_h / 2} * s, {trunk_r} * s, {trunk_r * 0.6:.3f} * s);
""" + NEWPART.format(ring=("length2(x, z) - " + f"{spread + lobe_r * 0.8:.2f}" + " * s") if leaves else "10.0", rmax=spread + lobe_r * 0.8, rin=spread * 0.7, rnoise=lobe_r * 1.2, cy=cy, cr=cr, limbs=limbs, step=6.2832 / limbs, lb=trunk_h * 0.85, lr=spread * 0.75, lt=trunk_h + (top - trunk_h) * 0.45, lw=trunk_r * 0.45, extra_sdf=extra_sdf) + f"""  let crown = sphere(x, y - {top} * s, z, {lobe_r * 1.1:.2f} * s);
  for (let i = 0; i < {lobes}; i++) {{
    const a = turn + i * 2.39996 + hash(k.seed, i) * 0.6;
    const r = ({spread * 0.6:.2f} + hash(k.seed, i, 3) * {spread * 0.5:.2f}) * s;
    const ly = ({trunk_h + (top - trunk_h) * 0.35:.2f} + hash(k.seed, i, 4) * {(top - trunk_h) * 0.5:.2f}) * s;
    crown = min(crown, sphere(x - cos(a) * r, y - ly, z - sin(a) * r, ({lobe_r * 0.8:.2f} + hash(k.seed, i, 5) * {lobe_r * 0.4:.2f}) * s));
  }}
  // Leafy edges and holes: a broad wobble and a fine one.
  const leafy = crown + (noise3(x * 1.3, y * 1.3, z * 1.3) - 0.5) * 0.9 + (noise3(x * 3.7, y * 3.7, z * 3.7) - 0.5) * 0.35;
  return min(min(wood, ground), leafy * 0.7);
}}

export function color(x, y, z, k) {{
  const s = 0.78 + hash(k.seed) * 0.26;
  let f = k.a;
  if (hash(k.seed, 7) < 0.25) f = hash(k.seed, 8);
  const n = noise3(x * 2.6, y * 2.6, z * 2.6);
  const m = noise3(x * 0.45 + k.seed * 0.01, y * 0.45, z * 0.45);
  // The fallen leaves on the ground.
  if (y < 0.06) {{
    const g = hash(floor(x * 7.0), floor(z * 7.0), k.seed);
    return mix({colour}, rgb(120, 84, 44), 0.35 * g) * (0.7 + 0.3 * g);
  }}
  // Bark on the trunk and the limbs low in the crown.
  if (y < {trunk_h * 1.05:.2f} * s && length2(x, z) < {trunk_r * 1.6:.2f} * s) return {bark};
  if (y < {trunk_h + (top - trunk_h) * 0.3:.2f} * s && n < 0.45 && length2(x, z) < {spread * 0.8:.2f} * s) return {bark};{extra_color}
  // Deeper in the crown, darker; outside, the sun on it.
  const out = clamp(length3(x, (y - {cy:.2f} * s) * 1.2, z) / ({cr * 0.85:.2f} * s), 0, 1);
  const spec = hash(floor(x * 5.0), floor(y * 5.0) * 31 + floor(z * 5.0), k.seed);
  return {colour} * (0.62 + 0.38 * out) * (0.9 + 0.2 * spec);
}}
"""
    if not leaves:
        # No ring of leaves: a flat ring can't follow a slope (it floats
        # off the downhill side); fallen leaves are scattered on their own.
        import re
        src = re.sub(r"  // Leaves on the ground.*\n  const ground = .*\n", "  const ground = 10.0;\n", src)
        src = re.sub(r"  // The fallen leaves on the ground\.\n  if \(y < 0\.06\) \{\n.*?\n.*?\n  \}\n", "", src, flags=re.S)
    with open(os.path.join(HERE, "base", fname), "w") as f:
        f.write(src)


BARK_GREY = "mix(rgb(84, 76, 70), rgb(118, 108, 98), noise3(x * 6, y * 2, z * 6))"
BARK_DARK = "mix(rgb(56, 48, 42), rgb(86, 74, 62), noise3(x * 5, y * 5, z * 5))"
BARK_BIRCH = "(noise3(x * 4, y * 9, z * 4) > 0.7 ? rgb(46, 42, 40) : mix(rgb(222, 218, 206), rgb(244, 240, 230), noise3(x * 8, y * 3, z * 8)))"
BARK_APPLE = "mix(rgb(70, 58, 46), rgb(98, 82, 64), noise3(x * 6, y * 6, z * 6))"

if __name__ == "__main__":
    # Maples: each stand scarlet, flame orange or gold, the colour wandering
    # across the crown (a red maple going orange at its edges, gold on top).
    tree("1-sugar-maple.js", "sugar maple",
         "A sugar maple in its autumn colour: a straight grey-brown trunk forking into\nlimbs under a tall crown of separate leafy lobes gone scarlet, flame orange\nor gold, the colour wandering across it.",
         "tree", (4.6, 10.6, 4.6), 3.2, 0.42, 3, 5, 8.4, 1.9, 2.3,
         "hsv((f < 0.4 ? 0.005 : (f < 0.75 ? 0.045 : 0.1)) + (m - 0.5) * 0.09 + (n - 0.5) * 0.02, 0.86, 0.62 + n * 0.25)", BARK_GREY)
    tree("2-paper-birch.js", "paper birch",
         "A paper birch: a slender white trunk marked with black forking into thin\nlimbs, a narrow, airy crown of small lobes of leaves gone bright gold.",
         "birch", (2.8, 9.2, 2.8), 4.2, 0.17, 2, 4, 8.0, 1.1, 1.2,
         "hsv(0.115 + (m - 0.5) * 0.05 + (n - 0.5) * 0.02, 0.8, 0.66 + n * 0.28)", BARK_BIRCH, props="{ mass: 900, burns: 0.6, alive: 1, fuel: 0.9 }")
    tree("3-red-oak.js", "red oak",
         "A red oak: a thick dark trunk forking low into heavy limbs under a broad,\nspreading crown of lobes gone russet, burgundy and copper, holding its\nleaves late.",
         "oak", (5.4, 9.6, 5.4), 2.6, 0.6, 3, 6, 7.8, 2.0, 3.0,
         "hsv(0.025 + hash(k.seed, 3) * 0.035 + (m - 0.5) * 0.05 + (n - 0.5) * 0.02, 0.78, 0.5 + n * 0.26)", BARK_DARK, props="{ mass: 5000, burns: 0.45, alive: 1, fuel: 1 }")
    tree("5-apple-tree.js", "apple tree",
         "An old apple tree of the orchards: a short crooked trunk forking low into a\nrounded crown of yellowing leaves hung with red apples, windfalls in the\ngrass beneath.",
         "apple", (3.2, 4.8, 3.2), 1.3, 0.22, 3, 5, 3.6, 1.1, 1.5,
         "hsv(0.16 + (m - 0.5) * 0.06 + (n - 0.5) * 0.04, 0.62, 0.46 + n * 0.22)", BARK_APPLE, props="{ mass: 900, burns: 0.5, alive: 1, fuel: 0.9 }",
         extra_color="""
  // Apples dotted thick over the outside of the crown.
  const fx = fract(x * 2.4) - 0.5;
  const fy = fract(y * 2.4) - 0.5;
  const fz = fract(z * 2.4) - 0.5;
  if (hash(floor(x * 2.4), floor(y * 2.4) * 7 + floor(z * 2.4), k.seed) < 0.3 && fx * fx + fy * fy + fz * fz < 0.09) return mix(rgb(176, 24, 28), rgb(220, 60, 40), hash(floor(z * 2.4), floor(x * 2.4)));""")
