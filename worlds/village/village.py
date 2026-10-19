"""What stands in Marinella: a small walled harbour village, every house a
shop or a workshop with a home over it, each step of the hill a terrace."""
import math
import random

WALL_X = 46.0
WALL_N = 58.0
GATE_X = 27.0
RAMP_X = -41.0
CHURCH = (-14.0, 46.0)
TOWER = (41.0, 53.0)

# Rows of buildings along the back of each level: (centre z, floor height).
ROWS = {"quay": (-15.2, 1.8), "street": (4.8, 4.8), "piazza": (30.8, 7.8), "upper": (52.0, 10.8)}
# Stone stairs up the front of each level: (x, z of the upper level's edge).
STAIRS = [(-13.5, -10), (22.5, -10), (-4.5, 10), (31.5, 10), (18.0, 36)]

# Every building: (business key, shell type, x, row). Its people live over it.
BUILDINGS = [
    ("cantiere", "shop house", -31.5, "quay"),
    ("pescheria", "shop house", -22.5, "quay"),
    ("osteria", "shop house", -4.5, "quay"),
    ("gelateria", "shop house", 4.5, "quay"),
    ("capitaneria", "shop house", 13.5, "quay"),
    ("bar", "shop house", 31.5, "quay"),
    ("farmacia", "shop house", -31.5, "street"),
    ("macelleria", "shop house", -22.5, "street"),
    ("alimentari", "shop house", -13.5, "street"),
    ("pizzeria", "shop house", 4.5, "street"),
    ("barbiere", "shop house", 13.5, "street"),
    ("veterinario", "shop house", 22.5, "street"),
    ("libreria", "shop house", 40.5, "street"),
    ("panetteria", "shop house", -31.5, "piazza"),
    ("sartoria", "shop house", -22.5, "piazza"),
    ("albergo", "hotel", 3.0, "piazza"),
    ("caffe", "shop house", 27.0, "piazza"),
    ("fioraio", "shop house", 36.0, "piazza"),
    ("scuola", "shop house", 4.5, "upper"),
    ("canonica", "shop house", 13.5, "upper"),
]
SHELL = {"shop house": (3.8, 4.4, 1.9, -1.5), "hotel": (6.6, 4.8, 2.6, -2.6)}   # W, D, door x, window x
G, U = 3.4, 3.0


def where(key):
    """A building's (x, z, floor height, W, D, door x, window x)."""
    for k, ty, x, row in BUILDINGS:
        if k == key:
            z, h = ROWS[row]
            W, D, dx, wx = SHELL[ty]
            if ty == "hotel":
                z -= 0.4
            return x, z, h, W, D, dx, wx
    raise KeyError(key)


def to_world(key, lx, lz, ly=0.0):
    """A point in a building's own frame (front +z) in the world. Buildings
    face the sea (south), so their frame is turned half round."""
    x, z, h, *_ = where(key)
    return x - lx, z - lz, h + ly


def door_of(key):
    x, z, h, W, D, dx, wx = where(key)
    return x - dx, z - D - 1.2, h


def buildings(p):
    for key, ty, x, row in BUILDINGS:
        bx, bz, h, W, D, dx, wx = where(key)
        p.put(ty, bx, bz, 180, y=h)
        if key != "canonica":
            sx, sz, sy = to_world(key, (dx + wx) / 2, D + 0.12, 2.9)
            p.put(f"{key} sign", sx, sz, 180, y=sy)


def town(p, terrace_h, rnd):
    p.put("church of san pietro", CHURCH[0], CHURCH[1], 180, y=10.8)
    p.put("church steps", CHURCH[0], 36 - 1.8, 180, y=7.8)
    for x, z in STAIRS:
        p.put("stone stairs", x, z - 1.8, 180, y=terrace_h(z + 1) - 3.0)
    # The piazza: the fountain and benches round it, the café's tables, the market.
    p.put("piazza fountain", 6.0, 18.0, 0, y=7.8)
    for a in range(4):
        ang = a * math.pi / 2 + math.pi / 4
        p.put("stone bench", 6.0 + math.sin(ang) * 5.2, 18.0 + math.cos(ang) * 5.2, math.degrees(ang), y=7.8)
    for x in (22.5, 27.0, 31.5):
        p.put("cafe table", x, 22.6, 0, y=7.8)
    for z in (13.5, 18.0, 22.5):
        p.put("market stall", -30.0, z, 90, y=7.8)
    for x, z in ((-20, 12), (-6, 25), (16, 12), (40, 12), (-34, 25)):
        p.put("street lantern", x, z, 0, y=7.8)
    for x, z in ((-12, 24.5), (12, 24.5), (-26, 25)):
        p.put("lemon tree", x, z, 0, y=7.8)
    # Lanterns along the street and the upper lane.
    for x in (-36, -27, -18, 0, 9, 18, 27, 36):
        p.put("street lantern", x + 0.5, -8.6, 0, y=4.8)
    for x in (-2, 8, 22, 34):
        p.put("street lantern", x, 37.4, 0, y=10.8)


