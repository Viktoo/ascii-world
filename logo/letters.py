"""Writes types/letters.js: INFINITE over POCKET as solid slab-serif letters,
both lines the same width, standing on the ground, face +z."""
import os
import sys

W = float(sys.argv[1]) if len(sys.argv) > 1 else 11.0  # line width, metres
DEPTH = 0.32       # half thickness
GAP = 0.18         # between the lines (in metres)
TRACK = 0.12       # between letters (in cap heights)

# Glyphs in a cell: x 0..w, y 0..1 (cap height 1). Stroke weights.
S = 0.21   # stem
H = 0.15   # bar
SE = 0.07  # serif slab half height
SO = 0.09  # serif overhang


def f(v):
    return f"{v:.4f}"


def rect(x0, y0, x1, y1):
    cx, cy, hx, hy = (x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2, (y1 - y0) / 2
    return f"max(abs(px - {f(cx)}) - {f(hx)}, abs(py - {f(cy)}) - {f(hy)})"


def slant(ax, ay, bx, by, half):
    """A thick straight stroke from (ax, ay) to (bx, by), cut flat at y = ay, by."""
    import math
    dx, dy = bx - ax, by - ay
    L = math.hypot(dx, dy)
    nx, ny = -dy / L, dx / L
    lo, hi = min(ay, by), max(ay, by)
    return f"max(abs((px - {f(ax)}) * {f(nx)} + (py - {f(ay)}) * {f(ny)}) - {f(half)}, abs(py - {f((lo + hi) / 2)}) - {f((hi - lo) / 2)})"


def ell(cx, cy, rx, ry):
    m = min(rx, ry)
    return f"(length2((px - {f(cx)}) / {f(rx)}, (py - {f(cy)}) / {f(ry)}) - 1) * {f(m)}"


def ring(cx, cy, rx, ry, tx, ty):
    return f"max({ell(cx, cy, rx, ry)}, -({ell(cx, cy, rx - tx, ry - ty)}))"


def stem(x0, serif_top=True, serif_bot=True, w=S):
    parts = [rect(x0, 0, x0 + w, 1)]
    if serif_top:
        parts.append(rect(x0 - SO, 1 - 2 * SE, x0 + w + SO, 1))
    if serif_bot:
        parts.append(rect(x0 - SO, 0, x0 + w + SO, 2 * SE))
    return parts


def U(parts):
    out = parts[0]
    for p in parts[1:]:
        out = f"min({out}, {p})"
    return out


GLYPH = {}
GLYPH["I"] = (0.21 + 2 * SO, lambda: stem(SO))
GLYPH["N"] = (0.86, lambda: [rect(0.0, 0, S * 0.7, 1), rect(-SO * 0.6, 0, S * 0.7 + SO, 2 * SE), rect(-SO * 0.6, 1 - 2 * SE, 0.2, 1),
                             rect(0.86 - S * 0.7, 0, 0.86, 1), rect(0.86 - S * 0.7 - SO, 1 - 2 * SE, 0.86 + SO * 0.6, 1),
                             slant(0.08, 1.0, 0.78, 0.0, S * 0.55)])
GLYPH["F"] = (0.66, lambda: stem(0.0) + [rect(0, 1 - H, 0.66, 1), rect(0.66 - 0.09, 0.72, 0.66, 1), rect(0, 0.47, 0.5, 0.47 + H * 0.9), rect(0.44, 0.40, 0.5, 0.62)])
GLYPH["E"] = (0.68, lambda: stem(0.0) + [rect(0, 1 - H, 0.66, 1), rect(0.66 - 0.09, 0.72, 0.66, 1), rect(0, 0.46, 0.5, 0.46 + H * 0.9), rect(0.44, 0.39, 0.5, 0.61),
                                         rect(0, 0, 0.68, H), rect(0.68 - 0.09, 0, 0.68, 0.3)])
GLYPH["T"] = (0.76, lambda: [rect(0.38 - S / 2, 0, 0.38 + S / 2, 1), rect(0.38 - S / 2 - SO, 0, 0.38 + S / 2 + SO, 2 * SE),
                             rect(0, 1 - H, 0.76, 1), rect(0, 0.7, 0.09, 1), rect(0.67, 0.7, 0.76, 1)])
GLYPH["P"] = (0.74, lambda: stem(0.0) + [max_(ring(0.37, 0.735, 0.37, 0.265, 0.2, 0.14), f"-(px - 0.15)"),
                                         rect(0, 1 - H * 0.93, 0.38, 1), rect(0, 0.47, 0.38, 0.47 + H * 0.9)])
GLYPH["O"] = (0.92, lambda: [ring(0.46, 0.5, 0.46, 0.51, 0.22, 0.13)])
GLYPH["C"] = (0.84, lambda: [max_(ring(0.46, 0.5, 0.46, 0.51, 0.22, 0.13), f"-max(0.6 - px, abs(py - 0.5) - 0.13 - (px - 0.6) * 0.55) * 0.87")])
GLYPH["K"] = (0.84, lambda: stem(0.0) + [slant(0.16, 0.42, 0.66, 1.0, S * 0.45), slant(0.34, 0.62, 0.8, 0.0, S * 0.55),
                                         rect(0.5, 1 - 2 * SE, 0.9, 1), rect(0.6, 0, 0.92, 2 * SE)])


def max_(a, b):
    return f"max({a}, {b})"


def line(text):
    ws = [GLYPH[c][0] for c in text]
    return ws, sum(ws) + TRACK * (len(text) - 1)


top, bot = "INFINITE", "POCKET"
_, wt = line(top)
_, wb = line(bot)
ht, hb = W / wt, W / wb       # cap heights in metres
y_bot, y_top = 0.0, hb + GAP
total_h = y_top + ht


def emit(text, cap, y0, var):
    ws, tot = line(text)
    left = -W / 2
    lines = [f"  // {text}: cap height {cap:.2f} m"]
    lines.append(f"  {var} = (max(abs(ty - {f(y0 + cap / 2)}) - {f(cap / 2)}, 0.0));")
    lines.append(f"  if (ty > {f(y0 - 0.05)} && ty < {f(y0 + cap + 0.05)}) {{")
    lines.append(f"    const py = (ty - {f(y0)}) / {f(cap)};")
    x = 0.0
    first = True
    for i, c in enumerate(text):
        w = ws[i]
        a, b = x - TRACK / 2, x + w + TRACK / 2
        cond = f"gx < {f(b)}" if i < len(text) - 1 else "true"
        kw = "if" if first else "} else if"
        first = False
        lines.append(f"    {kw} ({cond}) {{")
        # Only this cell's letter is measured; the cell's edge bounds the rest.
        parts = []
        for j in (i - 1, i, i + 1):
            if 0 <= j < len(text):
                ox = sum(ws[:j]) + TRACK * j
                parts.append(U(GLYPH[text[j]][1]()).replace("px", f"(gx - {f(ox)})"))
        lines.append(f"      {var} = {U(parts)} * {f(cap)};")
        x += w + TRACK
    lines.append("    }")
    lines.append("  }")
    return "\n".join(lines), f"const gx = (tx + {f(W / 2)}) / {f(cap)};"


ebot, gb = emit(bot, hb, y_bot, "db")
etop, gt = emit(top, ht, y_top, "dt")

src = f"""// The title of the game, INFINITE over POCKET, in big solid slab-serif
// letters cut from painted wood, both words the same width, standing in the
// meadow. Its face is +z.
export const meta = {{
  name: "infinite pocket letters",
  bounds: [{W / 2 + 0.3:.2f}, {total_h + 0.1:.2f}, {DEPTH + 0.05:.2f}],
  tags: ["sign", "landmark"],
  props: {{ mass: 4000, burns: 0.3 }},
}};

export function sdf(x, y, z, k) {{
  const tx = -x;
  const ty = y;
  let db = 9;
  let dt = 9;
  {{
    {gb}
{ebot}
  }}
  {{
    {gt}
{etop}
  }}
  // Out of both lines: the distance to them.
  const d2 = min(db, dt);
  // Extruded, the edges softly rounded.
  const r = 0.06;
  const wx = d2 + r;
  const wz = abs(z) - {f(DEPTH)} + r;
  return min(max(wx, wz), 0.0) + length2(max(wx, 0.0), max(wz, 0.0)) - r;
}}

export function color(x, y, z, k) {{
  const n = noise3(x * 3, y * 3, z * 3);
  const grain = noise3(x * 1.5, y * 40, z * 2);
  // The face: warm cream, a little golden low down; the sides deep red.
  if (abs(z) > {f(DEPTH - 0.03)}) {{
    const top = ty_(y);
    return mix(rgb(255, 214, 120), rgb(255, 246, 226), top) * (0.94 + 0.06 * n);
  }}
  return mix(rgb(150, 40, 28), rgb(190, 72, 34), grain) * (0.9 + 0.1 * n);
}}
""".replace("ty_(y)", f"clamp(((y < {f(y_top - GAP / 2)} ? (y - {f(y_bot)}) / {f(hb)} : (y - {f(y_top)}) / {f(ht)})) * 0.8 + 0.2, 0, 1)")

open(sys.argv[2] if len(sys.argv) > 2 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "types", "letters.js"), "w").write(src)
print(f"line width {W} m; POCKET cap {hb:.2f} m, INFINITE cap {ht:.2f} m, total height {total_h:.2f} m", file=sys.stderr)
