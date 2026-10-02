# Plan: species

## Vision

The world has more than people. A dog follows its owner and brings back a
thrown stick. A cat sleeps in the sun and won't be hugged by a stranger. An
orc and an elf share a village and don't trust each other. A dragon wraps its
wings around the one person it loves.

None of this is scripted per species. Every species is described with the
same few numbers (the taxonomy), every body answers to the same pose roles,
and the existing verbs, needs, relationships and rules do the rest.

Creating a species costs one builder call today. As models get cheaper, the
same primitives allow more species, more variety and richer bodies without
changing the engine.

## Scope

- Bodies from cat or dog size (about 0.4 m) up to dragons (about 12 m). Nothing
  smaller: at terminal resolution a mouse or a songbird is a pixel or less, and
  you can't see it or point at it.
- Earth worlds: humans plus animals like dogs, cats, horses, goats, deer, wolves.
- Magic worlds: elves, orcs, giants, griffins, dragons… any number of species,
  some of which fly.
- Not in scope: swarms, insects, fish under water, small birds.

## Principles

These carry over from `emergence-plan.md`, applied to bodies and minds.

1. **Meaning apart from animation.** A hug is consent, affection, distance and
   a relationship change (`social.rs`). How a body shows it is that body's
   business.
2. **Pose roles, not limbs.** Gestures move roles ("reach toward", "lower
   head"). Each body decides what a role moves: arms, wings, paws, a neck.
3. **Behaviour comes from the taxonomy, never from names.** A wolf scares a
   goat because it eats meat and is bigger, not because it is a "wolf".
4. **Same verbs for everyone.** A dog uses `follow`, `hold`, `catch`, `eat`,
   `gesture`, `propose` like a person does. What it can't do, it can't do
   because of its body or mind, not because of a special case.
5. **Humans are just one species.** The built-in figure becomes the human body,
   and old worlds load unchanged (missing species = human).
6. **Looks are data, not fixed fields.** Proportions, colours and clothing are
   open sliders and layers that a world, a region, a culture or one person can
   set. "A warrior village" or "everyone is a giant" changes values and adds
   layers. It never needs new engine code.
7. **The traveller stays the traveller.** The player is always a human-shaped
   traveller, but their size and dress are values like anyone else's.

## What exists today

- One built-in body, `src/builtin/figure.js`, shared by the player and every
  character (`WorldSnapshot::figure_type`). Its pose is 8 floats (`k.s0…s7`).
  7 are used, and the 8th is free.
- Pose channels are named after human limbs: `l_raise`, `r_raise`, `l_fwd`,
  `r_fwd`, `lean`, `nod`, `crouch` (`src/sim/actor.rs`). Built-in gestures are
  hard-coded poses, and custom gestures are key poses written once by the LLM
  and stored in the `gestures` table.
- Where the hand is comes from Rust code that mirrors the figure's arms
  (`Actor::hand`, `hand_local`). Eye height (1.65), reach (2.4) and walking
  speeds are constants.
- A character's look is `FigureLook` (height, build, skin, shirt, trousers).
- Minds (`npc.rs`) are the same for everyone: needs, traits read from the
  persona's words, option scoring (eat, rest, play, company, curiosity, flee,
  gather, wander), and LLM goals within a budget.
- Relationships (`social.rs`) are per pair, seeded from personas.

## The primitives

### 1. Body (a type with a body contract)

A body is an ordinary object type (the strict JS subset, probed and compiled
like any other) with the tag `body` and a `body` block in `meta`:

