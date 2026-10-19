"""A painted sign over every shop's door, its trade lettered on it."""
import os

from glyphs import block

HERE = os.path.dirname(os.path.abspath(__file__))

# Business key: (sign text, board colour, letter colour).
SIGNS = {
    "cantiere": ("CANTIERE NAVALE", (30, 70, 120), (250, 240, 210)),
    "pescheria": ("PESCHERIA", (30, 110, 150), (255, 255, 250)),
    "osteria": ("OSTERIA DEL PORTO", (110, 40, 30), (250, 220, 150)),
    "gelateria": ("GELATERIA", (236, 150, 170), (255, 255, 255)),
    "capitaneria": ("CAPITANERIA", (240, 236, 226), (30, 50, 110)),
    "bar": ("BAR TABACCHI", (30, 30, 34), (250, 210, 80)),
    "farmacia": ("FARMACIA", (30, 120, 70), (255, 255, 255)),
    "macelleria": ("MACELLERIA", (150, 30, 30), (255, 245, 230)),
    "alimentari": ("ALIMENTARI", (60, 110, 40), (255, 240, 200)),
    "pizzeria": ("PIZZERIA", (190, 50, 30), (255, 240, 200)),
    "barbiere": ("BARBIERE", (240, 236, 226), (170, 30, 30)),
    "veterinario": ("VETERINARIO", (50, 130, 150), (255, 255, 255)),
    "libreria": ("LIBRI E GIORNALI", (70, 50, 40), (240, 220, 170)),
    "panetteria": ("PANETTERIA", (200, 150, 70), (80, 40, 20)),
    "sartoria": ("SARTORIA", (110, 70, 130), (250, 240, 230)),
    "albergo": ("ALBERGO MARINELLA", (30, 60, 100), (250, 220, 120)),
    "caffe": ("CAFFE DEL PORTO", (20, 80, 60), (250, 236, 200)),
    "fioraio": ("FIORI", (230, 210, 120), (60, 110, 40)),
    "scuola": ("SCUOLA", (240, 236, 226), (40, 60, 120)),
}


def sign(key):
    text, bg, fg = SIGNS[key]
    w = max(1.6, len(text) * 0.27 + 0.4) / 2
    lines = block([(text, 0, 0.12, 0.32, {"stroke": 0.14})])
    src = f"""
// The sign over a Marinella shop: {text}, in painted letters on a board
// fixed to the wall over its door and window. Its face is +z.
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
    fingerpost("crossroads-signpost", "The signpost at the crossroads below Marinella: a weathered post with four\nred arms lettered in white, each pointing the way: MARINELLA down to the\ngate, BOSCOVERDE up the woodcutters' road, the beach west along the coast,\nthe lighthouse east.",
               [("MARINELLA", 182, 2.35), ("BOSCOVERDE", 22, 2.05), ("SPIAGGIA", 277, 1.75), ("FARO", 97, 1.45)])
