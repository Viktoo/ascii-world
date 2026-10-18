"""Lettering for the town's signs, as code a type's color() can run.

The type language has no strings, so a line of text becomes a test of
whether a point on the sign lies on a lit stroke: each letter is drawn with
up to fourteen strokes (a 14-segment face), chosen by a number per letter.
`run(...)` returns statements that set `lit{n} = 1` where the point (tx, ty)
lies on the text.
"""

# Strokes in a letter's cell (x 0..1 across, y 0..1 up), by name.
SEG = {
    "a": ((0, 1), (1, 1)), "b": ((1, 1), (1, 0.5)), "c": ((1, 0.5), (1, 0)), "d": ((0, 0), (1, 0)),
    "e": ((0, 0), (0, 0.5)), "f": ((0, 0.5), (0, 1)), "g1": ((0, 0.5), (0.5, 0.5)), "g2": ((0.5, 0.5), (1, 0.5)),
    "h": ((0, 1), (0.5, 0.5)), "i": ((0.5, 1), (0.5, 0.5)), "j": ((1, 1), (0.5, 0.5)), "k": ((0, 0), (0.5, 0.5)),
    "l": ((0.5, 0.5), (0.5, 0)), "m": ((0.5, 0.5), (1, 0)),
}
ORDER = list(SEG)

FONT = {
    "A": "a b c e f g1 g2", "B": "a b c d e f g2", "C": "a d e f", "D": "a b c d i l", "E": "a d e f g1",
    "F": "a e f g1", "G": "a c d e f g2", "H": "b c e f g1 g2", "I": "a d i l", "J": "b c d e", "K": "e f g1 j m",
    "L": "d e f", "M": "b c e f h j", "N": "b c e f h m", "O": "a b c d e f", "P": "a b e f g1 g2",
    "Q": "a b c d e f m", "R": "a b e f g1 g2 m", "S": "a c d f g1 g2", "T": "a i l", "U": "b c d e f",
    "V": "e f k j", "W": "b c e f k m", "X": "h j k m", "Y": "h j l", "Z": "a d j k",
    "0": "a b c d e f", "1": "b c", "2": "a b d e g1 g2", "3": "a b c d g2", "4": "b c f g1 g2",
    "5": "a c d f g1 g2", "6": "a c d e f g1 g2", "7": "a b c", "8": "a b c d e f g1 g2", "9": "a b c d f g1 g2",
    "-": "g1 g2", " ": "", ".": "", "&": "a d e h m g1",
}


def mask(ch):
    on = FONT.get(ch.upper(), "").split()
    return sum(1 << ORDER.index(s) for s in on)


def _f(v):
    return f"{v:.4f}".rstrip("0").rstrip(".") if v != int(v) else f"{int(v)}"


def strokes(w, h, t, n):
    """Statements testing the 14 strokes of letter mask `m{n}` at (cx{n}, cy{n})."""
    out = []
    for k, name in enumerate(ORDER):
        (x1, y1), (x2, y2) = SEG[name]
        # Strokes stop short of the corners, so letters read at a distance.
        ins = 0.08
        if x1 == x2:
            ya, yb = sorted((y1, y2))
            d = f"max(abs(cx{n} - {_f(x1)}) * {_f(w)}, max(max({_f(ya + ins)} - cy{n}, cy{n} - {_f(yb - ins)}), 0) * {_f(h)})"
        elif y1 == y2:
            xa, xb = sorted((x1, x2))
            d = f"max(abs(cy{n} - {_f(y1)}) * {_f(h)}, max(max({_f(xa + ins)} - cx{n}, cx{n} - {_f(xb - ins)}), 0) * {_f(w)})"
        else:
            vx, vy = (x2 - x1) * w, (y2 - y1) * h
            ll = vx * vx + vy * vy
            px, py = f"(cx{n} - {_f(x1)}) * {_f(w)}", f"(cy{n} - {_f(y1)}) * {_f(h)}"
            tt = f"clamp(({px} * {_f(vx)} + {py} * {_f(vy)}) / {_f(ll)}, 0.12, 0.88)"
            d = f"length2({px} - {_f(vx)} * {tt}, {py} - {_f(vy)} * {tt})"
        out.append(f"if (fract(m{n} / {1 << (k + 1)}) >= 0.5 && {d} < {_f(t)}) lit{n} = 1;")
    return out


def run(text, x0, y0, h, n, pitch=0.82, width=0.58, stroke=0.11, centre=True):
    """Statements lighting `text` (cap height h, metres) at (x0, y0) on the
    sign's plane (tx across, ty up). With `centre`, x0 is its middle."""
    text = text.upper()
    w = h * width
    p = h * pitch
    total = len(text) * p - (p - w)
    left = x0 - total / 2 if centre else x0
    masks = [mask(c) for c in text]
    chain = " : ".join(f"ci{n} < {i + 1} ? {m}" for i, m in enumerate(masks[:-1])) + f" : {masks[-1]}" if len(masks) > 1 else f"{masks[0]}"
    t = stroke * h
    body = [
        f"const gx{n} = (tx - {_f(left)}) / {_f(p)};",
        f"const ci{n} = floor(gx{n});",
        f"if (gx{n} >= 0 && ci{n} < {len(text)} && ty > {_f(y0 - t)} && ty < {_f(y0 + h + t)}) {{",
        f"  const cx{n} = (gx{n} - ci{n}) * {_f(p / w)};",
        f"  const cy{n} = (ty - {_f(y0)}) / {_f(h)};",
        f"  const m{n} = {chain};",
    ]
    body += ["  " + s for s in strokes(w, h, t, n)]
    body.append("}")
    return body


def block(runs):
    """`let lit0 = 0; …` then each run's statements, indented for color():
    after them lit{i} is 1 where run i's text is."""
    lines = [f"let lit{i} = 0;" for i in range(len(runs))]
    for i, r in enumerate(runs):
        lines += run(*r[:4], n=i, **(r[4] if len(r) > 4 else {}))
    return "\n".join("  " + l for l in lines)