```js
export const meta = { name: "dragon", bounds: [6, 5, 12], tags: ["body"],
  props: { mass: 4000, burns: 0, heat: 40 },
  body: {
    height: 5,            // m, standing
    eye: 4.2,             // m
    radius: 2.5,          // collision radius
    grip: [0, 3.8, 5.5],  // where held things sit (mouth, claw, hands), local
    seat: [0, 3.2, 0.5],  // where a rider sits, local (omit if it can't be ridden)
    reach: 3.5,           // m
    roles: ["reach", "raise", "head", "crouch", "lean", "spread"],
    gait: "quad",         // walk animation hint: biped | quad | slither | hover
    flies: true
  } };
export function sdf(x, y, z, k) { /* reads k.s0…k.s7 as roles, k.f as walk phase */ }
export function color(x, y, z, k) { /* … */ }
```

The 8 pose slots get role names. Today's names stay as aliases, so saved
gestures still load:

| Slot | Role | Human | Dog | Dragon |
|---|---|---|---|---|
| s0 / s1 | `raise_l` / `raise_r` | arm up sideways | front paw up | wing up |
| s2 / s3 | `reach_l` / `reach_r` | arm forward | paw forward | wing wraps forward |
| s4 | `lean` | bend at hips | front down, rear up (play bow) | neck forward |
| s5 | `head` | nod | head down | head down |
| s6 | `crouch` | crouch / sit | sit / lie | lie down |
| s7 | `spread` | (none) | tail wag | wings spread / frill |

- A body says which roles it has (`roles`). The probe drives each role from 0
  to 1 and checks the shape stays finite, in bounds and actually changes.
- `Actor::hand` uses `grip` plus a small offset scaled by the reach role, and no
  longer mirrors the figure's arm maths. The human figure gets a `grip` at the
  right hand, so nothing visible changes.
- Bodies have a tighter fuel budget than other types (many are on screen and
  they're animated). The probe rejects slow bodies with a clear message, the
  same way it rejects slow shapes today.

### 2. Species (data)

A species is a body plus a row of numbers and a few words, stored in a new
`species` table (`name`, `body_type`, `json`). The built-in `human` species
uses `figure.js`.

```json
{ "name": "dragon", "body": "dragon", "plural": "dragons",
  "size": 5.0,
  "mind": "sapient",
  "speech": "words",
  "diet": { "plants": 0.0, "meat": 1.0, "props": { "heat": 0.5 } },
  "social": "solitary",
  "temper": { "bold": 0.9, "wary": 0.2, "playful": 0.2, "tame": 0.1 },
  "move": { "walk": 2.0, "run": 8.0, "fly": 14.0, "swim": 0 },
  "life": { "sleep_hours": [2, 9], "nocturnal": false },
  "needs": { "hunger": 0.6, "fatigue": 1.0, "social": 0.2, "fun": 0.4, "curiosity": 0.8 },
  "look": "description used when writing characters and talking about them" }
```

### 3. The taxonomy (shared axes)

Every species sits on the same axes, and every behaviour between species
reads only these.

| Axis | Values | What it drives |
|---|---|---|
| `size` | metres | who looks dangerous, who can carry whom, hug distance, collision, how far you notice it |
| `mind` | `instinct` · `simple` · `sapient` | `instinct`: scoring only, no LLM. `simple`: rare LLM goals, no words. `sapient`: full minds, plans, crafts, create |
| `speech` | `none` · `sounds` · `words` | can you talk with it? `sounds` answers in noises and gestures, `words` in dialogue. Gossip only passes between those that share words |
| `diet` | weights for plants, meat, and any property | what counts as food for them (`edible` × diet), and who is prey |
| `social` | `solitary` · `pair` · `pack` · `herd` · `village` | who they seek out for company and how far they stray from their group |
| `temper` | bold, wary, playful, tame (0–1) | fear and flight distance, play, how quickly they warm to others |
| `move` | walk, run, fly, swim speeds | paths, flight, escaping |
| `life` | sleep window, nocturnal | daily rhythm |
| `needs` | how fast each need drifts | a dragon is rarely lonely, a dog often is |

Derived relations, all computed from the axes:

- **Predator and prey.** `A` hunts `B` when `A.diet.meat > 0.5` and
  `B.size < 0.7 × A.size` and `B` is not in `A`'s group (family, owner,
  village). `B` fears `A` by the same test seen from the other side. Fear and
  flight distance scale with `B.temper.wary` and the size ratio. Hunting can be
  turned off per universe (cosy worlds) with one setting. In that case predators
  only stalk and chase, and nothing is killed.
