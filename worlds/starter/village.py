"""What stands in Maple Cove: a loose village of single-storey cottages with
bright roofs round a green and along three lanes, its businesses on the
green and on the landing, and the wooden pier out into the cove.

Plan metres, before the world's quarter turn (see make.py): +x along the
shore, +z inland, the sea to the south. A bearing is the way a thing's
front looks: 0 inland, 90 along +x, 180 out to sea."""
import math
import random

SPAWN = (0.0, 24.5)
LANDING_H = 1.8
PIER_X = 4.0
GREEN = (0.0, 34.0)

# The type's half width, half depth, door x, porch depth.
SHAPE = {
    "cottage": (3.6, 3.0, 0.9, 0.0), "family cottage": (4.6, 3.4, 0.6, 0.0), "tavern": (6.2, 4.6, 1.0, 2.4),
    "inn": (8.6, 4.4, -2.0, 2.0), "general store": (4.8, 4.0, 0.0, 2.0), "bakery": (4.4, 3.6, 0.6, 0.0),
    "fish shed": (3.8, 3.0, 0.6, 0.0), "boat shop": (5.0, 4.2, 0.0, 0.0), "mayor's house": (6.0, 4.8, 0.0, 2.4),
    "schoolhouse": (4.2, 3.4, 0.0, 0.0), "smithy": (3.8, 3.2, 0.0, 0.0),
}

# The businesses: (key, type, x, z, bearing). On the landing they stand at
# its height; the rest on their own level ground.
BUSINESSES = [
    ("tavern", "tavern", -20.0, -16.5, 180),
    ("fishshed", "fish shed", 15.0, -17.0, 270),
    ("boatshop", "boat shop", 31.0, -16.0, 180),
    ("inn", "inn", 0.0, 57.0, 180),
    ("store", "general store", -31.0, 36.0, 90),
    ("bakery", "bakery", -29.0, 15.0, 90),
    ("mayor", "mayor's house", 32.0, 40.0, 270),
    ("smithy", "smithy", 30.0, 16.0, 270),
    ("school", "schoolhouse", -24.0, 66.0, 150),
]
ON_LANDING = {"tavern", "fishshed", "boatshop"}

# The lanes out from the green, along the bluff west and east and up north
# toward the woods. Cottages stand along them, doors to the lane.
LANES = {
    "west": [(-22.0, 26.0), (-50.0, 22.0), (-78.0, 15.0), (-104.0, 9.0), (-126.0, 5.0)],
    "east": [(22.0, 26.0), (50.0, 24.0), (78.0, 19.0), (104.0, 14.0), (124.0, 12.0)],
    "north": [(22.0, 50.0), (26.0, 72.0), (30.0, 112.0)],
}
# Along each lane: (how far along, side +1 left / -1 right, type, key).
HOMES = {
    "west": [(34, -1, "cottage", "w1"), (44, 1, "family cottage", "w2"), (56, -1, "cottage", "w3"), (73, 1, "cottage", "w4"),
             (82, -1, "family cottage", "w5"), (100, 1, "cottage", "w6")],
    "east": [(34, 1, "cottage", "e1"), (44, -1, "cottage", "e2"), (58, 1, "family cottage", "e3"), (72, -1, "cottage", "e4"),
             (86, 1, "cottage", "e5"), (98, -1, "family cottage", "e6")],
    "north": [(28, 1, "cottage", "n1"), (40, -1, "cottage", "n2"), (54, 1, "cottage", "n3")],
}

PLACED = {}      # key: (x, z, bearing, floor height, type)


def along(pts, d):
    """The point d metres along a polyline, and the lane's direction there."""
    for (ax, az), (bx, bz) in zip(pts, pts[1:]):
        L = math.hypot(bx - ax, bz - az)
        if d <= L:
            t = d / L
            return ax + (bx - ax) * t, az + (bz - az) * t, (bx - ax) / L, (bz - az) / L
        d -= L
    (ax, az), (bx, bz) = pts[-2], pts[-1]
    L = math.hypot(bx - ax, bz - az)
    return bx, bz, (bx - ax) / L, (bz - az) / L


def local(key, lx, lz):
    """A point in a building's own frame (front +z) in plan metres."""
    x, z, b, h, ty = PLACED[key]
    r = math.radians(b)
    return x + lx * math.cos(r) + lz * math.sin(r), z - lx * math.sin(r) + lz * math.cos(r)


def door_of(key, out=1.0):
    x, z, b, h, ty = PLACED[key]
    W, D, dx, porch = SHAPE[ty]
    px, pz = local(key, dx, D + porch + out)
    return px, pz, h + (0.2 if porch else 0.0)


