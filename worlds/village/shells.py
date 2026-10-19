"""Buildings people live and work in: a shop on the ground floor and a home
of two floors over it, with real stairs inside (and the hotel, the same but
wider). Front +z: the shop door and window on the street."""
import os

HERE = os.path.dirname(os.path.abspath(__file__))

G = 3.4      # ground floor: floor to floor
U = 3.0      # upper floors
SLAB = 0.2
WALL = 0.25


def write(name, src):
    with open(os.path.join(HERE, "types", name + ".js"), "w") as f:
        f.write(src.lstrip())


def stairs_sdf(name, x0, run, rise, n, y0, zc, width, up):
    """A straight flight along x from x0 (its foot) rising `rise` over `run`
    in `n` steps toward +x (up=1) or -x (up=-1), against the back wall."""
    t = run / n
    r = rise / n
    return f"""
  // {name}: {n} steps up {'+x' if up > 0 else '-x'}.
  const u{name} = ({up} * (x - ({x0}))) / {t:.4f};
  const i{name} = clamp(floor(u{name}), 0, {n - 1});
  const s{name} = min(box(x - ({x0}) - {up} * (i{name} + 0.5) * {t:.4f}, y - ({y0}) - ((i{name} + 1) * {r:.4f}) * 0.5, z - ({zc}), {t / 2:.4f}, ((i{name} + 1) * {r:.4f}) * 0.5, {width / 2:.3f}),
    box(x - ({x0}) - {up} * (clamp(i{name} + 1, 0, {n - 1}) + 0.5) * {t:.4f}, y - ({y0}) - ((clamp(i{name} + 1, 0, {n - 1}) + 1) * {r:.4f}) * 0.5, z - ({zc}), {t / 2:.4f}, ((clamp(i{name} + 1, 0, {n - 1}) + 1) * {r:.4f}) * 0.5, {width / 2:.3f}));"""


