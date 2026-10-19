#!/usr/bin/env python3
"""Marinella's answers to the builder, written from one place.

A Mediterranean fishing village of painted houses climbing its hill in
terraces above a stone harbour, walled on the landward side with a gate and a
watchtower; meadows of wildflowers and tall grass round it, olive hills, and a
chestnut wood up the slope where the woodcutters' hamlet of Boscoverde stands
in its clearing. World metres: +x east, +z north, the piazza at (0, 40).

Run it (it builds the land once with `pocket author --genesis-only` to read the
natural heights its edges blend into), then `pocket author worlds/village`.
"""
import json
import math
import os
import random
import subprocess

import hamlet
import people
import village

HERE = os.path.dirname(os.path.abspath(__file__))
REGION = 256.0
POCKET = os.path.join(HERE, "..", "..", "target", "release", "pocket")
WORK = os.path.join(HERE, ".work")


# ---------------------------------------------------------------------------
# The land's large shape.

def ring(cx, cz, r, n=18, wob=0.0, seed=1):
    rnd = random.Random(seed)
    return [[round(cx + math.cos(i / n * 6.2832) * r * (1 + rnd.uniform(-wob, wob)), 1),
             round(cz + math.sin(i / n * 6.2832) * r * (1 + rnd.uniform(-wob, wob)), 1)] for i in range(n)]


# The shore, west to east: a sandy cove, the village's harbour, the headland.
COAST = [[-3000, -70], [-900, -60], [-420, -58], [-240, -52], [-190, -40], [-150, -34], [-112, -30], [-96, -26],
         [-60, -24], [0, -24], [60, -24], [100, -26], [135, -32], [160, -60], [185, -130], [215, -205], [245, -215],
         [275, -170], [300, -95], [340, -60], [520, -58], [900, -64], [3000, -70]]
FOREST_AT = (130.0, 520.0)
CAPE = (240.0, -184.0)


def landforms():
    return [
        {"name": "the coast", "biome": "flowering meadows", "outline": [[-3000, -2500], [3000, -2500], [3000, 3000], [-3000, 3000]], "edge": 400},
        {"name": "the olive hills", "biome": "olive hills", "outline": ring(-560, 320, 330, 16, 0.2, 3), "edge": 160, "rise": 8, "rugged": 0.3},
        {"name": "the eastern olive hills", "biome": "olive hills", "outline": ring(600, 260, 260, 16, 0.2, 4), "edge": 160, "rise": 6, "rugged": 0.3},
        {"name": "the village hill", "biome": "flowering meadows", "outline": ring(0, 60, 150, 16, 0.08, 5), "edge": 120, "rise": 10},
        {"name": "the chestnut wood", "biome": "chestnut woods",
         "outline": ring(FOREST_AT[0], FOREST_AT[1] + 140, 300, 20, 0.15, 6), "edge": 110, "rise": 12, "rugged": 0.25},
        {"name": "the bay", "biome": "the sea", "outline": COAST + [[3000, -3000], [-3000, -3000]], "edge": 45},
        {"name": "Capo Faro", "biome": "limestone headland",
         "outline": [[140, -30], [170, -110], [215, -205], [262, -196], [300, -100], [320, -40], [230, 10]], "edge": 30, "rise": 12, "rugged": 0.5},
    ]


# ---------------------------------------------------------------------------
# The village's ground: terraces stepping up from the quay.

COBBLE = (166, 148, 126)
STONE = (198, 188, 166)
DIRT = (176, 158, 120)
GRAVEL = (194, 182, 156)
SAND = (232, 216, 174)
PAVING = (196, 166, 128)

# (south edge z, north edge z, height): the quay, then the terraces, each
# 3 m over the one below (a flight of stone stairs climbs each step).
TERRACES = [(-30, -10, 1.8), (-10, 10, 4.8), (10, 36, 7.8), (36, 58, 10.8)]
WALL_X = 46.0
WALL_N = 58.0


def terrace_h(z):
    for z0, z1, h in TERRACES:
        if z0 <= z < z1:
            return h
    return TERRACES[-1][2] if z >= TERRACES[-1][1] else TERRACES[0][2]