- **Mounts and carrying.** Someone can ride or be carried by a species whose
  `size` is at least 1.8× their own and that is tame towards them.
- **Pets and owners.** A relationship `"Bren: owner"` on a non-sapient species
  means following, guarding, greeting and fetching. These are existing actions
  (`follow`, `catch`, `gesture`), chosen by the scorer.
- **Groups.** `pack` and `herd` add a "stay near my kind" option to the scorer,
  and `herd` adds "flee together" when one member flees.
- **Species attitudes.** Genesis can seed a starting attitude between sapient
  species (affection, trust, rivalry), e.g. elves and orcs: trust −0.4. It's
  only the starting value for a new pair. After that, each person's own
  history with the other takes over, so a single orc can earn an elf's trust.

### 4. Look: sliders, varieties and layers

Today a person's look is five fixed numbers (`FigureLook`), and nobody wears
anything. That becomes three open pieces.

**Sliders.** A body declares its own named look parameters with ranges, and
the engine passes them in `k` like today's `k.a…k.e`:

```js
body: { …, look: { height: [1.2, 2.4], build: [0.7, 1.4], skin: [0, 1],
                   hair: [0, 1], ears: [0, 1], age: [0, 1] } }
```

A dragon declares `scales`, `horns` and `belly` instead. A character stores
`look` as a free map, and unknown keys are dropped with a warning. The human
figure keeps its current five, plus `hair` and `age`.

**Varieties.** A species can have varieties: named slider ranges plus default
layers. "Hill folk": short and broad, with wool cloaks. "Tide elves": tall,
with blue-grey skin. Genesis and region plans choose them per settlement, so
"a warrior village" is a variety with armour, chosen for that village. A
variety is data only (no code), so it costs no builder call when it reuses
existing layers.

**Layers (dress and gear).** A layer is a small type module with the tag
`layer`, written for one body. The builder gets the body's code in its prompt,
so the layer follows the same pose maths (a breastplate tilts with `lean`, a
cloak hangs from the shoulders). Each actor wears up to 3 layers, for example
armour, a cloak and a hat.

- Rendering: the shader takes `min` over the body and its layers in the same
  instance frame, and colour comes from whichever surface is nearest. Only
  dressed actors pay for the extra layers.
- Layers have properties like any thing (`burns`, `mass`, `fragile`), so the
  rules apply: a cloak catches fire, and armour makes you heavier and slower.
- A layer can be taken off and becomes a live thing (`drop` the helmet), and
  a held layer can be put on (`wear`, a new small verb next to `hold`).
- Layers are reused by name across a world. Once there's an "iron helmet" for
  humans, any human can wear one.

**World-scale values.** Genesis can set:

- per-species size multipliers ("everyone is a giant": human height ×1.8)
- the traveller's own size ("I'm small": traveller height 0.9 m)

Eye height, reach, speed, what you can lift and contact distance all follow
from size (see 5), so "small among giants" plays out without special code.
Characters carry you, things are heavy, and doors are high.

### 5. Size-aware actors

`Actor` stops assuming a human:

- Eye height, reach, collision radius and speeds come from the species and body.
- Contact gestures keep a distance between surfaces, not centres:
  `distance + radius_a + radius_b`. A person hugging a dragon stands at its side.
- Contact gestures check the size gap. A hug between bodies whose sizes differ
  more than 4× becomes "lean against", which is still a hug socially.
- `liftable` and joint carrying include carrying a smaller actor (a child, a
  cat) as a held body.
- Placement and the region plan keep large species' homes on open ground.

### 6. Gestures across bodies