def dress(p, rnd):
    """Flowers at the doors, bougainvillea up some fronts, washing out."""
    for i, (key, ty, x, row) in enumerate(BUILDINGS):
        bx, bz, h, W, D, dx, wx = where(key)
        front = bz - D
        r = rnd.random()
        if r < 0.4:
            p.put("bougainvillea", bx + W - 0.9, front - 0.3, 180, y=h)
        elif r < 0.75:
            p.put("potted geraniums", bx - dx - 1.0, front - 0.45, 180, y=h)
    for x, z, h in ((-36.5, -24.0, 1.8), (36.5, -2.0, 4.8), (-28, 44, 10.8)):
        p.put("laundry line", x, z, 90, y=h)


def balustrades(p, terrace_h):
    """Along the front edge of every terrace, so no one walks off its wall:
    broken by the stairs, the ramp and the church steps."""
    for z0 in (-10, 10, 36):
        cuts = [(sx - 1.6, sx + 1.6) for sx, sz in STAIRS if sz == z0]
        if z0 == 36:
            cuts.append((CHURCH[0] - 4.2, CHURCH[0] + 4.2))
        cuts.sort()
        edges, x = [], RAMP_X + 3.6
        for a, b in cuts:
            edges.append((x, a))
            x = b
        edges.append((x, WALL_X - 0.9))
        for a, b in edges:
            n = int((b - a) // 4.0)
            if n < 1:
                continue
            left = (a + b) / 2 - n * 2.0
            for i in range(n):
                p.put("stone balustrade", left + 2.0 + 4.0 * i, z0 + 0.3, 0, y=terrace_h(z0 + 1))


def walls(p, terrace_h):
    p.put("town gate", GATE_X, WALL_N, 0, y=10.8)
    x = GATE_X + 5.5 + 4.0
    while x < WALL_X:
        p.put("town wall", x, WALL_N, 0, y=10.8)
        x += 8.0
    x = GATE_X - 5.5 - 4.0
    while x > -WALL_X:
        p.put("town wall", x, WALL_N, 0, y=10.8)
        x -= 8.0
    for side in (-1, 1):
        z = 50.0
        while z > -10:
            top = max(terrace_h(z - 3.9), terrace_h(z + 3.9))
            p.put("town wall", side * WALL_X, z, 90, y=top)
            z -= 8.0
    p.put("watchtower", TOWER[0], TOWER[1], 180, y=10.8)


def harbour(p):
    for x in range(-36, 37, 8):
        if all(abs(x - s) > 3 for s in (-24, 24)):
            p.put("bollard", x, -29.0, 0, y=1.8)
    boats = [(-34, -33.5, 90), (-14, -33.5, 270), (-4, -34, 90), (10, -33.5, 270), (32, -34, 90), (-18, -50, 200), (14, -58, 160)]
    for x, z, face in boats:
        p.put("fishing boat", x, z, face, y=0.0)
    for x, z in ((-25, -21.5), (-19, -22), (6, -22)):
        p.put("fish crates", x, z, 30 * (x % 7), y=1.8)
    for x, z in ((-28, -22), (-8, -27.5)):
        p.put("lobster pots", x, z, 90, y=1.8)
    p.put("net rack", -34.0, -24.0, 0, y=1.8)
    for x in (-10.0, 18.0):
        p.put("stone bench", x, -27.6, 180, y=1.8)
    for x in range(-32, 33, 16):
        p.put("street lantern", x, -28.2, 180, y=1.8)
    p.put("harbour light", 10, -84, 0, y=2.2)
    # Steps down into the water at either end of the quay: a way out for
    # anyone who goes in.
    for x in (-24.0, 24.0):
        p.put("stone stairs", x, -29.3 - 1.8, 180, y=1.8 - 3.0)


def cape(p, cape_h):
    p.put("cape lighthouse", 240, -184, 180, y=cape_h)


def lay(p, terrace_h, clearing, cape_h):
    rnd = random.Random(11)
    buildings(p)
    dress(p, rnd)
    town(p, terrace_h, rnd)
    balustrades(p, terrace_h)
    walls(p, terrace_h)
    harbour(p)
    cape(p, cape_h)