class Plan:
    def __init__(self):
        self.ground = []
        self.things = []
        self.people = []
        self.creatures = []
        self.items = []

    def line(self, what, pts, width, how, height=None, sides=None, color=None, rim=None, rim_width=None, name="", water=None, pattern=None, pattern_size=None):
        e = {"what": what, "name": name, "line": [[round(x, 2), round(z, 2)] for x, z in pts], "width": width, "how": how}
        if height is not None:
            e["height"] = height
        if sides is not None:
            e["sides"] = sides
        if color:
            e["color"] = list(color)
        if rim:
            e["rim"], e["rim_width"] = list(rim), rim_width or 0.6
        if water is not None:
            e["water"] = water
        if pattern:
            e["pattern"] = pattern
            if pattern_size:
                e["pattern_size"] = pattern_size
        self.ground.append(e)

    def area(self, what, pts, how, height=None, sides=None, color=None, rim=None, rim_width=None, name="", water=None, pattern=None, pattern_size=None):
        e = {"what": what, "name": name, "outline": [[round(x, 2), round(z, 2)] for x, z in pts], "how": how}
        if height is not None:
            e["height"] = height
        if sides is not None:
            e["sides"] = sides
        if color:
            e["color"] = list(color)
        if rim:
            e["rim"], e["rim_width"] = list(rim), rim_width or 0.6
        if water is not None:
            e["water"] = water
        if pattern:
            e["pattern"] = pattern
            if pattern_size:
                e["pattern_size"] = pattern_size
        self.ground.append(e)

    def rect(self, what, x0, z0, x1, z1, how, **kw):
        self.area(what, [(x0, z0), (x1, z0), (x1, z1), (x0, z1)], how, **kw)

    def item(self, ty, near, x, z):
        """A small loose thing lying by its person (`near`), at their home."""
        self.items.append((ty, near, x, z))

    def put(self, ty, x, z, face=180.0, y=None):
        """`face`: the bearing its front looks along (180: south, to the sea)."""
        self.things.append((ty, x, z, face % 360.0, y))


def natural_heights(points):
    out = subprocess.run([POCKET, "author", "heights", os.path.join(WORK, "land.pocket"), "-"],
                         input="\n".join(f"{x} {z}" for x, z in points), capture_output=True, text=True, check=True).stdout
    return [float(l.split()[2]) for l in out.splitlines()]


def build_land():
    os.makedirs(WORK, exist_ok=True)
    env = dict(os.environ, POCKET_HOME=WORK)
    subprocess.run([POCKET, "author", HERE, "--genesis-only", "--out", os.path.join(WORK, "land.pocket")], env=env, check=False, capture_output=True)


def lay_ground(p, nat):
    # Terraces, each a level shelf held by a stone wall at its foot.
    for z0, z1, h in TERRACES:
        quay = z0 < -20
        p.rect("terrace", -WALL_X - 4, z0, WALL_X + 4, z1, "level", height=h, sides=0.7, color=COBBLE if quay else PAVING,
               rim=STONE, rim_width=0.5, name="the quay" if quay else "", pattern="setts" if quay else "flagstones")
    # Behind the wall the hill comes down to the meadows, softly.
    p.rect("hill", -WALL_X - 4, 57, WALL_X + 4, 64, "level", height=10.8, sides=16)
    # The harbour: a basin dug to the sea bed, its quay wall sheer.
    p.rect("harbour", -40, -72, 40, -30, "cut", height=-3.5, sides=0.6, rim=STONE, rim_width=0.8, name="the harbour")
    p.line("breakwater", [(46, -28), (50, -54), (34, -76), (10, -84)], 7, "fill", height=2.2, sides=1.5, color=STONE, rim=(150, 146, 136), rim_width=1.0,
           name="the breakwater", pattern="setts", pattern_size=0.3)
    # The cove west of the village: a sandy beach shelving into the sea.
    for x in range(-205, -100, 15):
        p.line("beach", [(x, -18), (x, -32), (x, -48), (x, -70)], 20, "level", height=[3.0, 1.4, -0.2, -2.0], sides=6)
    p.rect("sand", -210, -70, -100, -16, "paint", color=SAND, name="Spiaggia della Conca")
    # Streets: a ramp up the west side for carts and scooters, the lanes, the piazza.
    p.line("ramp", [(-41, -24), (-41, -12), (-41, 44)], 5, "level", height=[1.8, 1.8, 10.8], sides=1.0, color=COBBLE, name="Via del Ponente", pattern="cobbles")
    p.rect("piazza", -36, 11, 44, 26, "paint", color=(196, 140, 108), name="Piazza del Mare", pattern="bricks")
    p.line("lane", [(-37, -6), (WALL_X - 2, -6)], 5.0, "paint", color=COBBLE, name="Via del Porto", pattern="cobbles")
    p.line("lane", [(-37, 40), (WALL_X - 2, 40)], 4.0, "paint", color=COBBLE, name="Via della Chiesa", pattern="cobbles")
    # Roads out of the gate: north to the woods, east and west along the coast,
    # down to the cove and out to the lighthouse on the cape.
    p.line("road", [(27, 60), (30, 110), (34, 210), (70, 300), (100, 400), (124, 458), (128, 492)], 4.0, "level", color=DIRT, name="the woodcutters' road")
    p.line("road", [(34, 210), (-90, 226), (-220, 240), (-380, 262)], 4.0, "level", color=DIRT, name="the coast road")
    p.line("road", [(34, 210), (150, 196), (290, 190), (420, 186)], 4.0, "level", color=DIRT, name="the coast road")
    p.line("path", [(-150, -14), (-158, 60), (-176, 150), (-200, 236)], 2.6, "level", color=GRAVEL, name="the cove path")
    p.line("path", [(250, 190), (262, 100), (250, 0), (238, -110), (236, -178)], 2.6, "level", color=GRAVEL, name="the lighthouse path")
    # The woodcutters' clearing, levelled, and their pond.
    fx, fz = FOREST_AT
    h = round(nat["clearing"], 2)
    p.area("clearing", ring(fx, fz, 30, 14, 0.12, 9), "level", height=h, sides=8, color=(122, 150, 70), name="Boscoverde")
    p.area("pond", ring(fx + 34, fz + 18, 9, 12, 0.15, 10), "cut", height=h - 2.0, sides=2.0, water=h - 0.5, name="the woodcutters' pond")
    # The lighthouse's level on the ridge of the cape.
    p.area("cape top", ring(CAPE[0], CAPE[1], 9, 12, 0.0, 12), "level", height=round(nat["cape"], 2), sides=3, color=GRAVEL)
    return h