- A gesture is role poses, so every built-in and custom gesture plays on any
  body that has those roles.
- If a body lacks a role a gesture needs, the engine uses the gesture's
  fallback (`head` for `nod`, and `reach` for `high_five` on a body without
  `raise`). If there's still no match, the first use asks the LLM once for that
  species' version. It is stored in `gestures` as `name@species`, the same way
  custom gestures are stored now.
- `spread` lets bodies show feeling without arms: a tail wag on a greeting, wings
  spread when angry. The scorer and plans can use it through gestures like
  `/gesture wag`.

### 7. Minds by species

- `npc.rs` keeps one mind. The species scales need drift, sets the sleep window,
  adds the group, fear and owner options, and maps diet onto "eat".
- `instinct` and `simple` minds don't use the dialogue model. Talking to them
  (`Enter` near a dog) becomes a `Decide` with a sound, a gesture and an action,
  answered by a template or the fast decider. They still have memories and
  relationships: a dog remembers who fed it.
- Traits (`Traits::from_persona`) start from the species `temper` and are then
  adjusted by the persona's words, as they are today.
- Non-sapient characters get a short persona: a name if someone gave them one,
  an owner and a look, but no voice or trade.

### 8. Flight and riding

- A flying actor has an altitude above the ground. It cruises a few metres
  above the highest solid near its path (a sphere check against nearby SDFs,
  the same as for things) and lands to eat, sleep, hold or do contact gestures.
- Flight is a way of moving inside `goto`, not a new verb. The scorer picks it
  when the target is far or across water.
- The camera, culling and shadows already handle anything with bounds. Flying
  bodies just need the vertical cull test to include their altitude.

**Riding** reuses `hold`, but attaches an actor instead of a thing.

- A body that can be ridden declares a `seat` point. The `ride` and
  `dismount` verbs join the shared action set, so the player, characters and
  agents use them the same way.
- **Who can ride:** the mount must be at least 1.8× the rider's size (see
  Mounts and carrying), have a seat, and agree. Agreeing is a consent check
  like a contact gesture: `tame` plus affection for the rider. A wild horse
  throws off a stranger (dismount, with a small impact), and a horse that
  knows you lets you on.
- **While riding:** the rider follows the seat each step and their pose
  switches to sitting (`crouch`). The rider can still talk, hold one thing,
  and use or throw it.
- **The traveller steering:** walk keys drive the mount at the mount's speed.
  The camera moves to seat height plus the traveller's eye height. Turning
  works as on foot, and a flying mount climbs and dives with look up and down.
- **A character riding:** they give the mount a `goto`. The scorer picks
  riding for long trips when they own or are close to a mount.
- **The mount's own mind keeps running.** Fear can make it bolt or throw its
  rider (a wolf spooks the horse), and fatigue makes it slow down and stop.
  Affection for the rider grows with gentle rides and feeding.
- The rider's mass counts toward what the mount carries, so a small pony can't
  carry a giant.
- Flying with a rider is just a flying `goto` with someone in the seat.

### 9. Doing, editing and creating with beings

The free-text `/` path (do, edit, create) works on beings too, with the same
schema the interpreter already uses. It never writes free text into a being's
body code.

- **Edit means layers.** `/give the guard a red cloak` or `/take off his
  helmet` reshapes, adds or removes a layer through the existing reshape
  pipeline. The body module stays fixed. A sapient being has to agree, which is
  a social act like a contact gesture with its own intimacy (dressing someone
  needs trust). Tame animals accept a collar, saddle or harness from someone
  they like.
- **Do means values.** `/feed the horse`, `/calm the dragon`, `/teach the dog to
  sit` go through the interpreter, which answers only with property changes,
  need changes, relationship changes, look sliders inside the body's ranges, or
  a new learned gesture. Answers are cached by (what, with what, on whom), like
  today.