def yard(key):
    """How far a building's level yard reaches past its walls: (sides,
    front, back). A cottage's holds its fenced front garden and the
    vegetable patch behind."""
    ty = PLACED[key][4]
    if ty in ("cottage", "family cottage"):
        return 1.5, 4.6, 6.4
    return 1.5, 1.5 + SHAPE[ty][3], 1.5


def footprint(key):
    x, z, b, h, ty = PLACED[key]
    W, D, dx, porch = SHAPE[ty]
    gs, gf, gb = yard(key)
    return [local(key, sx * (W + gs), D + gf if sz > 0 else -D - gb) for sx, sz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]


YARD_SIDES = 3.5


def ground(nat, x, z):
    """The ground's height at (x, z) once the yards and the green are laid:
    level on them, easing back to the land over their soft sides."""
    h = nat(x, z)
    best = None
    for key, (bx, bz, b, bh, ty) in PLACED.items():
        if key in ON_LANDING:
            continue
        W, D, dx, porch = SHAPE[ty]
        gs, gf, gb = yard(key)
        r = math.radians(b)
        lx = (x - bx) * math.cos(r) - (z - bz) * math.sin(r)
        lz = (x - bx) * math.sin(r) + (z - bz) * math.cos(r)
        dxo = max(abs(lx) - (W + gs), 0.0)
        dzo = max(lz - (D + gf), -lz - (D + gb), 0.0)
        d = math.hypot(dxo, dzo)
        if d < YARD_SIDES and (best is None or d < best[0]):
            best = (d, bh)
    gx, gz = GREEN
    d = math.hypot(max(abs(x - gx) - 21, 0.0), max(z - (gz + 14), gz - 13 - z, 0.0))
    if d < 6.0 and (best is None or d / 6.0 < best[0] / YARD_SIDES):
        best = (d * YARD_SIDES / 6.0, nat(gx, gz))
    if best is None:
        return h
    t = best[0] / YARD_SIDES
    return best[1] + (h - best[1]) * t


def homes():
    """Every cottage along its lane: a little off the lane, a little turned,
    its door to the lane."""
    rnd = random.Random(5)
    out = []
    for lane, spots in HOMES.items():
        for d, side, ty, key in spots:
            x, z, ux, uz = along(LANES[lane], d)
            off = 9.5 + rnd.uniform(-1.5, 2.0) + (1.0 if ty == "family cottage" else 0.0)
            hx, hz = x - uz * off * side, z + ux * off * side
            b = math.degrees(math.atan2(x - hx, z - hz)) + rnd.uniform(-12, 12)
            out.append((key, ty, hx, hz, b))
    return out


def place_all(nat):
    for key, ty, x, z, b in BUSINESSES + homes():
        h = LANDING_H if key in ON_LANDING else round(nat(x, z), 2)
        PLACED[key] = (x, z, b % 360, h, ty)


def buildings(p, signs):
    for key, (x, z, b, h, ty) in PLACED.items():
        p.put(ty, x, z, b, y=h)
        if key not in ON_LANDING:
            p.area("yard", footprint(key), "level", height=h, sides=YARD_SIDES)
        if key in signs:
            W, D, dx, porch = SHAPE[ty]
            if porch:
                sx, sz = local(key, dx, D + porch + 0.42)
                sy = h + {"tavern": 3.2, "inn": 3.0, "general store": 3.1, "mayor's house": 3.3}[ty] - 0.95
            elif ty == "smithy":
                sx, sz = local(key, 0.0, 3.3)
                sy = h + 3.15
            else:
                sx, sz = local(key, dx, D + 0.12)
                sy = h + 2.28
            p.put(f"{key} sign", sx, sz, b, y=sy)


def dress(p, rnd):
    """Pumpkins and mums by the doors, picket fences, a garden behind."""
    for i, (key, (x, z, b, h, ty)) in enumerate(sorted(PLACED.items())):
        if ty not in ("cottage", "family cottage", "bakery", "general store", "inn", "mayor's house"):
            continue
        W, D, dx, porch = SHAPE[ty]
        r = rnd.random()
        if r < 0.55:
            p.put("pumpkins", *local(key, dx + 1.6, D + porch + 0.8), b + rnd.uniform(-20, 20), y=h + (0.2 if porch else 0.0))
        if r > 0.35:
            p.put("pots of mums", *local(key, dx - 1.5, D + porch + 0.5), b, y=h + (0.2 if porch else 0.0))
        if ty in ("cottage", "family cottage"):
            if rnd.random() < 0.6:
                # Fence along the front garden, a gap for the path.
                for fx in (-W + 0.9, dx - 2.0, dx + 2.0, W - 0.9):
                    if abs(fx - dx) > 1.0:
                        p.put("picket fence", *local(key, fx, D + 3.6), b, y=h)
            if rnd.random() < 0.5:
                p.put("vegetable garden", *local(key, rnd.uniform(-1.5, 1.5), -D - 3.2), b + 90, y=h)
    for key in ("inn", "mayor"):
        x, z, b, h, ty = PLACED[key]
        W, D, dx, porch = SHAPE[ty]
        for fx in (-W + 1.2, -W + 2.4, W - 1.2):
            p.put("rocking chair", *local(key, fx, D + 0.9), b + rnd.uniform(-15, 15), y=h + 0.2)
    x, z, b, h, ty = PLACED["store"]
    for fx in (-3.6, -2.7, 3.2):
        p.put("apple barrel", *local("store", fx, 5.2), b, y=h + 0.2)
    p.put("park bench", *local("store", 2.0, 4.6), b, y=h + 0.2)


