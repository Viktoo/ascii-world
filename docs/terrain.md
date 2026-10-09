# The land: cliffs, fields and woods

Built 2026-10-09. The land is still a pure function of the seed and the
biome table (nothing stored), and still exists twice, in Rust and WGSL, tested
to agree. Two things are new: land that steps up in **cliffs**, and things
that gather in **patches** the way they do in nature.

## Cliffs (`terrain.rs: Terrain::cliff`, `common.wgsl: cliff_h`)

A biome's `cliffs` (0 … 1) says how much of its relief stands as cliffs. The
rolling height is cut into steps 5–12 m tall (varying across the land). In
each step the ground keeps a fifth of its slope as a ledge, then rises the
rest of the way in a face:

- **Faces only where there are hills.** A face is as tall as the step it
  replaces, so its steepness comes from the land's own slope. Low land has
  fewer cliffs (none where the biome's `amp` is under 3 m, all of them from
  12 m), so a town stays level.
- **In stretches.** A slow noise decides where a cliffy biome actually has
  cliffs (`k`): at 0.3 the odd crag, at 0.7 canyon country, at 1 nearly all
  of its hills.
- **Rugged, not contour lines.** Where a step is cut wanders a little (in
  hilly land only: on a plain that wander alone would raise a face), and a
  face's foot wanders in and out along it (buttresses and clefts).
- **Gullies.** In places a face slumps into a steep, walkable slope, so the
  ledges above can be reached.
- **Continuous everywhere** (at a step's top the cut is zero on both sides),
  so there are no seams for the raymarcher or for feet.
- **Rock with beds.** Steep ground is the palette's rock, banded in level beds.
- **Old worlds are untouched.** A biome saved without `cliffs` reads 0, and
  its ground is exactly what it was (tested).

Rendering: the camera-centred heightmap is 0.75 m a cell, too coarse for a
sheer face, so within 80 m a cell whose corners differ by more than 1.2 m is
drawn from the exact height (primary rays and shadow rays alike; a shadow ray
started inside a coarse cell's wall drew black blots along every cliff foot).

Walking (`world/collide.rs: too_steep`): ground rising steeper than
`MAX_CLIMB` (1.4, about 54°) stops anyone walking up it, and they slide along
its foot as along a shore. Down it they go. Dug ground doesn't count (anyone
climbs out of a pit or a cellar), nor does ground under a body standing on
something. Pointing at a face, `describe` calls it a `cliff`.

## Patches (`world/scatter.rs`)

Each scattered kind grows in patches of its own: a noise field per tag
(`patch_depth`, 0 outside … 1 in a patch's heart, two layers so edges fray).
A biome's density for a tag becomes

    density × ((1 − c) + c × depth × PATCH_BOOST)

where `c` is how strongly it clumps (a biome's `clump`, else the kind's own
in `clump_of`) and `PATCH_BOOST` (2.6) keeps the average where the density
says (tested within 15% for every kind). So:

| Kind | Clumps | Patch size | Looks like |
|---|---|---|---|
| tree, palm | 0.6 | 60 m | woods with clearings, lone trees between |
| pine | 0.45 | 80 m | forest with glades |
| flower | 0.9 | 24 m | fields, bare grass between |
| tallgrass, reed, fern | 0.85 | 22 m | meadows and reed beds |
| bush | 0.5 | 28 m | thickets |
| rock | 0.55 | 36 m | boulder fields |
| grass | 0.45 | 30 m | lusher and thinner ground |
| anything else | 0.5 | 40 m | |

And things follow each other:

- **Stands.** Within a stretch of land a little wider than its patches
  (`stand_of`), one of a tag's types leads (three in four of its things), so
  a birch wood gives way to an oak wood rather than every tree being random.
  Things in a stand share `k.a` (0 … 1): a flower field's colour, a grove's
  tint, a meadow's ripeness, a thicket's blossom. `k.b` is how deep in its
  patch a thing stands. Trees grow tallest in the heart of a wood.
- **Litter** (sticks, mushrooms) lies where the trees are.
- **Talus.** Boulders (×7) and stones (×5) heap at the foot of sheer faces
  (`Terrain::height_and_foot`).

### Fields fill in (`is_cover`, `sub_cell`)

A cell is 4 m and holds one thing, too few for a field. Where ground cover
(flowers, tall grass, grass, reeds, ferns) is chosen in the thick of a patch,
up to three more fill the cell's other 2 m quarters. They show within 40 m
(the cell's own item carries the field into the distance, and the 1500
nearest things the renderer keeps still reach the far trees). Each quarter
has its own identity, in a range no real cell reaches
(`SUB + 4·cell + quarter`), so picking a flower, eating one or burning one is
remembered for that one alone and saved like any other cell. `item_at`,
`cell_centre` and `parent_cell` understand both.

## Built-ins

`wildflowers` (tag `flower`, a clump of stems whose colour is the stand's
`k.a`: buttercup, daisy, violet, poppy or cornflower, with a few strays) and
`tall grass` (tag `tallgrass`, a sheaf of leaning blades with seed heads,
green to straw by `k.a`). A world's own types of a tag replace them. The
default biomes now have flower fields (meadow, heath), tall grass (meadow,
lakeshore, highlands) and cliffs (highlands 0.7, heath 0.3, pine forest 0.15).

## For the world's LLM (`prompts.rs`)

Genesis may give each biome `cliffs` (0 … 1, with `amp` 15+) and `clump`
(per tag, 0 … 1), and is told that flowers and tall grass exist, that a
patch's things share `k.a` and that cover should be drawn as clumps.

## Tests

`terrain::tests` (old worlds unchanged, faces and ledges, flat land stays
flat, no jumps, boulders find the foot), `world::collide::tests` (faces stop
a climber but not a descent; anyone climbs out of a pit),
`world::scatter::tests` (density kept, patches patchy, stands hold, quarter
identities), `sim::tests::fields_woods_and_screes` (quarters picked one by
one, sticks under trees, boulders under cliffs),
`sim::tests::a_rock_face_is_seen_as_a_cliff`,
`render::tests::cliffs_agree_on_gpu_and_cpu` (12k points packed on faces:
worst error 2.5e-4 m). `sim::tests::land_pictures` (ignored) writes maps and
views of a default land and of red canyon country to `POCKET_PNG_OUT`.
