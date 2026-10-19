"""A painted sign over every shop's door, its trade lettered on it."""
import os

from glyphs import block

HERE = os.path.dirname(os.path.abspath(__file__))

# Business key: (sign text, board colour, letter colour).
SIGNS = {
    "tavern": ("THE ANCHOR TAVERN", (30, 40, 60), (236, 200, 110)),
    "inn": ("HARVEST MOON INN", (40, 70, 50), (246, 236, 210)),
    "store": ("GENERAL STORE", (246, 242, 230), (40, 60, 120)),
    "bakery": ("BAKERY", (120, 60, 40), (250, 230, 190)),
    "fishshed": ("FRESH FISH", (36, 110, 120), (250, 250, 244)),
    "boatshop": ("BOAT SHOP", (246, 242, 230), (150, 40, 34)),
    "school": ("SCHOOL", (246, 242, 230), (40, 40, 40)),
    "smithy": ("SMITHY", (30, 30, 32), (230, 170, 80)),
}


def sign(key):
    text, bg, fg = SIGNS[key]
    w = max(1.6, len(text) * 0.27 + 0.4) / 2
    lines = block([(text, 0, 0.12, 0.32, {"stroke": 0.14})])
    src = f"""
// The sign over a Maple Cove business: {text}, in painted letters on a
// board over its door. Its face is +z.
export const meta = {{
  name: "{key} sign",
  bounds: [{w + 0.05:.2f}, 0.6, 0.1],
  tags: ["sign", "prop"],
  props: {{ mass: 20, light: 1 }},
}};

export function sdf(x, y, z, k) {{
  return roundBox(x, y - 0.28, z, {w:.2f}, 0.28, 0.04, 0.02);
}}

export function color(x, y, z, k) {{
  if (z < 0.02) return rgb(90, 70, 50);
  if (abs(abs(x) - {w - 0.06:.2f}) < 0.03 || abs(abs(y - 0.28) - 0.22) < 0.03) return rgb({fg[0]}, {fg[1]}, {fg[2]});
  const tx = -x;
  const ty = y;
{lines}
  if (lit0 > 0) return glow(rgb({fg[0]}, {fg[1]}, {fg[2]}), 0.5);
  return rgb({bg[0]}, {bg[1]}, {bg[2]});
}}
"""
    with open(os.path.join(HERE, "types", f"{key}-sign.js"), "w") as f:
        f.write(src.lstrip())
    return w


if __name__ == "__main__":
    for k in SIGNS:
        sign(k)


def fingerpost(slug, desc, arms):
    """A wooden post with lettered arms, each pointing along a bearing
    (degrees, 0 = +z, 90 = +x) at its own height: (text, bearing, height)."""
    import math
    runs = [(t, 0, 10 * i - 0.075, 0.15, {"stroke": 0.15}) for i, (t, b, h) in enumerate(arms)]
    lines = block(runs)
    parts, which = [], []
    for i, (t, b, h) in enumerate(arms):
        s, c = math.sin(math.radians(b)), math.cos(math.radians(b))
        L = max(1.2, len(t) * 0.125 + 0.35)
        parts.append(f"""  const u{i} = x * {s:.4f} + z * {c:.4f};
  const w{i} = x * {c:.4f} - z * {s:.4f};
  const a{i} = max(box(u{i} - {L / 2 + 0.08:.3f}, y - {h}, w{i}, {L / 2:.3f}, 0.11, 0.025), (u{i} - {L + 0.02:.3f}) + abs(y - {h}) * 0.9);""")
        which.append(f"""  if (abs(y - {h}) < 0.12 && abs(u{i} - {L / 2 + 0.08:.3f}) < {L / 2 + 0.12:.3f} && abs(w{i}) < 0.04) {{ k = {i}; tx = w{i} < 0 ? -(u{i} - {L / 2 + 0.03:.3f}) : (u{i} - {L / 2 + 0.03:.3f}); ty = y - {h} + {10 * i}; }}""")
    arm_min = "min(" + ", ".join(f"a{i}" for i in range(len(arms))) + ")" if len(arms) > 1 else "a0"
    top = max(h for _, _, h in arms) + 0.35
    reach = max(max(1.2, len(t) * 0.125 + 0.35) for t, _, _ in arms) + 0.15
    desc = desc.replace(chr(10), chr(10) + "// ")
    src = f"""
// {desc}
export const meta = {{
  name: "{slug.replace('-', ' ')}",
  bounds: [{reach:.2f}, {top + 0.1:.2f}, {reach:.2f}],
  tags: ["sign", "landmark"],
  props: {{ mass: 60, burns: 0.5 }},
  sound: {{ hard: 0.6, dry: 0.7, ring: 0.1 }},
}};

export function sdf(x, y, z, k) {{
  const post = box(x, y - {top / 2:.3f}, z, 0.07, {top / 2:.3f}, 0.07);
  const cap = cappedCone(x, y - {top + 0.05:.3f}, z, 0.06, 0.12, 0.02);
{chr(10).join(parts)}
  return min(min(post, cap), {arm_min});
}}

export function color(x, y, z, k) {{
{chr(10).join(p.split(chr(10))[0] + chr(10) + p.split(chr(10))[1] for p in parts)}
  let kk = -1;
  let tx = 0.0;
  let ty = -100.0;
{chr(10).join(w.replace('k = ', 'kk = ') for w in which)}
  if (kk < 0) return mix(rgb(96, 70, 46), rgb(120, 90, 60), noise3(x * 8, y * 8, z * 8));
{lines}
  if (lit0 + lit1 + lit2 + lit3 > 0) return rgb(250, 246, 236);
  return rgb(150, 40, 30);
}}
"""
    with open(os.path.join(HERE, "types", f"{slug}.js"), "w") as f:
        f.write(src.lstrip())


if __name__ == "__main__":
    fingerpost("crossroads-signpost", "The signpost where the roads meet above Maple Cove: a weathered post with\nfour red arms lettered in white, each pointing the way: MAPLE COVE down to\nthe green, PINE HOLLOW up the woodcutters' road, the beach along the coast,\nthe lighthouse out on the point.",
               [("MAPLE COVE", 182, 2.35), ("PINE HOLLOW", 22, 2.05), ("THE BEACH", 277, 1.75), ("LIGHTHOUSE", 97, 1.45)])
