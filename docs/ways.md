# Ways, water and hollows

Built 2026-10-15 on branch `ways`. The land can now be shaped the way people
shape it: canals and lagoons, islands and terraces, streets that follow the
land, bridges, rivers from the seed, water above the sea, caves you walk
down into, boats and carts people actually use. All of it is a handful of
primitives the LLM fills in; nothing knows "Venice" or "canal".

## 1. Ground edits (`ground.rs`)

An **edit** is:

- a **shape**: a line through points with a width, an outline, or a disc;
- a **mode**: `lower` to a height (only higher ground), `raise` to it (only
  lower ground), `level` at it (either way), or `paint` (height untouched);
- a **height**: one value, or one per point of a line (a road climbing a
  hill, a river falling);
- an **edge**: how far its sides slope (0.3 m a quay wall, 2 m a bank,
  10 m a shelving shore);
- optionally a **colour** (cobbles, gravel, a canal's murky bed), a **rim**
  (quay stones or a kerb along its top and down its sides, colour and
  width), and **water** (a level standing over it: a pond, a river, a canal
  on a hill; the sea stands below 0 everywhere anyway);
- a `kind` and `name` in words, for the story only.

Edits apply in order, each on the ground the ones before left. They are
**stamped into 16 m tiles** of 0.5 m cells holding how far the ground moved,
its paint (premultiplied, so soft edges fade to the ground and not through
black) and the water level. Reading the ground is one bilinear lookup however
many edits there are, on the CPU (`Ground::at`) and the GPU (`ground_*` in
`common.wgsl`, the tiles round the camera uploaded as one buffer, a 640 m
window moving in 64 m steps). Changing an edit restamps only the tiles it
touches; stamping runs on all cores. Tested to agree GPU/CPU (height 1e-4 m).

Three **layers**, applied in order: the land's own (rivers, from the seed),
settlements' (from their plans, saved in the region plan as `edits`), and
made in play (saved with the live world). Roads are edits now (old worlds'
roads are converted when loaded); the 16-roads-near-the-camera limit is gone.

## 2. Settlements shape their land (`settle.rs`, `model.rs`)

A region plan's settlement may have a `ground` list. Each entry: one shape
(`line` with `width`, `circle`, `rect` with an angle, or `outline`), `how`
(cut, fill, level, paint), `height` (0 is the sea; leave it out for a street
or square that follows the land), `sides`, `color`, `rim`, `rim_width`,
`water`, and `frontage`. Laid in order (streets read their heights off the
ground as shaped before them, stay above water, and cross the sea as
causeways), with cuts along lines going last so canals cut through streets.

- **Frontage**: buildings of a type lined along a line (both sides, left or
  right, every N metres), set back past its edge and rim, doors turned toward
  it, skipping spots across other ways.
- **Roads**: with streets, doors off every street get a spur to the nearest;
  without, the old spanning tree between doors.
- **Bridges**: wherever a way crosses a cut (a canal, a moat, a river), the
  built-in `bridge` (or the plan's own type) is set at the ways' height,
  humped so boats pass under, stone or timber by the street's colour.
- **The vehicle**: one per settlement, like the ball: the plan names one
  (`"boat"` exists); a boat is moored by a quay, anything else by the middle.
- **Lamps** stand along streets as along roads.
- Spawn and homes move off ground a settlement flooded.

The prompt (`REGION_TASK`) describes all this in general words (a canal city,
a harbour, a moated keep, terraces, a causeway).

## 3. Seats and vehicles (`sim/vehicle.rs`)

- **Anything with seat anchors can be sat on**: a bench (built-in, seats
  two), a chair, a throne. The use key or `ride`/`sit on` in a plan.
- A vehicle's **first seat steers**; the rest carry **passengers**, who ride
  along until near where they are going.
- **What drives it** (`meta.drive.by`): muscle (legs, oars, a pole) goes at
  the driver's strength and tires them; wind (a sail) goes with the weather's
  wind; a beast (horse, ox) needs one harnessed: the nearest willing beast is
  hitched when someone takes the reins and walks before it (pushed by hand
  without one it crawls); anything else (an engine, magic) as before.
- **Saddles**: an animal wearing anything with a seat anchor can be ridden.
- **Characters drive**: one going far takes a free cart; one whose way
  crosses water takes a boat, steers along a route through the water (a
  search on a 2 m grid that keeps off the shore and so passes under bridge
  arches), gets out where the water ends nearest the goal and walks on.

## 4. Rivers and water above the sea (`rivers.rs`)

Springs on high ground (in 512 m cells, more in a wetter land: the look's
`rivers`, 0–1, set at genesis; older worlds get 0.6) run downhill along the
smoothed land with momentum and a wander. The water never climbs: land in the
way is cut through as a gorge up to 7 m, else the river ends in a pond. It
widens and deepens as it goes, has sandy sides and low banks where the land
beside lies lower than its water, and runs out into the sea. A river is a
pure function of the seed, so only stretches within 750 m of the traveler
are laid, again on another thread every 250 m walked.

Water now stands wherever an edit says. `Terrain::water(x, z)` is the
surface over a point (the sea's 0, or higher); swimming, wading, floating,
boats, scatter, sounds, descriptions and the planner's map all read it. The
renderers draw it where the ground seen lies under it, as a level sheet at
that level.

### The sea, ground water, and holes that fill (2026-10-17)

Three primitives decide where water stands; nothing knows "lake",
"well" or "cave world":

- **The sea** is the world's own (`Look::sea`, metres, 0 by default; `null`
  for a land with no sea). It stands over all ground below it. With none,
  low ground is only low ground (a world of caves and bunkers, a desert
  basin, the Moon). `Terrain::sea` is read everywhere the old constant was;
  the GPU already took it as `G.water.w`.
- **Ground water** (`Look::groundwater`, metres under the land's own
  surface; none in older worlds) seeps into ground dug deeper than it:
  a pit, a crater from a blast, a ditch. A marsh fills a hole half a metre
  down; a desert none.
- **A basin fills to its lip** (`ground::basin`): from the bottom of the
  hollow at a point the water rises over the ground joined to it, lowest
  first, until the next ground lies lower than it already stands (it would
  run out there), or it reaches a cap. The water it holds is one ground
  edit: its outline traced round the wet cells, its level. So a pond dug on
  a slope fills only to its downhill edge, and nothing spills down the
  slope below. Ground water is a basin capped at its level; a deed's
  `"fill": true` is a basin with no cap (a pond, a lake on a mountain, a
  moat).

## 5. Caves and the floor under a point

A **cave** is a few capsules (a passage, a chamber) with a mouth where it
meets the surface. The ground is solid below its surface **and outside every
hollow**: the renderers march exactly that (rock and earth inside, the sky's
light fading with depth, no sea inside; the 24 capsules nearest the camera
on the GPU), `Terrain::floor(x, top, z)` is the highest ground at or below a
point (bodies stand on cave floors; cave walls and low roofs stop them;
things fall to cave floors and bounce off walls), and people find their way
in and out through the mouth as through a doorway.

They come from plans (`{ "what": "cave" | "mine" | "cellar" | "crypt" …,
"mouth", "toward", "length", "depth", "room" }`; things whose `near` names
one lie on its floor), from the seed (hillside caves, the look's `caves`),
and from deeds.

## 6. Shaping the land in play (`sim/interp.rs`)

A deed's `ground` effect may carry a `way`: a canal, ditch or moat is cut
from the doer the way they look or to where they point (joining a river or
the sea it fills with that water); a road or path is laid level in its
colour; a causeway or dyke is built up; a tunnel (mine, burrow) digs a real
hollow. A person does a stretch at a time, the traveler as much as asked.
Saved with the live world.

## Tests

`ground::tests` (canals and quays, order and heights along lines, islands
and ponds, restamping only changed tiles, the window), `settle::tests` (a
Venetian plan read and laid, frontage, crossings), `rivers::tests` (monotone
water, ends at the sea or a pond, the same from anywhere),
`render::tests::ground_edits_agree_on_gpu_and_cpu`, and `sim::tests::ways`:
a canal city (lagoon, island, canal, houses on both quays facing it, the
bridge, the moored boat, the same land after loading), benches seating two
and a villager rowing out of the canal and round the island to the far
shore, beasts pulling carts / legs pedalling / sails needing wind, a river
bridged by a street, a dwelling dug under flat ground that a villager walks
into and out of, and the land shaped in play surviving a save. Ignored
picture tests: `canal_city_png`, `river_png`, `cave_png`
(`POCKET_PNG_DIR=… cargo test --release <name> -- --ignored`).

## Not done

- Locks; water only runs into a cut canal from its ends when it is dug.
- Water doesn't flow: a full basin doesn't spill into a river, rain doesn't
  fill hollows, and a hole filled once doesn't drain if dug deeper beside it.
- Only water: no other liquid (lava, oil) has a level of its own yet.
- Settlement plans' cuts don't take the ground water (only deeds and blasts).
- Rivers don't join into one wider river; where two meet they just overlap.
- Cave floors are the curve of their capsules; no flat floors, no stairs
  shapes of their own (a ladder or stairs is a thing set in one).
- "Near" for hearing and seeing doesn't yet count floors and rock between
  (someone in a cave hears the street above).
- Land vehicles drive straight at where they go (no following roads).
- A bus stop is only what people make of a bench; no timetable.
- Regions already planned in an older world keep their old layout; new
  regions use all this.
