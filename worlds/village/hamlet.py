"""Boscoverde: the woodcutters' hamlet in its clearing in the chestnut wood."""
import math

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
