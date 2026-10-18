# Under the ground and up off it

A few primitives so homes can be dug into the earth or set up in the trees,
reached by ladders, hatches and walkways, with caves that have their own
life. None of them knows what an elf, a dwarf hold or a tree-house is: the
LLM writes the shapes, the engine only reads these.

## Set into the ground (`meta.in_ground`, `src/inground.rs`)

`in_ground: depth` on a shape says it is set into the earth. The earth is
cleared **exactly from its solid's box** (traced in from its bounds, so a
margin in the bounds leaves no gap) as a box-shaped cave part, and stays
round it and over its roof. A cliff dwelling keeps the cliff over it; an
earth house keeps the lawn on its roof.

- **Where it stands** (`inground::seat`): its floor `depth` metres under the
  ground just outside its doorway. With `depth` under 1 (a doorway meant to
  open onto the surface) it is turned to the open side of a hill when the
  plan's turn faces it into the earth. A hatch sits level with the ground;
  with neither, y = 0 is the ground's level (a well).
- **Its way in** (`inground::cave`): each doorway gets an opening through the
  earth; a doorway under the ground gets a ramp dug up to the surface (1:2);
  a hatch gets a shaft through any earth over it. The cave's mouth is the
  ramp's top (or the step outside the door, or the hatch).
- The hollows live in their own cave layer (`Layer::Shapes`), rebuilt with
  every world version. Bodies, falling things, the renderers and the planner
  all read them as caves. The GPU takes the 40 nearest cave parts (it took 24).
- People going in walk to the mouth, down the ramp and through the door;
  coming out, through the door first, then up to the mouth (`rooms::route`).
  Inside one room (any floors of it) its own stairs lead; a
  room whose doorway opens onto the ground counts as ground level for the
  routing between levels. On a stair, people go on along it to the end
  nearer where they are going (no turning back halfway).

The plain cut under a building with a doorway (so no slope pokes through its
floor) now runs out to its walls and slopes in from them, so the ground meets
the walls with no ring of air round them.

## Doors and hatches of any style

- A `door` anchor may name its own door in `holds`: a type on a hinge (a
  round door, an iron gate, a stone slab, a hide flap) hung in the doorway
  instead of the plain wooden one. The region builder writes it if no type of
  that name exists, sized to the doorway (`brain::write_contents`).
- A `hatch` anchor (`size: [width, depth]`) is an opening in a floor, a roof
  or the ground. A lid is laid in it, hinged along its back edge and lifting
  toward `face`: its own (`holds`) or the built-in `hatch`. It opens with E
  like any door (anything with `meta.joint` does). As a doorway, a hatch is
  "over it" and "under it" (`rooms`).
- A room ends at its shape's real roof (its probed top), so someone standing
  on a roof hatch is outside it.

## Ladders (`src/sim/climb.rs`)

A `stairs` anchor steeper than 1.3 (rise over run) is a ladder: rungs, a rope
ladder, pegs up a trunk. Its `at` is the floor in front of its foot and its
`to` the floor it reaches, a step past its top.

- The traveler goes up by walking into its foot and down by walking off its
  top toward it (or looking down while walking). Anyone falling past a ladder
  catches hold of it.
- On it a body is held (no falling) and moves along it; at either end it
  steps off.
- A shut hatch over its top stops a climber: people open it; the traveler
  waits under it until they press E.
- People take a ladder when their way goes up or down it (`ladder_step`).

## Up off the ground on a support

A placement with a height (`y`) that would float is set on whatever placed
thing stands under it within reach (`WorldModel::support_below`): a platform
in a great tree, a ledge, stilts, a roof, placed earlier in the same commit or
before. With nothing under it, it is refused as before, saying why. Region
plans give buildings, landmarks and things a `"height"` above the ground.

## Walkways (built-in `walkway`)

`settlement.walkways: [{ "from": [dx, dz, height], "to": [dx, dz, height],
"width", "look": "rope" | "timber" | "stone" }]`. Each end rests on what stands
there at that height (found as above) or on the ground at height 0; a
walkway with nothing under an end is left out. It is laid end to end in
straight spans of up to 14 m (more where it climbs steeply), a rope bridge
sagging in the middle (`settle::spans`). A span climbs up to 8 m over its
length, in 0.2 m steps where it is steeper than 0.35. Its distance function
is exact on a gentle slope, so a body walks straight onto it.

## Finding the way between levels (`src/sim/levels.rs`)

The places a way can turn are the ends of every stair and ladder, of every
walkway span, and the step outside every doorway. Two places at one level
(within 0.6 m) are joined where something holds a body up all the way between
them; places on the ground are all joined. When a body or where it is going is
off the ground and no straight walk joins them, its next place comes from
the shortest way through these (climbing costs double), remembered for 1.5 s.
A body on the ground going somewhere on the ground needs none of it.

## Life in caves

- `meta.grows: { at: "cave" }`: it grows only on cave floors (never on the
  open land), as thick as the biome over it says for its tag, where there is
  room under the roof. Not in the rooms of shapes set into the ground. Its
  identity is its cell's own quarter 0.
- A species with `"caves": true` has its home in the nearest cave's chamber
  (the land's own caves from the seed, laid or not, or a settlement's), starts
  on a cave's floor, and wanders only over cave floors.
- People and creatures now keep the height they were saved at (`SavedState::y`),
  so someone up a tree or down a cave is still there after a load.

## Tests

`inground::tests` (the solid box, turning to a hill's open side, a ramp to a
door under flat ground), `sim::tests::underground` (an earth house a villager
walks into and out of; a hatch home: the hatch hung, opened, the ladder down
and up; the traveler held under a shut hatch until it opens; fungus on a
cave's floor and a cave cricket that keeps to the cave),
`sim::tests::heights` (a hut on a great tree's platform, a walkway between two
platforms, a villager climbing, crossing, going in and coming down). Pictures:
`earth_house_png`, `tree_village_png`
(`POCKET_PNG_DIR=… cargo test --release <name> -- --ignored`).

## Not done

- Not yet tried with a real LLM.
- Walkways are the built-in span only; a plan can't give its own span type.
- Ladder climbing is kinematic: a climber passes through what lies between
  its foot and top lines (a roof's edge by a hatch).
- A ramp dug to a door under the ground is a round tunnel (a capsule), not a
  trench with straight walls or steps.
- A shape set into the ground doesn't take the ground water (a well dug
  below it stays dry unless its shape holds water).
- Routing between levels joins places up to 30 m apart only; a way along a
  long platform with no turn on it may not be found.
- Level routes cost a look along each way when they are worked out, at most
  every 1.5 s per body: fine for a village, not tried with crowds.