- **Body changes come from rules and magic.** A curse that turns someone into a
  toad is a `transform` to another species, allowed only when the universe has
  a rule or property for it. Size changes (`growth`) scale within the species'
  range. Memories and relationships survive a transform.
- **Create means a new being.** `/make a clay golem` or a wizard's plan can create
  a being of an existing or new species. The universe setting `sim.create_beings`
  (off by default in earth worlds) gates it, it goes through the species
  pipeline, and it counts against the creature caps. The maker becomes the
  creature's owner or parent.

### 10. Breeding and change over time

**Offspring.** Two beings of the same species (or of species the universe
marks as able to mix), who are partners or pair-bonded and have high affection,
with food and a home, can have young:

- The young are a new character of that species. Each look slider is the
  parents' average plus a small random drift. Temper values blend the same way.
  Layers aren't inherited, but the parents' variety is.
- They start small (`growth` 0.3), follow a parent, and grow to full size over
  some game days. The size-aware rules handle this for free: they can be
  carried, they can't lift much, and they play more.
- Sapient young get a name from a template at birth and a persona from the LLM
  later, when they first matter (the player meets them, or they come of age).
- Family relationships (`parent`, `child`, `sibling`) are seeded at birth.
- Population caps per species and per region, plus a birth rate that drops as
  a region fills up, keep numbers bounded. Death of old age is a universe
  setting (`sim.lifespan`, off by default).

**Lineages.** Each being records its parents. A lineage is the line of
descent of one founding pair.

**Drift.** Over generations, look sliders and temper drift with what happens:

- Young born to parents who were fed and handled by people inherit a slightly
  higher `tame`.
- Lines that are hunted inherit a higher `wary`.
- Sliders drift randomly within the body's ranges.

All of this is in data. No code changes, and no LLM.

**New varieties and species.** When a lineage's averages move far enough from
its variety's ranges (a distance threshold on the taxonomy and look vectors),
it becomes a new variety. That is one small LLM call to name and describe it,
for example "the tame wolves of Pinewood, now called hounds". A further drift,
or a big taxonomy change (diet, mind, social), makes a new species that reuses
the same body module with new ranges. Universe rules can push drift on purpose
(a magic spring makes everything born near it larger).

### 11. Generation

