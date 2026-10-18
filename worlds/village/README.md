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
- `village.py`: Marinella's houses, church, piazza, stairs, balustrades,
  lanterns, harbour things, walls, gate, watchtower, the cape lighthouse.
- `hamlet.py`: Boscoverde's cabins, shrine oak, fire pit, kiln and the rest.
- `people.py`: the 14 villagers, 8 woodcutters, their things, the animals.
- `walls.py`, `glyphs.py`: the wall, gate (MARINELLA lettering) and tower.
- `genesis.json`, `base/*.js`: palette, climate, biomes, landforms, species,
  and the scattered plants. `types/*.js`: every placed type, by slug.
- `regions/`, `world.json`: written by `make.py`; don't edit by hand.

World metres: +x east, +z north, the piazza at (0, 45), the bay to the south.
