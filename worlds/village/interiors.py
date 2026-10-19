"""What is inside every building: the trade's fittings on the ground floor
and a home over it, sized to the family that lives there."""
from village import where, G, U

FLOOR = [0.0, G, G + U]


def put_in(p, key, ty, lx, lz, floor=0, face=0.0):
    """Something stood in a building, in its own frame (front +z, its door on
    the right at +x), on a floor (0 the shop, 1 and 2 the home)."""
    x, z, h, *_ = where(key)
    p.put(ty, x - lx, z - lz, (180.0 + face) % 360.0, y=h + FLOOR[floor])


# The trade on each ground floor: (type, x, z, facing), in the building's frame.
SHOPS = {
    "cantiere": [("boat on trestles", 0.4, 0.6, 90), ("workbench", -3.05, 1.8, 90), ("bucket", 2.6, 3.2, 0), ("broom", 3.2, -2.6, 0)],
    "pescheria": [("fish counter", -0.4, 1.4, 0), ("fish crates", 2.6, -1.8, 0), ("bucket", 2.9, 0.4, 0), ("lobster pots", -2.6, -1.9, 0)],
    "osteria": [("bar counter", 0.6, -2.2, 0), ("wine shelves", -3.3, 0.6, 90), ("wine barrels", 3.0, 0.8, 270), ("trattoria table", -1.2, 2.2, 0)],
    "gelateria": [("gelato counter", -0.6, 0.9, 0), ("shop counter", 2.0, -1.9, 0), ("cafe table", -2.3, 3.0, 0)],
    "capitaneria": [("desk", -1.0, 1.0, 180), ("bookshelf", -3.35, 0.4, 90), ("telescope", -1.6, 3.2, 0), ("chest of drawers", 3.25, -0.6, 270)],
    "bar": [("bar counter", 0.6, -2.2, 0), ("wine shelves", -3.3, 0.4, 90), ("cafe table", 1.0, 2.4, 0)],
    "farmacia": [("shop counter", -0.6, 0.9, 0), ("pharmacy shelves", -3.3, 0.4, 90), ("pharmacy shelves", 3.3, -0.8, 270)],
    "macelleria": [("butcher counter", -0.5, 0.9, 0), ("chest of drawers", 3.25, -1.4, 270)],
    "alimentari": [("shop counter", -0.6, 1.0, 0), ("grocery shelves", -3.3, 0.4, 90), ("grocery shelves", 3.3, -0.8, 270)],
    "pizzeria": [("bread oven", 2.4, -2.0, 0), ("trattoria table", -1.6, 1.4, 0), ("shop counter", 0.4, -2.3, 180)],
    "barbiere": [("barber chair", -2.8, 0.8, 90), ("barber chair", -2.8, 2.6, 90), ("sofa", 2.9, 0.6, 270)],
    "veterinario": [("vet table", -0.8, 0.8, 0), ("pharmacy shelves", -3.3, 2.0, 90), ("desk", 2.6, -1.4, 270)],
    "libreria": [("book shelves", -3.3, 0.4, 90), ("book shelves", 3.3, -0.8, 270), ("shop counter", -0.4, 1.4, 0), ("armchair", 2.6, 2.6, 225)],
    "panetteria": [("shop counter", -0.6, 1.0, 0), ("bread shelves", -3.3, 0.4, 90), ("bread oven", 2.4, -2.0, 0)],
    "sartoria": [("sewing table", -2.4, 0.6, 90), ("cloth shelves", 3.3, -0.4, 270), ("shop counter", -0.2, 2.2, 0)],
    "caffe": [("bar counter", 0.6, -2.2, 0), ("wine shelves", -3.3, 0.6, 90), ("cafe table", -1.2, 2.2, 0)],
    "fioraio": [("flower stand", -1.8, 1.2, 0), ("shop counter", 1.6, -1.8, 0), ("bucket", 3.0, 2.4, 0)],
    "scuola": [("blackboard", 0.0, -2.2, 0), ("school desk", -1.6, 0.0, 180), ("school desk", 1.4, 0.0, 180), ("school desk", -1.6, 2.0, 180), ("school desk", 1.4, 2.0, 180), ("bookshelf", 3.35, 2.4, 270)],
    "canonica": [("desk", 1.2, 1.4, 180), ("bookshelf", -3.35, 0.6, 90), ("upright piano", 2.8, -1.4, 270), ("armchair", -2.0, 2.6, 160)],
    "albergo": [("reception desk", 2.6, -1.6, 0), ("sofa", -5.6, 0.6, 90), ("armchair", -3.4, 2.6, 160), ("armchair", -1.8, 2.6, 200), ("floor lamp", -6.0, 3.0, 0), ("lemon tree", 5.6, 3.6, 0), ("bookshelf", 6.1, 1.0, 270)],
}


def home(p, key, beds, kids, hobby):
    """Two floors of home: the kitchen, the table and the sofa on the first,
    beds and the bathroom on the second; a hobby's things where they fit."""
    put_in(p, key, "kitchen", 3.2, 1.6, 1, 270)
    put_in(p, key, "dining table", 0.2, 1.2, 1, 0)
    put_in(p, key, "sofa", -3.1, 0.8, 1, 90)
    put_in(p, key, "floor lamp", -3.2, 2.9, 1, 0)
    put_in(p, key, "bathroom", 3.1, 2.4, 2, 270)
    put_in(p, key, "wardrobe", -0.6, 3.8, 2, 180)
    if beds:
        put_in(p, key, "double bed", -2.4, 1.2, 2, 90)
    if kids == 1:
        put_in(p, key, "single bed", 0.6, -1.6, 2, 0)
    elif kids >= 2:
        put_in(p, key, "bunk beds", 0.6, -1.4, 2, 0)
    elif not beds:
        put_in(p, key, "single bed", -2.6, 1.2, 2, 90)
    put_in(p, key, "chest of drawers", -1.8, -2.7, 2, 0)
    if hobby:
        ty, floor = hobby
        put_in(p, key, ty, 1.2, 3.4 if floor == 1 else 0.6, floor, 180)


def hotel_rooms(p):
    key = "albergo"
    for floor in (1, 2):
        for i, x in enumerate((-4.6, 0.0, 4.6)):
            put_in(p, key, "double bed" if (i + floor) % 2 == 0 else "single bed", x, 2.6, floor, 180)
            put_in(p, key, "chest of drawers", x + 1.6, 3.9, floor, 180)
        put_in(p, key, "armchair", -5.6, -1.2, floor, 90)
        put_in(p, key, "floor lamp", -6.0, 0.0, floor, 0)
    put_in(p, key, "bathroom", 5.7, -0.6, 1, 270)
    put_in(p, key, "bathroom", 5.7, -0.6, 2, 270)


def lay(p, households):
    for key, things in SHOPS.items():
        for ty, lx, lz, face in things:
            put_in(p, key, ty, lx, lz, 0, face)
    for key, (beds, kids, hobby) in households.items():
        if key != "albergo":
            home(p, key, beds, kids, hobby)
    hotel_rooms(p)