# ---------------------------------------------------------------------------
# Cutting the plan into regions.

def clip_line(pts, hs, box):
    x0, z0, x1, z1 = box
    pieces, cur = [], []

    def flush():
        nonlocal cur
        if len(cur) >= 2:
            pieces.append(cur)
        cur = []

    for i in range(len(pts) - 1):
        (ax, az), (bx, bz) = pts[i], pts[i + 1]
        ha = hs[i] if hs else None
        hb = hs[i + 1] if hs else None
        dx, dz = bx - ax, bz - az
        t0, t1 = 0.0, 1.0
        ok = True
        for pp, qq in ((-dx, ax - x0), (dx, x1 - ax), (-dz, az - z0), (dz, z1 - az)):
            if pp == 0:
                if qq < 0:
                    ok = False
                    break
            else:
                r = qq / pp
                if pp < 0:
                    t0 = max(t0, r)
                else:
                    t1 = min(t1, r)
        if not ok or t0 > t1:
            flush()
            continue
        at = lambda t: ([ax + dx * t, az + dz * t], None if ha is None else ha + (hb - ha) * t)
        a, b = at(t0), at(t1)
        if not cur or t0 > 0:
            flush()
            cur = [a]
        cur.append(b)
        if t1 < 1:
            flush()
    flush()
    return pieces


def clip_poly(pts, box):
    x0, z0, x1, z1 = box
    out = pts
    for inside, cut in ((lambda p: p[0] >= x0, lambda a, b: x0), (lambda p: p[0] <= x1, lambda a, b: x1)):
        src, out = out, []
        for i, a in enumerate(src):
            b = src[(i + 1) % len(src)]
            if inside(a):
                out.append(a)
            if inside(a) != inside(b):
                x = cut(a, b)
                t = (x - a[0]) / (b[0] - a[0])
                out.append([x, a[1] + (b[1] - a[1]) * t])
    for inside, cut in ((lambda p: p[1] >= z0, lambda a, b: z0), (lambda p: p[1] <= z1, lambda a, b: z1)):
        src, out = out, []
        for i, a in enumerate(src):
            b = src[(i + 1) % len(src)]
            if inside(a):
                out.append(a)
            if inside(a) != inside(b):
                z = cut(a, b)
                t = (z - a[1]) / (b[1] - a[1])
                out.append([a[0] + (b[0] - a[0]) * t, z])
    return out


def region_of(x, z):
    return (math.floor(x / REGION), math.floor(z / REGION))


KNOWN = {"olive tree", "stone pine", "chestnut tree", "wildflowers", "tall grass", "bracken", "flowering shrub", "broom", "limestone boulder",
         "sea grass", "bench", "lamppost", "boat", "bridge", "door", "hatch", "walkway"}
FACTS = [
    "Marinella's wall was built against pirates long ago; now its watchtower watches for storms and returning boats.",
    "The woodcutters of Boscoverde bring chestnut wood and charcoal down the road to the village; the village sends fish and salt up.",
    "Every summer evening the whole village walks the quay and the piazza: the passeggiata.",
]


