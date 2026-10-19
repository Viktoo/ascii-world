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
