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
- Sitting in a vehicle or carrying an animal isn't saved: after a restart
  the traveler stands where they were.
- Carts can't be hitched to a horse yet; characters don't drive.

## Creatures in hand (`sim/carry.rs`)

- **e on a being** does what fits, told in the key hint: pet an animal
  close by, ride a mount that lets you, call an animal further off, nod
  to a stranger or shake hands with someone you know close by, wave at
  someone further off. With something in hand: give it (food to an
  animal is feeding), or work the held tool on them. The being meant is
  the one pointed at, else the one in front within 4 m (small animals
  are hard to point at).
- **Petting** (`{"do": "pet"}`): an animal easy with you (liking less
  wariness) makes a happy noise, wags and likes you more (more the less
  it does yet); a wary one shies off. Petting a horse until it trusts
  you is how a stranger's horse comes to be ridden.
- **Picking up** (`g`, `hold` on a being): small animals only (under 80%
  of what you can lift). One that trusts you settles in your arms for a
  minute or two; one that doesn't is caught only if it is slower than
  you walk or asleep, and squirms free in a few seconds and runs; a
  quick one darts off; a meat eater or a bold one may nip and get away.
  A carried animal sits at the holder's hands, can be petted, handed to
  someone with free arms, or set down (`g`); it can't be thrown.

## Vehicles (`sim/vehicle.rs`, `meta.drive`)

- A shape with `meta.drive: { speed, on: "land" | "water" | "air" }` and a
  `seat` anchor is a vehicle: a cart, a car, a boat, a flying carpet. It is
  never anchored, however heavy. `e` on it (or `/drive`, `/ride the car`)
  gets in; `e` again, or `/dismount`, gets out beside the seat (not into
  deep water).
- Driving: W/S throttle and brake (then reverse), A/D or ←→ steer. Land
  and water vehicles turn only while moving; flyers turn in place and
  go up with Space and down with c (in place too), or follow the look
  pitch while moving; let be, they hover. Land vehicles
  keep to the ground and tilt with it; boats float at the water line and
  can't leave it (they crawl in the shallows); everything is stopped by
  solids and bodies at its height. The view rises out of a closed cab.
- Boats set on water by a creation float there.
- Only the traveler drives for now; characters can't board.

## Blows, hurts and remains (`sim/remains.rs`, `Actor::sway`, `Sim::jolt`)

Nothing here is drawn per weapon or per kind of body; the body itself shows it.

- A blow (a sword, something heavy thrown) rocks the body that takes it: a
  `Jolt` leans the whole shape about its feet, away from the blow, for about
  half a second, and knocks it off its step for a third of that. Any body
  can, since it is a tilt of the instance, not a pose.
- Health under 0.6 shows in the gait: slower (a run no more than a hobble),
  stooped, lurching onto one side every other step, and now and then a
  stumble (a jolt forward). Living bodies mend, whole again in a game day.
- A being that dies leaves its body: a live thing of its own body type, in its
  look (sliders, clothes), `origin.remains` naming who it was ("body of Oda";
  "Oda", "the body", "corpse" find it). Upright walkers topple away from the
  blow; four-legged, crawling and floating bodies roll onto their side away
  from it. The fall takes 0.8 s. Where it rests is probed from the shape
  (`remains::lowest_point`), and any tilted thing that comes to rest is lifted
  out of the ground the same way. Being a thing, the rules go on acting on it
  (it no longer keeps itself warm), and it can be struck, cut, pushed, or
  carried by those strong enough (two people for a person).
- Remains go after three game days, only while the traveler is 90 m off;
  what they wore is left there.
- Fear (`sim/fear.rs`, `Rel::fear`, one way each) comes only from health
  going down at someone's hand: the struck fear the striker, and everyone
  within 25 m who saw a blow or a killing fears whoever did it (the timid
  more, the brave less). It halves every two game days. Near someone they
  fear, more so when that one holds a striking tool, a being runs; one that
  thinks is asked what to do (plead, back off, flee, stand its ground), and
  its fear and what the feared one holds are in its planner's context and
  its talk. Planners have a `flee` step.
- A thinking being that comes within 16 m of a body it didn't know of stops
  and stares, remembers it, and is asked what to do, shaken by how close it
  was to the dead (a dead animal shakes a person less). Those who saw the
  death don't "find" it again.
- In a world where things die (`hunting`), a body whose health the world's
  rules wear to nothing dies of it.

## Exposure (`sim/exposure.rs`)

`Sim::exposure(p)` says how open a spot is to the sky, 0..1: how far it rises
above the land within 25–60 m and above the water, broken up by standing growth
within 22 m, and 0 inside a room. It knows nothing about wind; the wind's sound
reads it now (`exposure^1.5`, so it is faint on flat meadows, gone in woods and
indoors, and strong on bare hilltops), and weather, drying or fire can read it later.