def write_regions(p):
    regions = {}

    def reg(r):
        return regions.setdefault(r, {"ground": [], "buildings": [], "characters": [], "creatures": [], "things": []})

    margin = 58.0
    for e in p.ground:
        key = "line" if "line" in e else "outline"
        pts = e[key]
        hs = e["height"] if isinstance(e.get("height"), list) else None
        xs, zs = [q[0] for q in pts], [q[1] for q in pts]
        for rx in range(math.floor((min(xs) - margin) / REGION), math.floor((max(xs) + margin) / REGION) + 1):
            for rz in range(math.floor((min(zs) - margin) / REGION), math.floor((max(zs) + margin) / REGION) + 1):
                ox, oz = rx * REGION, rz * REGION
                box = (ox - margin, oz - margin, ox + REGION + margin, oz + REGION + margin)
                pieces = clip_line(pts, hs, box) if key == "line" else ([[[q, None] for q in c]] if len(c := clip_poly(pts, box)) >= 3 else [])
                for piece in pieces:
                    q = dict(e)
                    q[key] = [[round(a[0][0] - ox, 2), round(a[0][1] - oz, 2)] for a in piece]
                    if hs:
                        q["height"] = [round(a[1], 2) for a in piece]
                    reg((rx, rz))["ground"].append(q)
    for ty, near, x, z in p.items:
        reg(region_of(x, z))["things"].append({"type": ty, "near": near})
    for ty, x, z, rot, y in p.things:
        r = region_of(x, z)
        b = {"type": ty, "dx": round(x - r[0] * REGION - 128, 2), "dz": round(z - r[1] * REGION - 128, 2), "rot": round(rot, 1)}
        if y is not None:
            b["y"] = round(y, 3)
        reg(r)["buildings"].append(b)
    for c in p.people:
        r = region_of(c["home_x"], c["home_z"])
        reg(r)["characters"].append(dict(c, home_x=round(c["home_x"] - r[0] * REGION, 1), home_z=round(c["home_z"] - r[1] * REGION, 1)))
    for c in p.creatures:
        r = region_of(c["x"], c["z"])
        reg(r)["creatures"].append(dict(c, x=round(c["x"] - r[0] * REGION, 1), z=round(c["z"] - r[1] * REGION, 1)))

    order = sorted(regions)
    seen, new = set(KNOWN), {}
    for r in order:
        for b in regions[r]["buildings"] + regions[r]["things"]:
            if b["type"] not in seen:
                seen.add(b["type"])
                new.setdefault(r, []).append(b["type"])
    os.makedirs(os.path.join(HERE, "regions"), exist_ok=True)
    for f in os.listdir(os.path.join(HERE, "regions")):
        os.remove(os.path.join(HERE, "regions", f))
    for (rx, rz), r in regions.items():
        plan = {
            "name": "Boscoverde" if rz >= 1 else "Marinella",
            "mood": "sunny, peaceful, colourful, the sea and the woods",
            "facts": FACTS,
            "new_types": [{"name": n, "tags": []} for n in new.get((rx, rz), [])],
            "landmarks": [],
            "settlement": {"name": "Marinella", "x": 128, "z": 128, "buildings": r["buildings"], "ground": r["ground"],
                           "road": None, "lamps": None, "bridge": None, "vehicle": None},
            "things": r["things"],
            "characters": r["characters"],
            "creatures": r["creatures"],
        }
        with open(os.path.join(HERE, "regions", f"{rx}_{rz}.md"), "w") as f:
            f.write("```json\n" + json.dumps(plan, indent=1) + "\n```\n")
    return order


def main():
    path = os.path.join(HERE, "genesis.json")
    with open(path) as f:
        g = json.load(f)
    g["landforms"] = landforms()
    with open(path, "w") as f:
        json.dump(g, f, indent=1)
        f.write("\n")
    w = {"seed": 31, "prompt": "A peaceful, colourful Mediterranean fishing village of painted houses and terracotta roofs climbing a hill above its stone harbour, walled with a watchtower; meadows of wildflowers and tall grass round it, and a woodcutters' hamlet in the chestnut woods up the road.",
         "spawn": [6, 44], "regions": [], "difficulty": "peaceful"}
    with open(os.path.join(HERE, "world.json"), "w") as f:
        json.dump(w, f, indent=1)
    build_land()
    nat = dict(zip(["clearing", "cape"], natural_heights([FOREST_AT, CAPE])))
    p = Plan()
    clearing = lay_ground(p, nat)
    village.lay(p, terrace_h, clearing, round(nat["cape"], 2))
    hamlet.lay(p, clearing)
    people.lay(p, round(nat["cape"], 2), clearing)
    regions = write_regions(p)
    w["regions"] = [list(r) for r in regions]
    with open(os.path.join(HERE, "world.json"), "w") as f:
        json.dump(w, f, indent=1)
        f.write("\n")
    print(f"landforms {len(g['landforms'])}, ground {len(p.ground)}, things {len(p.things)}, people {len(p.people)}, regions {len(regions)}")


if __name__ == "__main__":
    main()