def green(p, gh, rnd):
    gx, gz = GREEN
    p.put("gazebo", gx, gz, 180, y=gh)
    p.put("water pump", gx + 13.0, gz - 8.0, 200, y=gh)
    for a in (45, 135, 225, 315):
        r = math.radians(a)
        p.put("park bench", gx + math.sin(r) * 9.5, gz + math.cos(r) * 7.5, a + 180, y=gh)
    for x, z in ((-17, 22), (17, 22), (-17, 47), (17, 47), (-6, 21), (6, 21)):
        p.put("street lantern", gx + x, z, 0, y=gh)
    p.put("hay bale", gx - 15.5, gz + 9.0, 30, y=gh)
    p.put("hay bale", gx - 13.6, gz + 11.0, 75, y=gh)
    p.put("pumpkins", gx - 12.2, gz + 8.8, 160, y=gh)
    p.put("scarecrow", gx - 15.0, gz + 11.6, 160, y=gh)
    # Set out for the harvest supper: two long tables north of the gazebo.
    for x in (-6.5, 6.5):
        p.put("harvest table", gx + x, gz + 9.5, 0, y=gh)
    # Flowers round the gazebo and along the green's edges, pumpkins by its
    # steps, leaves raked into piles under the maples.
    for i in range(10):
        a = math.radians(i * 36 + 18)
        if abs(math.degrees(a) - 180) > 25:
            p.put("autumn wildflowers", gx + math.sin(a) * 5.4, gz + math.cos(a) * 5.4, i * 50, y=gh)
    for x in range(-18, 19, 4):
        for z in (gz - 11.5, gz + 12.5):
            if rnd.random() < 0.7 and abs(x) > 3:
                p.put("autumn wildflowers", gx + x + rnd.uniform(-1, 1), z + rnd.uniform(-0.5, 0.5), rnd.uniform(0, 360), y=gh)
    for z in range(int(gz - 9), int(gz + 11), 4):
        for x in (gx - 19.0, gx + 19.0):
            if rnd.random() < 0.6:
                p.put("autumn wildflowers", x + rnd.uniform(-0.8, 0.8), z, rnd.uniform(0, 360), y=gh)
    for x in (-2.2, 2.2):
        p.put("pumpkins", gx + x, gz - 4.6, 180 + x * 20, y=gh)
    for x, z in ((-17.5, -10.0), (16.0, -9.0), (17.0, 11.0), (-9.0, 12.0)):
        p.put("leaf pile", gx + x, gz + z, rnd.uniform(0, 360), y=gh)
    for i in range(10):
        p.put("fallen leaves", gx + rnd.uniform(-18, 18), gz + rnd.choice((-1, 1)) * rnd.uniform(6, 12), rnd.uniform(0, 360), y=gh)
    for x, z in ((-19.0, -1.0), (19.5, 4.0), (-19.5, 9.0)):
        p.put("blueberry bush", gx + x, gz + z, rnd.uniform(0, 360), y=gh)


def lanes(p, nat, rnd):
    for lane, pts in LANES.items():
        L = sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip(pts, pts[1:]))
        d = 12.0
        while d < L - 4:
            x, z, ux, uz = along(pts, d)
            side = 1 if int(d / 22) % 2 else -1
            lx, lz = x - uz * 2.8 * side, z + ux * 2.8 * side
            p.put("street lantern", lx, lz, math.degrees(math.atan2(-uz * side, ux * side)) - 90, y=round(ground(nat, lx, lz), 2) - 0.05)
            d += 22.0


