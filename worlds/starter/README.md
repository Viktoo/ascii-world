# Maple Cove, the starter world

A cozy fishing village on a northern coast in the middle of autumn. A loose
bunch of single-storey cottages with bright roofs sits round a green with a
gazebo and along three lanes. Below them a granite landing laid in setts
runs along the water, with a wooden pier out into a cove that faces the
sunset and a lighthouse out on Lantern Point. Round it all: meadows of asters
and goldenrod, apple orchards, and woods of red maple, gold birch, russet oak
and dark pine. Up the road in the woods is Pine Hollow, the woodcutters'
hamlet. About 30 people live here, and each one's work leans on someone
else's. Written by hand and made the way every world is made (the tooling
came from Marinella in `worlds/village`).

```bash
python3 worlds/starter/make.py
```

```bash
./target/release/pocket author worlds/starter
```

That writes `worlds/starter/starter.pocket` (about 0.9 MB, 30 s).

## What is where

- `make.py`: the land (landforms, the landing, the bank, the beach, the green,
  lanes and roads, the clearing), cutting everything into region plans,
  `world.json`.
- `village.py`: the layout: the businesses, the cottages along the lanes,
  the pier and boats, the green, the trees, the lighthouse.
- `shells.py`: every building type (cottage, family cottage, tavern, inn,
  general store, bakery, fish shed, boat shop, mayor's house, schoolhouse):
  one storey, clapboard, bright roof, porches, rooms walled off inside.
- `interiors.py`: what is in each building.
- `furniture.py`: furniture, fittings, goods, things to pick up, the pier,
  the gazebo, fences, the smithy; `signs.py`: the lettered signs and the
  signpost.
- `people.py`: the villagers and woodcutters, their households, their
  things and the animals; `hamlet.py`: Pine Hollow.
- `genesis.json`, `base/*.js`: palette, climate, biomes, landforms, species
  (harbor seals), and the scattered plants (maples, birches, oaks, pines,
  apple trees, autumn wildflowers). `types/*.js`: every placed type, by slug.
- `regions/`, `world.json`: written by `make.py`; don't edit by hand.

Plan metres: +x along the shore, +z inland, the sea to the south, the
gazebo at (0, 34). `make.py` turns the plan a quarter round into the world
(plan (x, z) is world (z, -x)), so the cove opens to the west and the sun
sets over it.
