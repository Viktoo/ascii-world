"""Sunset Beach, west of the landing: sand shelving gently into the cove,
shells along the tide line, crabs, a fire ring of driftwood, chairs facing
the sunset, a beach ball and a picnic."""
import random

# The beach's fall from the meadow's edge to under the water: (plan z, height).
PROFILE = [(-6, None), (-14, 2.2), (-22, 1.1), (-30, 0.2), (-40, -0.9), (-64, -2.6)]


def sand_h(z):
    pts = [(z0, h) for z0, h in PROFILE if h is not None]
    if z >= pts[0][0]:
        return pts[0][1]
    for (z0, h0), (z1, h1) in zip(pts, pts[1:]):
        if z1 <= z <= z0:
            return h0 + (h1 - h0) * (z0 - z) / (z0 - z1)
    return pts[-1][1]


def lay(p):
    rnd = random.Random(7)
    put = lambda ty, x, z, b=180.0: p.put(ty, x, z, b, y=round(sand_h(z), 2))
    # Shells strewn along the tide line.
    for i in range(18):
        put("seashells", rnd.uniform(-146, -54), rnd.uniform(-29.5, -24.0), rnd.uniform(0, 360))
    # Chairs on the sand facing the sunset, a blanket and a basket, the ball.
    for x in (-84.0, -81.6, -79.0):
        put("adirondack chair", x, -20.5 + rnd.uniform(-0.4, 0.4), 180 + rnd.uniform(-12, 12))
    put("picnic blanket", -74.0, -21.0, 170)
    put("beach ball", -71.0, -24.0)
    # Someone's easel down by the water, a painting of the sunset half done,
    # and a smooth rock beside it to sit on and look.
    # Easel, painting, stool, palette and brushes: each its own thing.
    put("easel", -62.0, -25.5, 0)
    p.put("half-finished painting", -62.0, -25.5 + 0.0915, 0, y=round(sand_h(-25.5), 2) + 0.74)
    put("painter's stool", -62.0, -24.7, 180)
    p.put("paint palette", -62.18, -24.74, 20, y=round(sand_h(-24.7), 2) + 0.485)
    put("jar of brushes", -61.6, -25.0)
    put("granite boulder", -60.0, -27.6, 30)
    put("driftwood log", -64.5, -22.0, 100)
    # A fire ring of stones with driftwood logs round it, for evenings.
    put("fire pit", -112.0, -18.0, 0)
    put("driftwood log", -112.0, -14.8, 0)
    put("driftwood log", -115.2, -19.5, 70)
    put("driftwood log", -108.6, -19.8, 290)
    put("driftwood log", -132.0, -25.0, 30)
    # A dory pulled up on the sand, pots by it; rocks at the ends.
    put("dory", -96.0, -21.0, 200)
    put("lobster pots", -93.5, -19.0, 110)
    for x, z in ((-150.0, -26.0), (-147.0, -30.0), (-52.0, -24.0), (-55.0, -28.0), (-124.0, -33.0)):
        put("granite boulder", x, z, rnd.uniform(0, 360))
    p.creatures += [
        {"species": "rock crab", "count": 4, "x": -100.0, "z": -27.0, "names": [], "owner": "", "description": "crabs scuttling along the tide line"},
        {"species": "rock crab", "count": 3, "x": -60.0, "z": -27.0, "names": [], "owner": "", "description": "crabs among the rocks by the landing"},
    ]
