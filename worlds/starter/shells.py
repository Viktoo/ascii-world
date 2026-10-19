"""The buildings of Maple Cove: every one a single storey of clapboard under
a steep, brightly painted roof, the homes and the businesses alike; some
with a porch along the front, some with rooms walled off inside, a hearth.
Front +z: the door and the windows look out that way."""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
WALL = 0.2
PITCH = 0.8          # the roof's rise over its run
EAVE = 0.35

# Clapboard: white, cream, dove grey, pale blue, sage, butter, barn red.
WALLS = [(240, 238, 230), (236, 224, 196), (196, 198, 196), (176, 194, 206), (172, 186, 152), (236, 214, 150), (150, 50, 40)]
# The roofs, every colour: barn red, teal, cobalt, forest green, mustard,
# plum, pumpkin, sky blue.
ROOFS = [(186, 48, 40), (36, 130, 130), (46, 80, 160), (46, 110, 70), (214, 160, 40), (124, 56, 104), (214, 104, 38), (78, 150, 204)]
SHUTTERS = [(40, 60, 46), (34, 40, 60), (30, 30, 34), (120, 40, 36)]


def write(name, src):
    with open(os.path.join(HERE, "types", name + ".js"), "w") as f:
        f.write(src.lstrip())


def palette(var, cols, key):
    out = [f"  let {var} = rgb({cols[0][0]}, {cols[0][1]}, {cols[0][2]});", f"  const c{var} = floor(hash(k.seed, {key}) * {len(cols)});"]
    out += [f"  if (c{var} == {i}) {var} = rgb({r}, {g}, {b});" for i, (r, g, b) in enumerate(cols) if i]
    return "\n".join(out)


