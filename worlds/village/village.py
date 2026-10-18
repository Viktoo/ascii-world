"""What stands in Marinella and Boscoverde, and who lives there."""
import math
import random

WALL_X = 92.0
WALL_N = 108.0
RAMPS = (-74.0, 76.0)

# Stairs up from one terrace to the next: (x, z of the upper terrace's edge).
STAIRS = [(-36, -12), (34, -12), (-20, 12), (40, 12), (-30, 34), (22, 34), (30, 80), (-50, 80)]
CHURCH = (-10.0, 70.0)
PALAZZI = [(-40.0, 46.0, 90), (42.0, 46.0, 270), (20.0, 52.5, 180)]
TOWER = (90.0, 106.0)


def houses(p, terrace_h, rnd):
    """Rows of painted houses along the back of each terrace, their doors on
    the lane in front, alleys and stairs between. Returns where each stands."""
    rows = [12, 34, 58, 80, 106]
    out = []
    for back in rows:
        z = back - 4.6
        h = terrace_h(z)
        x = -WALL_X + 6.0
        while x < WALL_X - 5.0:
            w = 7.6 + rnd.uniform(0, 1.6)
            cx = x + w / 2
            if cx + 4.5 > WALL_X - 0.9:
                break
            clear = all(abs(cx - r) > 8.5 for r in RAMPS)
            clear = clear and all(not (sz == back and abs(cx - sx) < 6.0) for sx, sz in STAIRS)
            if back == 58 and abs(cx) < 50:
                clear = False   # the piazza
            if back == 80 and abs(cx - CHURCH[0]) < 16:
                clear = False   # the church
            if back == 106 and (abs(cx) < 8 or cx > 78):
                clear = False   # the gate, the watchtower
            if clear:
                p.put("painted house", cx, z, 180, y=h)
                out.append((cx, z, h))
            x += w + rnd.uniform(1.0, 2.4)
    return out


def dress_houses(p, hs, rnd):
    """Flowers and pots at the doors, bougainvillea up some fronts."""
    for i, (cx, z, h) in enumerate(hs):
        front = z - 4.0
        r = rnd.random()
        if r < 0.35:
            p.put("bougainvillea", cx - 2.0, front - 0.3, 180, y=h)
        elif r < 0.7:
            p.put("potted geraniums", cx + 2.6, front - 0.45, 180, y=h)
        elif r < 0.85:
            p.put("lemon tree", cx - 1.6, front - 0.8, 180, y=h)


def town(p, terrace_h, rnd):
    # The church above the piazza, its steps down to it, the palazzi round it.
    p.put("church of san pietro", CHURCH[0], CHURCH[1], 180, y=terrace_h(CHURCH[1]))
    p.put("church steps", CHURCH[0], 58 - 1.8, 180, y=13.6 - 3.0)
    for x, z, face in PALAZZI:
        p.put("palazzo", x, z, face, y=10.6)
    for x, z in STAIRS:
        p.put("stone stairs", x, z - 1.8, 180, y=terrace_h(z + 1) - 3.0)
    # The piazza: the fountain, benches round it, the café, the market.
    p.put("piazza fountain", 0, 43, 0, y=10.6)
    for a in range(4):
        ang = a * math.pi / 2 + math.pi / 4
        p.put("stone bench", math.sin(ang) * 5.2, 43 + math.cos(ang) * 5.2, math.degrees(ang), y=10.6)
    for i, z in enumerate((39.5, 44.5, 49.5)):
        p.put("cafe table", 33.5, z, 0, y=10.6)
    for x in (14.0, 20.0, 26.0):
        p.put("cafe table", x, 43.5, 0, y=10.6)
    p.put("bread oven", 27.0, 45.6, 180, y=10.6)
    for z in (39.0, 44.0, 49.0):
        p.put("market stall", -30.5, z, 90, y=10.6)
    for x, z in ((-34, 37), (36, 37), (-16, 54), (-4, 54), (8, 37)):
        p.put("street lantern", x, z, 180, y=10.6)
    for x, z in ((-22, 37), (-8, 37), (28, 55.5)):
        p.put("lemon tree", x, z, 0, y=10.6)
    # Lanterns along every lane, clear of the stairs and ramps.
    for z0 in (-12, 12, 58, 80):
        for x in range(-84, 85, 21):
            if any(abs(x - sx) < 4 for sx, _ in STAIRS) or any(abs(x - r) < 5 for r in RAMPS):
                continue
            if z0 == 58 and abs(x - CHURCH[0]) < 10:
                continue
            p.put("street lantern", x + 0.5, z0 + 1.4, 0, y=terrace_h(z0 + 1))
    # Washing out to dry here and there along the terraces' fronts.
    for x, z0 in ((-58, -12), (52, -12), (-46, 12), (8, 12), (62, 12), (-58, 34), (60, 58), (-36, 80), (52, 80)):
        p.put("laundry line", x, z0 + 2.2, 0, y=terrace_h(z0 + 1))


