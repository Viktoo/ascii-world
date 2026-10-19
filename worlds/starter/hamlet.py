"""Pine Hollow: the woodcutters' hamlet in its clearing in the maple woods."""
import math
import random

C = (130.0, 520.0)


def lay(p, h):
    cx, cz = C
    at = lambda dx, dz: (cx + dx, cz + dz)
    p.put("shrine oak", *at(0, 17), 180, y=h)
    p.put("fire pit", *at(0, -2), 0, y=h)
    for a in (0, 120, 240):
        r = math.radians(a + 30)
        x, z = at(math.sin(r) * 3.4, -2 + math.cos(r) * 3.4)
        p.put("log bench", x, z, (a + 30 + 180) % 360, y=h)
    # The cabins round the clearing, their doors toward the fire.
    for dx, dz, face in ((-19, -2, 90), (-13, -16, 40), (19, -6, 270), (13, -19, 320), (-20, 12, 120)):
        x, z = at(dx, dz)
        p.put("log cabin", x, z, face, y=h)
    for dx, dz, face in ((-8, 9, 90), (8, -27, 0)):
        p.put("woodpile", *at(dx, dz), face, y=h)
    p.put("chopping block", *at(-6, 6), 0, y=h)
    p.put("sawhorse", *at(-10, 3), 30, y=h)
    p.put("charcoal kiln", *at(-22, 21), 0, y=h)
    p.put("beehives", *at(23, 12), 250, y=h)
    p.put("vegetable garden", *at(20, 4), 90, y=h)
    p.put("vegetable garden", *at(-24, -13), 50, y=h)
    p.put("horse shelter", *at(24, -24), 300, y=h)

    # The clearing's floor: ferns and fallen leaves round its edge, stumps
    # of trees long felled, flowers, blueberries, rocks, a few birches.
    rnd = random.Random(3)
    spots = [(-19, -2), (-13, -16), (19, -6), (13, -19), (-20, 12), (0, 17), (0, -2), (-22, 21), (23, 12), (20, 4), (-24, -13), (24, -24), (-8, 9), (8, -27), (34, 18)]
    def clear(dx, dz, r):
        return all(math.hypot(dx - a, dz - b) > r for a, b in spots)
    def scatter(ty, n, r0, r1, keep, y_off=0.0):
        done = 0
        for _ in range(n * 8):
            if done >= n:
                break
            a = rnd.uniform(0, 6.2832)
            r = rnd.uniform(r0, r1)
            dx, dz = math.sin(a) * r, math.cos(a) * r
            if clear(dx, dz, keep):
                spots.append((dx, dz))
                p.put(ty, *at(dx, dz), rnd.uniform(0, 360), y=h + y_off)
                done += 1
    scatter("fallen leaves", 22, 3, 29, 2.6)
    scatter("bracken", 30, 18, 30, 2.2)
    scatter("autumn wildflowers", 24, 6, 28, 2.0)
    scatter("tall grass", 22, 10, 29, 1.6)
    scatter("blueberry bush", 5, 20, 29, 4.0)
    scatter("tree stump", 7, 9, 26, 4.0)
    scatter("leaf pile", 3, 6, 22, 5.0)
    scatter("granite boulder", 3, 18, 28, 5.0)
    scatter("paper birch", 4, 26, 30, 6.0, -0.1)
    scatter("sugar maple", 3, 27, 30, 7.0, -0.1)
    p.put("pumpkins", *at(-16.5, 0.5), 90, y=h)
    p.put("pumpkins", *at(16.5, -4.0), 270, y=h)
    p.put("hay bale", *at(20.5, -20.0), 30, y=h)
