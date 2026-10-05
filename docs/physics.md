# Bodies, insides and tools

Built 2026-10-04 on branch `physics`. One set of small primitives that an
LLM fills in per world; the engine knows no houses, swords or shovels.

## Bodies (`sim/footing.rs`, `world/collide.rs`)

Every body (the traveler, characters, animals, mounts) is an upright capsule
sized by its own body: radius, height (less when crouching) and a step of a
quarter of its height.

- **Footing**: a body stands on the highest support under it: the terrain
  (with hollows) or the top of any solid between the ground and its step
  height (a floor, a stair, a roof, a rock). It steps down stairs, falls off
  ledges, lands. A solid that fills the column at step height is a wall, not
  a floor.
- **Walls**: solids block from just above the step to the top of the head,
  sampled through the body's height. Shapes carved by subtraction (a doorway
  cut out of a wall) report distances that are too small near the cut, so a
  near miss is confirmed by marching out to the rim and up its height (the
  sign is right even where the distance is not). Low growth (bushes,
  flowers) gives way below half a metre.
- **Jump** (half its height), **crouch** (down to 58%; stands up only with
  room overhead), **run** (×1.7). Characters crouch on their own when a gap
  fits them crouched and not standing.
- A body that is already pressing into something may move only if that
  doesn't press it deeper (no jostling in place).

## Shapes say what goes where (`lang/ir.rs`: `meta.anchors`, `joint`, `tool`, `hollow`)

- `anchors`: `door` (an opening, its size and the way out), `slot` (a real
  thing set there: `holds` a type name), `part` (a fixed part), `stairs`
  (foot and top), `bed`, `seat`, `light`.
- `joint`: a hinge (axis, pivot, how far it opens).
- `tool`: grip, tip, and the motions it knows (swing, chop, thrust, dig, pour).
- `hollow`: the ground taken away under it, down to a floor (a cellar, a
  burrow, a house set into a hill).

Things now turn fully (`GpuInst::tilt`, a quaternion after the yaw; CPU and
GPU agree, tested): doors swing on it, tools swing with it, a felled tree
lies on it.

## What the world does with them

- **Placing** a shape places its children (`model::children`): the built-in
  `door` hung in each doorway at the anchor's size, the thing named by each
  slot sitting on it (a real thing: `g` picks it up), each part. A probe
  check refuses a doorway that isn't really open or has no room behind it.
- **Doors** open and shut (`sim/joint.rs`): `e`, `/open the door`, plans
  (`{"do": "open"}` / `"close"`). Anyone with hands works one, and so does a
  body big and clever enough. Collision turns with the door.
- **Insides** (`sim/rooms.rs`): a shape with a doorway is a room. Characters
  walk in and out through the doorway and between floors by the stairs,
  open shut doors in their way, and sleep in a bed in the house by their
  home at night.
- **Clearance**: region and building tasks are told the folk height (the
  tallest people who build here, and the traveler), and doorways are sized
  for them; houses of beings too small for the traveler stay closed.
- **Tools** (`sim/tool.rs`): a held tool is held by its grip and turned
  through keyframed motions; at the motion's impact the tip lands on what was
  aimed at with its own speed: things are knocked and broken by physics and
  touched by the world's rules (with `force`), a chop notches and wears down
  what grows until it falls, a dig opens or deepens a pit in the ground
  (saved), a blow hurts a being (who flees, distrusts, or plans a response).
- **Roads and lamps**: a settlement's buildings are joined door to door by a
  spanning tree of roads (painted on the ground, kept clear of scatter);
  lamps of the plan's type stand along them. Placed things with `light`, and
  `light` anchors, shine and light the ground at night.

## Known gaps

- Interiors are lit only by the sky by day (they look dim and blue); `light`
  anchors light them at night.
- Characters route through one room's doorways and stairs; a building made
  of several shapes, or a path that leaves and enters by different doors, is
  not planned as a whole.
- The traveler takes no harm from blows.