def shell(name, desc, W, D, floors_up, colours, door_x, window_x, sign_y):
    H = G + U * floors_up
    back = -D + WALL
    sw = 1.0                       # stair width
    zc = back + sw / 2
    # Ground to first floor: along the back wall from the left, up +x.
    run1 = 14 * 0.28
    x1 = -W + WALL + 0.05
    # First to second floor: on from where the first ends, up +x again
    # until the right wall.
    x2 = x1 + run1 + 0.25
    run2 = (W - WALL) - x2
    n2 = 12
    stairs = stairs_sdf("a", x1, run1, G, 14, 0.0, zc, sw, 1)
    steps2 = ""
    anchors = [
        f'{{ kind: "door", at: [{door_x}, 0, {D}], face: 0, size: [1.1, 2.3] }}',
        f'{{ kind: "stairs", at: [{x1 - 0.0 + 0.4:.2f}, 0, {back + sw + 0.55:.2f}], to: [{x1 + run1 + 0.2:.2f}, {G}, {zc:.2f}], face: 90 }}',
        f'{{ kind: "light", at: [{door_x}, 2.7, {D + 0.3}] }}',
    ]
    openings = f"box(x - ({x1 + run1 * 0.62:.3f}), y - {G - SLAB / 2}, z - ({zc}), {run1 * 0.4:.3f}, 0.3, {sw / 2 + 0.02})"
    if floors_up >= 2:
        steps2 = stairs_sdf("b", x2, run2, U, n2, G, zc, sw, 1)
        anchors.append(f'{{ kind: "stairs", at: [{x2 - 0.3:.2f}, {G}, {back + sw + 0.55:.2f}], to: [{W - WALL - 0.4:.2f}, {G + U}, {back + sw + 0.6:.2f}], face: 90 }}')
        openings = f"min({openings}, box(x - ({x2 + run2 * 0.6:.3f}), y - {G + U - SLAB / 2}, z - ({zc}), {run2 * 0.42:.3f}, 0.3, {sw / 2 + 0.02}))"
    floor_ys = [G] + [G + U * i for i in range(1, floors_up)]
    slabs = " , ".join(f"box(x, y - {fy - SLAB / 2}, z, {W - WALL + 0.01}, {SLAB / 2}, {D - WALL + 0.01})" for fy in floor_ys)
    slab_expr = f"min({slabs})" if len(floor_ys) > 1 else slabs
    rooms = f"box(x, y - {(H - 0.2) / 2:.3f}, z, {W - WALL}, {(H - 0.2) / 2:.3f}, {D - WALL})"
    cols = ", ".join(f"rgb({r}, {g}, {b})" for r, g, b in colours)
    write(name, f"""
// {desc}
// Inside: a shop on the ground floor; stairs up the back wall to the home
// over it ({floors_up} floor{'s' if floors_up > 1 else ''}). Its door and shop window face the street (+z).
export const meta = {{
  name: "{name.replace('-', ' ')}",
  bounds: [{W + 0.45}, {H + 2.7:.1f}, {D + 0.6}],
  tags: ["building", "house", "shop"],
  props: {{ mass: 100000, burns: 0.25, light: 1 }},
  sound: {{ hard: 0.9, dry: 0.8, ring: 0.1 }},
  anchors: [
    {(',' + chr(10) + '    ').join(anchors)},
  ],
}};

export function sdf(x, y, z, k) {{{stairs}{steps2}
  const body = box(x, y - {H / 2}, z, {W}, {H / 2}, {D});
  const rooms = {rooms};
  const slabs = min(max({slab_expr}, -{openings}), box(x, y - 0.025, z, {W - WALL + 0.01}, 0.025, {D - WALL + 0.01}));
  // Doorway and shop window on the street; windows above, front and back.
  const door = min(box(x - ({door_x}), y - 1.15, z - {D}, 0.55, 1.15, 0.4), max(length2(x - ({door_x}), y - 2.3) - 0.55, abs(z - {D}) - 0.4));
  const shopwin = box(x - ({window_x}), y - 1.55, z - {D}, 1.1, 0.85, 0.4);
  const wx = x - 1.9 * floor(x / 1.9 + 0.5);
  const fy = y - {G} - {U} * floor((y - {G}) / {U});
  const win = max(max(box(wx, fy - 1.45, abs(z) - {D}, 0.42, 0.7, 0.4), max(y - {H - 0.3}, {G} - y)), abs(x) - {W - 0.5});
  const shell = max(body, -min(min(rooms, door), min(shopwin, win)));
  const glass = box(x - ({window_x}), y - 1.55, z - {D - 0.12}, 1.1, 0.85, 0.03);
  const stairs = min(s{'a'}{', sb' if floors_up >= 2 else ''});
  const sills = max(max(box(wx, fy - 0.68, abs(z) - {D + 0.08}, 0.5, 0.05, 0.1), abs(x) - {W - 0.5}), max(y - {H - 0.3}, {G} - y));
  const shutters = max(max(box(abs(wx) - 0.67, fy - 1.45, abs(z) - {D + 0.05}, 0.24, 0.7, 0.03), abs(x) - {W - 0.3}), max(y - {H - 0.3}, {G} - y));
  const awning = max(box(x - ({window_x}), y - 2.75 + (z - {D}) * 0.35, z - {D + 0.6}, 1.3, 0.03, 0.6), {D} - z);
  const roof = max(max(box(x, y - {H + 0.9}, z, {W + 0.35}, 1.0, {D + 0.4}), (y - {H + 1.75} + abs(z) * 0.42) * 0.9), {H - 0.1} - y);
  const chim = box(x - {W * 0.5:.2f}, y - {H + 1.5}, z + 1.0, 0.32, 1.0, 0.32);
  return min(min(min(shell, slabs), min(glass, stairs)), min(min(sills, shutters), min(awning, min(roof, chim))));
}}

export function color(x, y, z, k) {{
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
  if (y > {H - 0.12}) {{
    if (abs(x + {W * 0.5:.2f}) < 0.36 && abs(z - 1.0) < 0.36) return y > {H + 2.4} ? rgb(70, 60, 56) : rgb(226, 214, 196);
    return mix(rgb(176, 78, 44), rgb(206, 104, 60), n) * (fract(x * 3.6) < 0.18 ? 0.8 : 1);
  }}
  const inside = abs(x) < {W - WALL + 0.02} && abs(z) < {D - WALL + 0.02};
  if (inside) {{
    // Floors of terracotta tiles, plastered walls, wooden stairs.
    // Height over this storey's floor (a hit a hair under a floor counts as on it).
    const yy = y + 0.06;
    const ly = yy - {G} * step({G}, yy) - {U} * max(floor((yy - {G}) / {U}), 0) * step({G}, yy) - 0.06;
    if (z > {back + 0.03} && z < {back + sw + 0.05} && y < {H - 0.4} && ly > 0.05) return mix(rgb(120, 84, 52), rgb(146, 104, 66), n);
    if (ly < 0.09) return (fract(x * 2.5) < 0.06 || fract(z * 2.5) < 0.06) ? rgb(150, 90, 62) : mix(rgb(196, 112, 76), rgb(214, 132, 92), n);
    return y < {G} ? rgb(236, 226, 206) : rgb(244, 236, 220);
  }}
  if (abs(z - {D - 0.12}) < 0.05 && abs(x - ({window_x})) < 1.1 && abs(y - 1.55) < 0.85) return glow(mix(rgb(60, 72, 80), rgb(255, 220, 160), 0.35), 0.6);
  if (z > {D + 0.05} && y > 2.5 && y < 3.0 && abs(x - ({window_x})) < 1.32) {{
    const c = floor(hash(k.seed, 9) * 4);
    const st = fract(x * 1.6) < 0.5;
    if (c < 1) return st ? rgb(200, 40, 40) : rgb(246, 240, 226);
    if (c < 2) return st ? rgb(30, 100, 60) : rgb(246, 240, 226);
    if (c < 3) return st ? rgb(30, 70, 150) : rgb(246, 240, 226);
    return st ? rgb(220, 150, 40) : rgb(246, 240, 226);
  }}
  const wx = x - 1.9 * floor(x / 1.9 + 0.5);
  const fy = y - {G} - {U} * floor((y - {G}) / {U});
  if (abs(z) > {D - 0.05} && y > {G}) {{
    if (abs(abs(wx) - 0.67) < 0.25 && abs(fy - 1.45) < 0.72) return (hash(k.seed, 5) > 0.5 ? rgb(46, 104, 170) : rgb(52, 120, 82)) * (fract(fy * 9.0) < 0.3 ? 0.82 : 1);
    if (abs(fy - 0.68) < 0.06) return rgb(236, 232, 222);
  }}
  if (y < 0.45) return mix(rgb(150, 140, 126), rgb(178, 168, 150), n);
  if (abs(y - {G}) < 0.1 || (abs(abs(x) - {W}) < 0.36 && abs(abs(z) - {D}) < 0.36 && fract(y * 1.1) < 0.5)) return rgb(240, 236, 226);
  let wall = rgb(226, 170, 80);
  const c = floor(hash(k.seed, 4) * {len(colours)});
  {chr(10).join(f'  if (c == {i}) wall = rgb({r}, {g}, {b});' for i, (r, g, b) in enumerate(colours))}
  return wall * (0.9 + 0.1 * n);
}}
""")


PLASTER = [(226, 170, 80), (238, 132, 100), (234, 156, 152), (240, 214, 112), (152, 192, 222), (164, 210, 176), (242, 230, 204), (248, 194, 146)]

if __name__ == "__main__":
    shell("shop-house", "A shop-house of Marinella: three floors of painted plaster (ochre, coral, rose, lemon, sky, mint, cream or apricot), a shop on the street with a wide window under a striped awning, green or blue shutters above, a terracotta roof and chimney.", 3.8, 4.4, 2, PLASTER, 1.9, -1.5, 3.0)
    shell("hotel", "The Albergo Marinella: a wide three-floor hotel of warm plaster on the piazza, its lobby on the ground floor behind a big window and an awning, guest rooms upstairs behind green shutters.", 6.6, 4.8, 2, [(232, 190, 120), (226, 160, 120)], 2.6, -2.6, 3.0)