def balustrades(p, terrace_h):
    """Along the front edge of every terrace, so no one walks off its wall:
    broken only by the stairs, the ramps and the church steps."""
    for z0 in (-12, 12, 34, 58, 80):
        cuts = [(sx - 1.6, sx + 1.6) for sx, sz in STAIRS if sz == z0]
        cuts += [(r - 3.4, r + 3.4) for r in RAMPS]
        if z0 == 58:
            cuts.append((CHURCH[0] - 4.2, CHURCH[0] + 4.2))
        cuts.sort()
        edges, x = [], -WALL_X + 0.9
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
    # The north wall either side of the gate, the side walls down to the quay,
    # the watchtower in the north-east corner.
    p.put("town gate", 0, WALL_N, 0, y=16.6)
    for side in (-1, 1):
        x = 5.5 + 4.0
        while x < WALL_X:
            if not (side > 0 and x > TOWER[0] - 6):
                p.put("town wall", side * x, WALL_N, 0, y=16.6)
            x += 8.0
        z = 103.0 if side < 0 else 99.0
        while z > -10:
            top = max(terrace_h(z - 3.9), terrace_h(z + 3.9))
            p.put("town wall", side * WALL_X, z, 90, y=top)
            z -= 8.0
    p.put("watchtower", TOWER[0], TOWER[1], 180, y=16.6)


def harbour(p):
    # Bollards along the quay's edge, boats moored at it, the fishermen's
    # things on the stones, benches looking out over the water.
    for x in range(-60, 61, 10):
        p.put("bollard", x, -25.0, 0, y=1.8)
    boats = [(-52, -30.5, 90), (-41, -30.5, 270), (-26, -30.5, 90), (-6, -31, 90), (12, -30.5, 270), (30, -30.5, 90), (48, -31, 270),
             (-20, -48, 200), (22, -56, 160), (-120, -60, 40)]
    for x, z, face in boats:
        p.put("fishing boat", x, z, face, y=0.0)
    for x, z in ((-48, -18), (-12, -19), (18, -17), (44, -19), (62, -18)):
        p.put("fish crates", x, z, 30 * (x % 7), y=1.8)
    for x, z in ((-56, -17), (6, -18), (54, -17)):
        p.put("lobster pots", x, z, 90, y=1.8)
    for x, z in ((-30, -20), (26, -20), (-66, -20)):
        p.put("net rack", x, z, 0, y=1.8)
    for x in (-44, -16, 2, 40):
        p.put("stone bench", x, -23.6, 180, y=1.8)
    for x in range(-70, 71, 20):
        p.put("street lantern", x, -24.2, 180, y=1.8)
    p.put("harbour light", 18, -94, 0, y=2.2)
    # Steps down into the water at either end of the quay: a way out for
    # anyone who goes in.
    for x in (-58.0, 0.0, 58.0):
        p.put("stone stairs", x, -26.0 - 1.8, 180, y=1.8 - 3.0)


def cape(p, cape_h):
    p.put("cape lighthouse", 240, -184, 180, y=cape_h)


def lay(p, terrace_h, clearing, cape_h):
    rnd = random.Random(11)
    hs = houses(p, terrace_h, rnd)
    dress_houses(p, hs, rnd)
    town(p, terrace_h, rnd)
    balustrades(p, terrace_h)
    walls(p, terrace_h)
    harbour(p)
    cape(p, cape_h)
