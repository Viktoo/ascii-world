# Marinella, the starter world

A peaceful Mediterranean fishing village of painted houses climbing its hill
in terraces above a stone harbour, walled on the landward side with a gate
and a watchtower; flowering meadows and olive hills round it, and the
woodcutters' hamlet of Boscoverde in a chestnut wood up the road. Written by
hand and made the way every world is made (see `worlds/santa-monica` on
`vik/santa-monica-world` for the same tooling).

```bash
python3 worlds/village/make.py
```

```bash
./target/release/pocket author worlds/village
```

That writes `worlds/village/village.pocket` (about 0.7 MB, 20 s).

## What is where

- `make.py`: the land (landforms, terraces, harbour, roads, the clearing),
  cutting everything into region plans, `world.json`.
- `village.py`: the layout: every building (a business with its family's
  home over it), stairs, balustrades, the church, piazza, harbour, walls.
- `shells.py`: the shop-house and hotel types, with floors and stairs inside.
- `interiors.py`: what is in each building, its trade's fittings downstairs
  and the home upstairs.
- `furniture.py`: furniture, shop fittings, goods, things to pick up and to
  ride; `signs.py`: the lettered shop signs.
- `people.py`, `newcomers.py`: the villagers, households, their things,
  rides and animals; `hamlet.py`: Boscoverde.
- `walls.py`, `glyphs.py`: the wall, gate (MARINELLA lettering) and tower.
- `genesis.json`, `base/*.js`: palette, climate, biomes, landforms, species,
  and the scattered plants. `types/*.js`: every placed type, by slug.
- `regions/`, `world.json`: written by `make.py`; don't edit by hand.

World metres: +x east, +z north, the piazza fountain at (6, 18), the bay to the south.