def house(slug, desc, W, D, H, door_x, windows, walls=WALLS, roofs=ROOFS, porch=0.0, inner=(), hearth=None, door_w=0.55,
          extra_sdf="", extra_color="", anchors=(), tags=("building", "house"), chimney=True, tall=0.0):
    """W, D: half width and depth; H: the walls' height. `windows`: x of the
    front windows (the back gets the same; each side one in its middle).
    `inner`: walls inside, ("x", at, z0, z1, door z) or ("z", at, x0, x1, door x).
    `hearth`: x of a fireplace on the back wall."""
    R = (D + EAVE) * PITCH
    top = H + R
    fz = D + porch
    win_front = " , ".join(f"box(x - ({wx}), y - 1.45, z, 0.45, 0.62, {D + 0.3})" for wx in windows)
    win_front = f"min({win_front})" if len(windows) > 1 else win_front
    walls_in = []
    for w in inner:
        if w[0] == "x":
            _, at, z0, z1, dz = w
            walls_in.append(f"max(box(x - ({at}), y - {(H - 0.2) / 2:.3f}, z - ({(z0 + z1) / 2}), 0.08, {(H - 0.2) / 2:.3f}, {(z1 - z0) / 2}), -box(x - ({at}), y - 1.05, z - ({dz}), 0.3, 1.05, 0.5))")
        else:
            _, at, x0, x1, dx = w
            walls_in.append(f"max(box(x - ({(x0 + x1) / 2}), y - {(H - 0.2) / 2:.3f}, z - ({at}), {(x1 - x0) / 2}, {(H - 0.2) / 2:.3f}, 0.08), -box(x - ({dx}), y - 1.05, z - ({at}), 0.5, 1.05, 0.3))")
    inner_expr = "10.0"
    for e in walls_in:
        inner_expr = f"min({inner_expr}, {e})"
    hearth_sdf = "10.0"
    hearth_col = ""
    all_anchors = [f'{{ kind: "door", at: [{door_x}, 0, {D}], face: 0, size: [{door_w * 2:.1f}, 2.2] }}',
                   f'{{ kind: "light", at: [{door_x + door_w + 0.35:.2f}, 2.3, {D + 0.15}] }}',
                   f'{{ kind: "light", at: [0, {H - 0.5:.1f}, 0] }}'] + list(anchors)
    if hearth is not None:
        hearth_sdf = f"max(box(x - ({hearth}), y - 0.85, z + {D - WALL - 0.3:.2f}, 0.8, 0.85, 0.35), -box(x - ({hearth}), y - 0.45, z + {D - WALL - 0.45:.2f}, 0.5, 0.42, 0.3))"
        hearth_col = f"""  if (abs(x - ({hearth})) < 0.82 && z < -{D - WALL - 0.67:.2f} && y < 1.72 && abs(z) < {D}) {{
    if (abs(x - ({hearth})) < 0.5 && y < 0.6 && y > 0.04) return glow(rgb(255, 150, 60), 0.9);
    return mix(rgb(130, 60, 44), rgb(160, 80, 56), noise3(x * 6, y * 6, z * 6)) * (fract(y * 8.0) < 0.12 ? 0.8 : 1.0);
  }}"""
        all_anchors.append(f'{{ kind: "light", at: [{hearth}, 0.4, {-(D - WALL - 0.6):.2f}] }}')
    porch_sdf = "10.0"
    porch_col = ""
    if porch > 0:
        porch_sdf = (f"min(min(box(x, y - 0.1, z - {D + porch / 2:.2f}, {W}, 0.1, {porch / 2:.2f}), "
                     f"box(abs(x) - {W - 0.15:.2f}, y - {H / 2:.2f}, z - {fz - 0.15:.2f}, 0.08, {H / 2:.2f}, 0.08)), "
                     f"box(x, y - ({H - 0.08:.2f} - (z - {D}) * 0.22), z - {D + porch / 2 + 0.1:.2f}, {W + 0.15:.2f}, 0.06, {porch / 2 + 0.25:.2f}))")
        if W > 4.5:
            porch_sdf = f"min({porch_sdf}, box(abs(x) - {W / 3:.2f}, y - {H / 2:.2f}, z - {fz - 0.15:.2f}, 0.08, {H / 2:.2f}, 0.08))"
        porch_col = f"""  if (z > {D + 0.02} && y < 0.21) return fract(x * 5.0) < 0.08 ? rgb(110, 90, 70) : rgb(170, 146, 116);
  if (z > {D + 0.05} && y > {H - 0.6:.2f}) return roof * 0.85;
  if (z > {D + 0.05}) return rgb(244, 242, 236);"""
    chim_sdf = f"box(x + {W * 0.55:.2f}, y - {(H + top + 0.7) / 2:.2f}, z + 0.5, 0.32, {(top + 0.7 - H) / 2:.2f}, 0.32)" if chimney else "10.0"
    write(slug, f"""
// {desc.replace(chr(10), chr(10) + '// ')}
export const meta = {{
  name: "{slug.replace('-', ' ').replace(' s ', "'s ")}",
  bounds: [{W + EAVE + 0.15:.2f}, {max(top + 0.8, tall):.2f}, {fz + EAVE + 0.2:.2f}],
  tags: {list(tags)!r},
  props: {{ mass: 60000, burns: 0.5, light: 1 }},
  sound: {{ hard: 0.6, dry: 0.8, ring: 0.05 }},
  anchors: [
    {(',' + chr(10) + '    ').join(all_anchors).replace("'", '"')},
  ],
}};

export function sdf(x, y, z, k) {{
  const body = box(x, y - {H / 2}, z, {W}, {H / 2}, {D});
  const rooms = box(x, y - {(H - 0.2) / 2:.3f}, z, {W - WALL}, {(H - 0.2) / 2:.3f}, {D - WALL});
  const door = box(x - ({door_x}), y - 1.1, z - {D}, {door_w}, 1.1, 0.4);
  const wins = min({win_front}, box(abs(x) - {W}, y - 1.45, z, 0.4, 0.62, 0.45));
  const shell = max(body, -min(min(rooms, door), wins));
  const floor0 = box(x, y - 0.025, z, {W - WALL + 0.01}, 0.025, {D - WALL + 0.01});
  const glass = min(box(abs(x) - {W - 0.06:.2f}, y - 1.45, z, 0.02, 0.6, 0.42), {win_front.replace(f'{D + 0.3}', '0.02').replace('box(x', 'box(x').replace(', z,', f', abs(z) - {D - 0.08:.2f},')});
  // The roof: a steep shell over the gable walls, the gables clapboard.
  const pl = ((y - {top:.3f}) + abs(z) * {PITCH}) * {1 / (1 + PITCH * PITCH) ** 0.5:.4f};
  const roof = max(max(abs(pl + 0.09) - 0.09, abs(x) - {W + EAVE}), max({H - 0.2} - y, abs(z) - {D + EAVE}));
  const gable = max(pl, max(abs(x) - {W}, {H - 0.05} - y));
  const chim = {chim_sdf};
  const inner = {inner_expr};
  const hearth = {hearth_sdf};
  const porch = {porch_sdf};{extra_sdf}
  return min(min(min(shell, floor0), min(glass, roof)), min(min(gable, chim), min(min(inner, hearth), porch)));
}}

export function color(x, y, z, k) {{
  const n = noise3(x * 1.3, y * 1.3, z * 1.3);
{palette("wall", walls, 4)}
{palette("roof", roofs, 5)}
{palette("shut", SHUTTERS, 6)}
{extra_color}  const pl = ((y - {top:.3f}) + abs(z) * {PITCH}) * {1 / (1 + PITCH * PITCH) ** 0.5:.4f};
  if (abs(x + {W * 0.55:.2f}) < 0.34 && abs(z + 0.5) < 0.34 && y > {H + 0.2}) return y > {top + 0.55:.2f} ? rgb(60, 54, 50) : mix(rgb(140, 64, 48), rgb(166, 84, 60), noise3(x * 8, y * 8, z * 8));
  if (pl > -0.2 && y > {H - 0.2} && (abs(x) > {W - 0.01} || pl > -0.17)) return roof * (fract((y - abs(z) * {PITCH}) * 3.0) < 0.12 ? 0.82 : 1.0) * (0.92 + 0.08 * n);
{porch_col}
  const inside = abs(x) < {W - WALL + 0.02} && abs(z) < {D - WALL + 0.02};
{hearth_col}
  if (inside) {{
    // Wide pine floorboards, whitewashed walls, beams overhead.
    if (y < 0.07) return fract(x * 3.0) < 0.06 ? rgb(110, 80, 52) : mix(rgb(176, 130, 84), rgb(196, 150, 100), noise3(x * 3, 1, z * 12));
    if (y > {H - 0.45}) return fract(x * 0.7) < 0.12 ? rgb(110, 80, 56) : rgb(236, 228, 210);
    return rgb(240, 234, 220);
  }}
  // Glass: warm with lamplight.
  if (abs(y - 1.45) < 0.62 && ((abs(abs(x) - {W - 0.06:.2f}) < 0.04 && abs(z) < 0.44) || abs(abs(z) - {D - 0.08:.2f}) < 0.04)) return glow(mix(rgb(90, 76, 60), rgb(255, 200, 120), 0.65), 0.75);
  // Window frames and shutters.
  const fw = abs(z) > {D - 0.03} || abs(x) > {W - 0.03};
  if (fw && abs(y - 1.45) < 0.72) {{
    const wx = abs(z) > {D - 0.03} ? x : z;
    const near = {" || ".join(f"abs(abs(wx - ({wx})) - 0.45) < 0.07" for wx in windows)} || (abs(x) > {W - 0.03} && abs(abs(z) - 0.45) < 0.07);
    if (near) return rgb(246, 244, 238);
    const sh = {" || ".join(f"abs(abs(wx - ({wx})) - 0.7) < 0.18" for wx in windows)} || (abs(x) > {W - 0.03} && abs(abs(z) - 0.7) < 0.18);
    if (sh && abs(y - 1.45) < 0.64) return shut * (fract(y * 10.0) < 0.3 ? 0.8 : 1.0);
  }}
  if (abs(z - {D}) < 0.06 && abs(x - ({door_x})) < {door_w + 0.12} && y < 2.32) return rgb(246, 244, 238);
  if (y < 0.3) return mix(rgb(120, 118, 112), rgb(150, 146, 138), noise3(x * 4, y * 4, z * 4));
  // Corner boards, then the clapboard.
  if (abs(abs(x) - {W}) < 0.14 && abs(abs(z) - {D}) < 0.14) return rgb(246, 244, 238);
  return wall * (fract(y * 5.0) < 0.14 ? 0.86 : 1.0) * (0.95 + 0.05 * n);
}}
""")


