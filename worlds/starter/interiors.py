"""What is inside every building: a trade's fittings where there is one, and
a home sized to the household that lives there."""
import village


def put_in(p, key, ty, lx, lz, face=0.0, up=0.0):
    """Something stood in a building, in its own frame (front +z, the door
    on the front wall), facing `face` from the building's own front."""
    x, z = village.local(key, lx, lz)
    b, h = village.PLACED[key][2], village.PLACED[key][3]
    p.put(ty, x, z, b + face, y=h + up)


# The trade in each business: (type, x, z, facing), in the building's frame.
FITTINGS = {
    "tavern": [("bar counter", 3.4, -2.2, 0), ("liquor shelves", 3.4, -4.15, 0), ("beer barrels", 5.5, 0.6, 270), ("tavern table", -1.4, 1.2, 0),
               ("tavern table", -4.0, 2.6, 0), ("tavern table", -4.2, -1.4, 0), ("tavern table", 3.4, 2.6, 0), ("rocking chair", -1.6, -3.0, 300),
               ("park bench", -3.6, 6.2, 0), ("park bench", 4.0, 6.2, 0)],
    "inn": [("armchair", -7.0, -2.4, 30), ("armchair", -4.8, -2.4, 330), ("sofa", -6.0, 0.6, 180), ("dining table", -3.0, 2.2, 0), ("floor lamp", -8.1, 3.8, 0),
            ("bookshelf", -8.25, 0.0, 90), ("desk", -1.6, -3.6, 0),
            ("double bed", 1.0, -2.4, 0), ("single bed", 4.1, -2.6, 0), ("double bed", 7.1, -2.4, 0), ("chest of drawers", 2.0, 0.4, 180), ("chest of drawers", 5.0, 0.4, 180),
            ("floor lamp", 8.0, 0.3, 0)],
    "store": [("shop counter", -2.0, 0.4, 0), ("grocery shelves", -4.35, 2.2, 90), ("grocery shelves", 4.35, 1.6, 270), ("apple barrel", 2.4, 2.8, 0),
              ("double bed", -3.2, -2.4, 90), ("kitchen", 1.0, -3.45, 0), ("armchair", 3.6, -2.6, 270)],
    "bakery": [("shop counter", -1.8, 0.6, 0), ("bread shelves", -3.95, 2.3, 90), ("bread oven", 3.05, 1.9, 270),
               ("double bed", -3.0, -2.1, 90), ("single bed", 0.6, -2.38, 0), ("kitchen", 2.9, -2.98, 0)],
    "mayor": [("dining table", -3.0, 1.6, 0), ("desk", 3.4, 1.4, 180), ("bookshelf", 5.6, 1.2, 270), ("floor lamp", 5.4, 3.8, 0),
              ("armchair", -4.9, -2.4, 60), ("armchair", -2.6, -2.4, 300), ("upright piano", 0.4, -3.9, 0),
              ("double bed", 3.8, -3.5, 0), ("wardrobe", 5.4, -1.4, 270), ("kitchen", -3.6, 4.2, 180)],
    "fishshed": [("fish counter", -1.6, -0.6, 0), ("fish crates", 2.6, -2.0, 0), ("lobster pots", 2.5, 1.0, 270)],
    "boatshop": [("boat on trestles", 0.0, -1.4, 90), ("workbench", -3.0, -3.55, 0), ("workbench", 3.4, -3.55, 0)],
    "school": [("blackboard", 0.0, -2.7, 0), ("desk", 2.6, -2.2, 180), ("bookshelf", -3.8, -2.2, 90)]
              + [("school desk", x, z, 180) for x in (-3.0, -1.5, 1.5, 3.0) for z in (-0.4, 1.4)],
}


def cottage(p, key, couple, kids, hobby):
    put_in(p, key, "double bed" if couple or kids else "single bed", -2.3, -1.6 if couple or kids else -1.75)
    put_in(p, key, "kitchen", 2.1, -2.45)
    put_in(p, key, "dining table", 2.35, 0.4)
    put_in(p, key, "armchair", -2.6, 1.6, 90)
    put_in(p, key, "wardrobe", -0.6, -2.45)
    put_in(p, key, "floor lamp", -3.0, 2.4)
    if hobby:
        put_in(p, key, hobby, -1.2, 2.45, 180)


def family_cottage(p, key, couple, kids, hobby):
    put_in(p, key, "double bed", -3.0, -1.9, 90)
    if kids >= 2:
        put_in(p, key, "bunk beds", 3.6, -2.1)
    elif kids == 1 or not couple:
        put_in(p, key, "single bed", 3.6, -2.15)
    put_in(p, key, "wardrobe", 0.4, -2.9)
    put_in(p, key, "kitchen", -2.8, -0.15)
    put_in(p, key, "dining table", -1.4, 1.4)
    put_in(p, key, "sofa", 3.2, 1.6, 270)
    put_in(p, key, "floor lamp", 4.0, 2.9)
    if hobby:
        put_in(p, key, hobby, -3.6, 2.8, 180)


def lay(p, households):
    for key, things in FITTINGS.items():
        for ty, lx, lz, face in things:
            put_in(p, key, ty, lx, lz, face, up=0.2 if lz > village.SHAPE[village.PLACED[key][4]][1] else 0.0)
    for key, (x, z, b, h, ty) in village.PLACED.items():
        couple, kids, hobby = households.get(key, (True, 0, None))
        if ty == "cottage":
            cottage(p, key, couple, kids, hobby)
        elif ty == "family cottage":
            family_cottage(p, key, couple, kids, hobby)