def landing(p):
    h = LANDING_H
    # The pier: sections from the landing's edge out over the cove, its head.
    z = -31.0 - 3.0
    while z > -66.0:
        p.put("pier section", PIER_X, z, 0, y=h - 7.8)
        z -= 6.0
    p.put("pier head", PIER_X, z - 1.0, 0, y=h - 7.8)
    # Lobster boats and dories tied alongside, one out on the water.
    for x, z, b, ty in ((PIER_X - 3.3, -40.0, 180, "fishing boat"), (PIER_X + 3.3, -47.0, 0, "fishing boat"), (PIER_X - 3.2, -56.0, 175, "dory"),
                        (PIER_X + 3.2, -60.0, 5, "dory"), (PIER_X + 8.0, -76.0, 300, "fishing boat"), (-16.0, -45.0, 40, "fishing boat"), (-22.0, -36.0, 120, "dory")):
        p.put(ty, x, z, b, y=0.0)
    for x in range(-40, 41, 8):
        if abs(x - PIER_X) > 3:
            p.put("bollard", x, -30.2, 0, y=h)
    for x, z in ((19.5, -22.0), (21.0, -24.0), (10.5, -21.5)):
        p.put("fish crates", x, z, 30 * (x % 7), y=h)
    for x, z in ((24.0, -21.5), (12.0, -25.0), (-4.0, -26.0)):
        p.put("lobster pots", x, z, 90, y=h)
    p.put("net rack", 38.0, -23.0, 0, y=h)
    for x in (-32.0, -8.0, 26.0):
        p.put("park bench", x, -28.4, 180, y=h)
    for x in (-38, -12, 14, 40):
        p.put("street lantern", x, -29.0, 180, y=h)
    # Ladders up the quay's face at either end: a way out for anyone who
    # falls in.
    for x in (-38.0, 38.0):
        p.put("quay ladder", x, -31.45, 180, y=h - 3.0)
    p.put("dory", 30.0, -24.0, 80, y=h)


def trees(p, nat, rnd):
    """Maples, oaks and birches along the lanes and round the green, and an
    apple tree in some yards: the village's autumn colour."""
    taken = [(x, z, max(SHAPE[ty][0], SHAPE[ty][1]) + SHAPE[ty][3] + 3.0) for x, z, b, h, ty in PLACED.values()]
    taken += [(GREEN[0], GREEN[1], 6.0), (PIER_X, -20.0, 6.0)]
    kinds = ["sugar maple"] * 5 + ["red oak"] * 2 + ["paper birch"] * 3 + ["white pine"]

    def free(x, z, r):
        return all(math.hypot(x - a, z - b) > r + c for a, b, c in taken)

    def plant(ty, x, z, r=3.5):
        if z < -6 or not free(x, z, r):
            return
        taken.append((x, z, r))
        p.put(ty, x, z, rnd.uniform(0, 360), y=round(ground(nat, x, z), 2) - 0.25)

    for lane, pts in LANES.items():
        L = sum(math.hypot(b[0] - a[0], b[1] - a[1]) for a, b in zip(pts, pts[1:]))
        d = 6.0
        while d < L:
            x, z, ux, uz = along(pts, d)
            side = rnd.choice((-1, 1))
            off = 5.5 + rnd.uniform(0, 3)
            plant(rnd.choice(kinds), x - uz * off * side, z + ux * off * side)
            d += 11.0 + rnd.uniform(-2, 4)
    # Round the green: a big maple at each corner, more along its sides.
    gx, gz = GREEN
    for x, z in ((-25, 17), (25, 17), (-25, 52), (25, 52), (-25, 34), (25, 30), (-8, 17), (9, 17)):
        plant("sugar maple" if abs(x) > 20 else rnd.choice(kinds), gx + x, z)
    # Groves behind the houses, up the slope.
    for cx, cz, n in ((-60, 50, 7), (-100, 40, 6), (60, 50, 7), (100, 44, 6), (-10, 86, 8), (50, 92, 6), (-130, 26, 4), (130, 30, 4)):
        for i in range(n * 3):
            if n <= 0:
                break
            x, z = cx + rnd.uniform(-16, 16), cz + rnd.uniform(-12, 12)
            before = len(taken)
            plant(rnd.choice(kinds), x, z, 3.0)
            n -= len(taken) - before
    # Apple trees in some yards.
    for key, (x, z, b, h, ty) in sorted(PLACED.items()):
        if ty in ("cottage", "family cottage") and rnd.random() < 0.45:
            ax, az = local(key, rnd.choice((-1, 1)) * (SHAPE[ty][0] + 3.5), -1.0)
            plant("apple tree", ax, az, 2.5)


def cape(p, cape_h, cross_h):
    p.put("cape lighthouse", 240, -184, 180, y=cape_h)
    p.put("crossroads signpost", 38.5, 205.5, 0, y=cross_h)


def lay(p, nat, cape_h, cross_h):
    rnd = random.Random(11)
    place_all(nat)
    import signs
    buildings(p, signs.SIGNS)
    dress(p, rnd)
    green(p, round(nat(*GREEN), 2), rnd)
    lanes(p, nat, rnd)
    landing(p)
    trees(p, nat, rnd)
    cape(p, cape_h, cross_h)