TAG_SHOP = ("building", "house", "shop")


def all_types():
    house("cottage", "A cottage of Maple Cove: a single storey of clapboard (white, cream, dove\ngrey, pale blue, sage, butter or barn red) under a steep roof painted a\nbright colour, a brick chimney, shutters at the windows. One snug room\ninside with pine floorboards. Its door faces +z.",
          3.6, 3.0, 2.8, 0.9, [-1.8, 2.3])
    house("family-cottage", "A family cottage of Maple Cove: a wider single storey of clapboard under a\nsteep, brightly painted roof, a brick chimney and shutters, the bedroom\nwalled off at the back. Its door faces +z.",
          4.6, 3.4, 2.9, 0.6, [-2.6, 2.8], inner=[("z", -0.6, -4.4, 4.4, 2.6)])
    house("tavern", "The Anchor, the tavern on the landing: a long single storey of grey clapboard\nwith a red roof, a deep porch along its front looking out over the pier and\nthe sunset, a big stone hearth inside, a bar, barrels and long tables. Its\ndoor faces +z.",
          6.2, 4.6, 3.2, 1.0, [-3.6, 4.0], walls=[(186, 186, 180)], roofs=[(176, 44, 38)], porch=2.4, hearth=-3.5, tags=TAG_SHOP, door_w=0.7)
    house("inn", "The Harvest Moon Inn on the green: a long single storey of white clapboard\nunder a deep green roof, a porch the length of its front with rocking\nchairs, a common room with a hearth at one end and guest rooms along a\nhall. Its door faces +z.",
          8.6, 4.4, 3.0, -2.0, [-6.4, 1.0, 4.6], walls=[(242, 240, 232)], roofs=[(40, 96, 62)], porch=2.0, hearth=-6.0,
          inner=[("x", -0.6, -4.2, 4.2, 2.4), ("z", 0.8, -0.6, 2.6, 1.0), ("z", 0.8, 2.6, 5.6, 4.1), ("z", 0.8, 5.6, 8.4, 7.0), ("x", 2.6, -4.2, 0.8, -100), ("x", 5.6, -4.2, 0.8, -100)], tags=TAG_SHOP,
          anchors=['{ kind: "bed", at: [1.1, 0.05, -2.8] }', '{ kind: "bed", at: [4.1, 0.05, -2.8] }', '{ kind: "bed", at: [7.1, 0.05, -2.8] }'])
    house("general-store", "Dale's General Store: a single storey of butter-yellow clapboard under a\nblue roof, a porch out front with barrels and a bench, a big window full of\ngoods, the storekeepers' rooms at the back. Its door faces +z.",
          4.8, 4.0, 3.1, 0.0, [-2.6, 2.6], walls=[(236, 210, 140)], roofs=[(46, 80, 160)], porch=2.0, inner=[("z", -0.8, -4.6, 4.6, 3.4)], tags=TAG_SHOP)
    house("bakery", "The bakery: a single storey of rose-pink clapboard under a pumpkin-orange\nroof, its window stacked with loaves and pies, a brick oven glowing inside,\nthe baker's rooms at the back. Its door faces +z.",
          4.4, 3.6, 2.9, 0.6, [-2.2], walls=[(226, 176, 166)], roofs=[(214, 104, 38)], inner=[("z", -0.8, -4.2, 4.2, 2.6)], tags=TAG_SHOP)
    house("fish-shed", "The fish shed on the landing: a single storey of weathered grey cedar\nshingles under a teal roof, a wide door to the pier, lobster floats hung on\nits wall; the fish counter inside. Its door faces +z.",
          3.8, 3.0, 2.8, 0.6, [-2.0], walls=[(150, 146, 138)], roofs=[(36, 130, 130)], tags=TAG_SHOP, door_w=0.8, chimney=False)
    house("boat-shop", "The boat shop on the landing: a tall single storey of barn-red clapboard\nunder a white roof, wide doors to the water, inside a lobster boat on\ntrestles, a workbench and the smell of fresh oak. Its door faces +z.",
          5.0, 4.2, 3.6, 0.0, [-3.2, 3.2], walls=[(150, 50, 40)], roofs=[(232, 230, 222)], tags=TAG_SHOP, door_w=1.2, chimney=False)
    house("mayor-s-house", "The mayor's house at the head of the green: the biggest home in Maple Cove,\nstill one storey, white clapboard with black shutters under a plum roof, a\nwide porch with columns, a parlour with a hearth, a study and a bedroom\nwalled off behind. Its door faces +z.",
          6.0, 4.8, 3.3, 0.0, [-3.6, -1.8, 1.8, 3.6], walls=[(244, 242, 236)], roofs=[(124, 56, 104)], porch=2.4, hearth=-3.8,
          inner=[("z", -0.6, -5.8, 5.8, -1.0), ("x", 1.4, -4.6, -0.6, -2.0)], tags=TAG_SHOP)
    house("schoolhouse", "The little schoolhouse: one room of barn-red clapboard under a white roof\nwith a bell in a cupola on its ridge, rows of desks and a blackboard inside.\nIts door faces +z.",
          4.2, 3.4, 3.2, 0.0, [-2.4, 2.4], walls=[(156, 48, 38)], roofs=[(232, 230, 222)], tags=("building", "school"),
          extra_sdf="\n  const cup = min(box(x, y - 6.45, z, 0.5, 0.5, 0.5), max(sphere(x, y - 6.95, z, 0.55), 6.95 - y));", tall=7.6,
          extra_color="  if (y > 5.9 && abs(x) < 0.66 && abs(z) < 0.66) return y > 6.95 ? rgb(232, 230, 222) : (abs(y - 6.45) < 0.25 && abs(x) < 0.25 ? rgb(196, 160, 60) : rgb(244, 242, 236));\n")


if __name__ == "__main__":
    all_types()