- **Genesis.** The universe call adds a `species` list: 1–5 species that fit the
  world (earth: human plus a few animals. magic: the peoples and beasts of that
  world). Each gets its body module in its own ```js block, like base types.
- **Looks at genesis.** The same call can set size multipliers, the traveller's
  size, and starting varieties with their layers ("a warrior culture": armour
  and shields for humans).
- **Regions.** The region plan's characters get `"species": "name"`, plus an
  optional `"variety"` per settlement and per-character `look` sliders and
  `layers`. The plan
  may add `new_species` (at most one per region), which the builder writes
  before the characters appear, the same way it writes `new_types`.
- **Creatures in regions.** The plan gets a `creatures` list, separate from
  characters, for non-sapient beings: a herd of goats by the farm, a wolf pack
  in the wood, a cat by the inn. Each entry is a species, a count, a home and
  optional names and owners.
- **Characters in the world.** A sapient character can `create` a new species
  only if the universe allows it (a wizard breeding griffins). It goes through
  the same pipeline and caps.
- **Old worlds.** Missing species means human, and the `species` table is added
  on open.

## Phases

Each phase ends with a proof scenario as a test in `src/sim/tests.rs`, like the
emergence plan.

1. **Pose roles and the body contract.** Rename channels to roles (keep the old
   names as aliases), add `meta.body`, move `Actor::hand` to `grip`, add `seat`, read eye,
   reach and radius from the body, and probe role coverage and body fuel.
   - *Proof:* every existing test passes unchanged, and a second built-in body
     (`dog.js`) plays wave, sit and hug from the same gesture code.
2. **Species and size.** The `species` table, built-in `human`, `dog` and `cat`,
   `"species"` on characters, size-aware contact distance and lifting, and
   speech levels for talking.
   - *Proof:* a dog with an owner follows them, brings back a thrown stick
     (catch plus mouth grip), and greets them with a tail wag. A stranger's hug
     is refused by a wary cat. Talking to the dog gives a sound and a gesture,
     never words.
3. **The taxonomy at work.** Diet as food, predator and prey, fear distance,
   packs and herds, owners, species attitudes, and the no-killing setting.
   - *Proof (`pocket sim --hours 24`):* goats stay together and flee a wolf as a
     group, the wolf never targets a grown person, elf and orc affection starts
     low and rises for pairs who play or work together, and no species
     disappears from the world.
4. **LLM-written species.** Genesis `species`, region `new_species` and
   `creatures`, per-species gesture variants written on first use, and probes
   with repairs.
   - *Proof (scripted model):* a dragon species is written, placed and hugs the
     one character it has high affection for (a wing-wrap through `reach`).
     A snake-like body without `raise` gets a written `high_five@naga` variant
     once, then reuses it.
5. **Looks and layers.** Body-declared sliders, varieties, layer modules
   rendered in the same instance, the `wear` verb, world size multipliers and
   the traveller's size.
   - *Proof (scripted model):* "a warrior village" gives its people an armour
     variety, and the armour tilts with `lean` and burns off in a fire. In
     "a world where I'm small and everyone is big", the traveller's eye height,
     reach and lifting all scale, and a character can carry them.
6. **Doing, editing and creating with beings.** Layer edits with consent,
   interpreter answers on beings, species `transform` by universe rules, and
   `create` for beings behind its setting.
   - *Proof:* `/give the guard a red cloak` adds a layer only when the guard
     agrees. A cursed character becomes another species and still remembers
     the player. A refused edit changes nothing.
7. **Flight and riding.** Altitude, clearance, landing, flying as part of
   `goto`, the `ride` and `dismount` verbs with consent, traveller steering
   and camera, and mounts that bolt in fear.
   - *Proof:* a griffin flies over a house to its owner across a lake, lands,
     and is greeted. It never passes through a solid. The traveller rides a
     horse that knows them, steers it with the walk keys, and the horse bolts
     and throws them when a wolf appears. A stranger's horse refuses them. A
     character rides a griffin to the next village, and both arrive.
8. **Breeding and drift.** Offspring with blended sliders, growth, family
   relationships, population caps, lineages, drift, and new varieties and
   species past a threshold.
   - *Proof (`pocket sim --days 30`):* a dog pair has pups that grow up and
     follow their parents. The population stays under its cap. A wolf line fed
     by villagers becomes measurably tamer and is named as a new variety. The
     same seed gives the same lineage history.

## Limits and performance

- Bodies on screen: the renderer already culls to 1,500 instances and walks a
  grid. The cost to watch is the cost of each body's shape, which the probe
  enforces. The human figure (about 12 capsules) sets the budget.
- Minds: `instinct` minds run only the scorer, at a lower rate when far away, and
  never use the LLM. The LLM budget (`POCKET_LLM_PER_MIN`) still goes nearest
  first, sapient minds first.
- New limits as settings: `POCKET_SIM_MAX_CREATURES` (per region and in total),
  `sim.hunting` (on or off), and the size range.
- `pocket sim` metrics gain: events between different species, predator chases,
  pets following owners, and species attitude drift. If these stay flat, the
  taxonomy isn't doing anything.

## Decided

- The player stays the traveller (a human-shaped body). Their size and dress
  can vary, but playing as another species is out for now.
- Breeding and change over time are in (phase 8).

- Mixing is not limited per universe by default: any two species that share a
  body module can have young together, and a universe can allow more.
- Once a line drifts into a new species, everyone uses the new name, including
  memories and village talk from then on.

## Parking lot

- Layers that change a body's roles or abilities (a saddle with wings, a
  jetpack). Wait until testing shows what people actually try to do with
  bodies, then decide.
