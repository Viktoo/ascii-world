# Plan: a pocket universe that surprises you

## Status

All eight phases are built (`src/sim/`, plus language, renderer and brain changes).
Each phase's proof scenario is a test in `src/sim/tests.rs`; the README lists them.
Where the build differs from the plan:

- **Properties:** 18 built in. Six more than planned (`fire`, `fuel`, `heat`, `char`,
  `health`, `growth`) because the rules needed state to act on.
- **Streams:** the terrain makes lakes and coasts, not brooks, so the fire proof uses a
  real water crossing with grass planted on both shores. A separate test shows any
  fuel-free gap wider than the heat's reach (about 6 m) stops fire.
- **Breaking:** fragile things split into smaller copies of themselves that keep their
  properties (a broken lamp still holds burning oil), not a cut-up SDF.
- **Throw:** `f` throws at what you point at, or lobs it to the ground you point at.
  With a mouse, hold the right button to wind up.
- **Added, not in the plan:** regrowth (eaten or burnt plants come back after a game
  day or so), and two cheap no-LLM behaviours: gathering loose sticks and stones into
  piles at home, and tossing stones for fun. Without these, a world with no LLM was
  mostly greetings.
- **Joint effort:** someone holding a thing too heavy for one person only grips it. It
  lifts when a second person takes the other end.
- **New gestures:** an unknown gesture name gets its key poses written once by the LLM
  (`/gesture salute`, or in a character's plan). It is stored in the world's `gestures`
  table and anyone can do it after.

**Vision:** anything you can describe can exist in the world, behave on its own,
and be used by anyone: the player, a character or the agent. Nobody designs the
stories. They come from things, rules and people meeting.

## Principles

1. **Few verbs, many nouns.** Keep the set of actions tiny and the set of things
   unlimited.
2. **Rules act on properties, never on names.** Fire burns anything that
   `burns`, not "wood" or "the hut".
3. **What the LLM invents must land in primitives**, as code, properties or
   state, never only as text. Its inventions then follow the same rules as
   everything else.
4. **Everyone has the same powers.** The player, characters and the test agent
   use the same verbs, including create.
5. **Everything persists and can be perceived.** Characters can see the
   aftermath of what happened.
6. **Same cause, same effect.** Results are cached, so the world is learnable.

## What exists today

- Objects are LLM-written programs in a strict JS subset (`src/lang/`). They
  compile to WGSL for the GPU and to bytecode for the CPU, and `meta` already has
  `bounds` and `tags`. Each instance gets 8 floats (`k`) that the code can read.
- There's CPU SDF collision with sliding (`src/world/collide.rs`).
- Instances are versioned rows in an immutable `WorldSnapshot`, which is what
  makes `/undo` work. Characters are the only thing that moves. They're uploaded
  to the GPU every frame (`Cast::instances`).
- Characters have personas, memories, summaries and a pluggable `Decider`.

## The seven primitives

### 1. Things

Everything is one kind of object: shape code, plus properties, plus optional
behavior. Items, buildings, creatures and tools are all things, and an "item" is
just a thing small enough to hold.

- Extend `meta` with `props` (see primitive 4) and optional behavior functions
  (see primitive 5).
- Add a **live layer** next to the versioned snapshot. Most things stay static
  and versioned. A thing becomes *live* when it's touched, thrown, burning or has
  behavior. Live things have position, velocity and an 8-float `state` that
  changes every tick. They're saved periodically, the same way character
  `state_json` is.
- Live things are uploaded per frame, like characters. `state` maps onto `k`, so
  shape code can read it: a door's angle or a fruit's ripeness is visible for
  free.

### 2. Hold

There's one physical verb: any body can attach a thing to itself.

- `Hold`, `Drop`, `Place`, `Throw { dir, force }`. A held thing follows the hand
  position, and throwing releases it with velocity.
- What you can hold comes from `bounds` and `mass`. A boulder is too heavy, but
  maybe two characters together can lift it later.

### 3. Toy physics

This is gravity, velocity, bounce and friction, not a full game engine.

- Bodies are spheres or capsules sized from `bounds`. They collide with
  terrain and with the SDF of nearby solids. Normals come from the SDF gradient
  (central differences).
- Because LLM-made shapes are SDFs, collision works on any invented shape with
  no extra work. A ball bounces off a hoop the LLM wrote 5 minutes ago.
- Bodies sleep when they come to rest. Only awake bodies cost anything, with a
  cap of a few hundred.
- Thing-against-thing collision uses sphere approximations. Stacking, joints
  and ropes come later, if ever.

### 4. Properties and universal rules

This is a small shared vocabulary that the LLM assigns and the rules read.

- v1 vocabulary: `mass`, `bounce`, `friction`, `solid`, `burns`, `temp`, `wet`,
  `light`, `edible`, `alive`, `fragile`, `conducts`. Unknown properties are
  rejected at compile time, like unknown APIs.
- The rules live in a data table, so adding a rule is one line. For example:
  - `temp > burn_point` and `burns` → on fire → raises `temp` of neighbors → ash
  - `wet` + on fire → extinguish
  - `fragile` + impact over a threshold → breaks into pieces (a generic split of
    the SDF)
  - `alive` → grows over time (scale/state drift) and needs `wet`
- Neighbors come from a coarse spatial grid, and rules tick a few times a
  second, not every frame.
- Generic transforms (burnt, broken, grown) don't need the LLM. They change
  props, state and color tint.

**Universe rules.** A universe's bible can add its own properties and rules.

- At `pocket new`, the genesis call can return extra properties (for example
  `cursed`, `holy` or `magnetic`) and rules written in the same data format. A
  magic world might say "`cursed` spreads to things held for more than an hour"
  or "`holy` + `cursed` → both destroyed".
- They're stored in a new `rules` table. They're checked like code: a rule may
  only use known properties (built-in plus this universe's), and a probe run in
  a test scene catches rules that explode, such as something spreading to
  everything in seconds.
- The built-in rules always apply. A universe can add rules but not remove the
  base physics, so every universe still feels like a real place.

### 5. Behavior code

Things can act on their own using the same sandboxed language.

- New optional functions: `tick(s, k, dt)`, `on_use(s, k, actor)` and
  `on_touch(s, k, other)`. They return a new 8-float state.
- Effects go through a tiny API that queues requests rather than running them:
  `emit(prop, amount)`, `spawn(type, offset)`, `sound(text)`, `say(text)`. The
  engine applies them with per-tick caps.
- They run on the existing VM with its fuel limit, so a bad script can't stall
  the world. They're checked and probed like `sdf`/`color` today.
- Examples: a clock that ticks, a seed that grows into a tree, a door that
  opens on use, a lantern that emits `light` and `temp`.

### 6. The LLM as interpreter

When an action falls outside the rules, the LLM decides what happens.

- Resolution order for `Use(A)`, `Use(A, on: B)` or free text `Do("…")`:
  1. universal rules (props)
  2. the thing's own `on_use` / `on_touch` code
  3. the LLM interpreter
- The interpreter must answer in an **effect schema**: create things (new code,
  run through the existing compile/probe/repair pipeline), change props or
  state, move, destroy, say. Anything outside the schema is rejected.
- Results are cached by `(verb, type A, type B)`. The same combination gives the
  same result every time, which makes the world learnable and keeps it cheap.
- Things record their origin (`made_from`, `made_by`), so characters can
  remember and talk about where things came from.

### 7. Everyone can create

Characters get `Create` and `Do`, just like the player.

- The decider can return a create intent ("make a fishing rod"). It goes
  through the same brain pipeline and budget as player `/create`.
- Creations are attributed to their creator, which feeds memory, pride,
  ownership and trade.
- Characters' needs and wants (hunger, warmth, curiosity, rivalry) give them
  reasons to hold, use and make things, so they don't just chat.

## Doing things together

Shared activities should come from the same primitives, not be special-cased.

- **Catch** needs no new code: `Throw` toward a person plus `Hold` when a thing
  arrives within reach. A father and son playing catch is two characters who
  like each other, one ball, and a decider that picks "throw it back".
- **Gestures** are a new body primitive: `Gesture { to, kind }`, such as
  wave, hug, kiss, handshake, high-five or bow. Gestures need two things:
  - **Poses for figures.** `figure.js` currently has only a walk phase. It gets
    pose slots (arm angles, lean, head turn), which means widening the per-instance
    params. Gesture kinds are short pose animations, and new ones can be written
    by the LLM as data.
  - **Two-body alignment.** Both bodies walk to a meeting distance and face each
    other, and the gesture plays only when both agree. Either one can refuse.
- **Joint effort:** `Hold` on a thing too heavy for one person waits for a
  second holder, and their strength adds up. Two characters carrying a log, or
  four raising a beam, comes out of `mass` and nothing else.
- **Proposals:** one actor suggests an activity (`Say` plus an intent, such as
  "let's play"), and the other accepts or refuses based on needs and
  relationship. While both are in it, each one's decider gets the other's
  actions as events.
- Relationships (trust, affection, rivalry) change with shared activities,
  which leads to friendships, couples and rivals.

## Input

- Turn on crossterm mouse capture. The cursor points at a thing, and picking
  casts a ray from the camera through that cell (CPU SDF march, or an
  instance-id readback from the GPU pass).
- **Left click** = use. **Right click** = hold or drop. Holding right click and
  releasing throws.
- **Typing** is the open-ended verb: `/create` already exists, and free text
  becomes `Do`.
- The agent uses `pocket act FILE --as player|<cid> '<action json>'`, with the
  same verbs and targets given as thing ids.

## Inspect view (admin)

`F2` toggles it (`F1` is already stats). It shows the raw truth of whatever the
cursor points at:

- **Thing:** type, props, live state, origin (`made_from`, `made_by`), behavior
  code, and the last few rules that fired on it.
- **Character:** needs, current activity and plan, relationships, recent
  memories, and the last decider input and output.
- **Ground:** region, biome, and which simulation tier it's in.

The agent gets the same data as JSON with `pocket inspect FILE <id>`. It's
built in phase 1 because every later phase is debugged with it.

## Simulation range

The world keeps changing at medium distance, so travelling around shows
progress. Every distance and budget is a setting, so it can be turned down for
cost without changing the design.

```rust
pub struct SimConfig {
    pub near: f32,            // full sim every frame (today's SIM_RANGE, 220 m)
    pub medium: f32,          // default 2 regions (~512 m)
    pub medium_hz: f32,       // ticks per second at medium distance (default 2)
    pub medium_llm: bool,     // may medium characters call the LLM?
    pub llm_per_min: u32,     // global decision budget, shared by all tiers
    pub far: FarMode,         // Frozen | CatchUp { max_hours }
}
```

- **Near:** everything runs every frame (physics, rules, behavior, characters).
- **Medium:** the same rules at a lower tick rate. Characters still act, trade,
  build and create. LLM calls are allowed, but nearer characters always go first
  for the shared budget.
- **Far:** frozen by default. With `CatchUp`, when the player comes back, the
  elapsed time is run quickly with rules only (no LLM), up to `max_hours`, and
  each character gets a summary memory of "while you were gone".
- The defaults are in code and can be overridden with environment variables
  (`POCKET_SIM_NEAR`, `POCKET_SIM_MEDIUM`, `POCKET_LLM_PER_MIN`, …) or the `kv`
  table, so they can be tuned per universe. `pocket sim` uses the same config.
- The design assumes every tier will eventually be "full". Lowering the numbers
  makes the world quieter, but nothing has to be rebuilt to raise them again.

## Shared action set

```rust
enum Action {
    Move { dir: Vec3 }, Turn { yaw: f32 }, LookAt { point: Vec3 },
    Hold { thing: ThingId }, Drop, Place { at: Vec3 }, Throw { dir: Vec3, force: f32 },
    Use { thing: ThingId, on: Option<Target> },
    Do { text: String },          // free text, interpreted
    Create { text: String },
    Say { to: Option<ActorId>, text: String },
}
```

There's a single `apply(world, actor, action)` path for everyone.

## Build order

Each phase ends with a **proof scenario** that becomes an e2e test with the
scripted LLM.

| # | Phase | Proof |
|---|---|---|
| 0 | Headless `pocket sim` with seeds and an event log | Same seed gives the same log twice |
| 1 | Shared actions, mouse picking, `pocket act`, `F2` inspect, `SimConfig` tiers | The agent drives a character and the player through identical calls, and `pocket inspect` shows the result |
| 2 | Live layer, Hold, toy physics | Throw a rock and it bounces off an LLM-made lighthouse, then sleeps |
| 3 | Properties, universal rules, universe rules from the bible | A lantern dropped in dry grass starts a fire that spreads to a hut and stops at a stream. In a magic universe, `cursed` spreads by its own rule |
| 4 | Behavior code | An LLM-made seed, once planted, grows into a tree over a game-day |
| 5 | Interpreter and cache | "Use flint on lantern" creates a lit lantern, and a second try hits the cache |
| 6 | Characters as full actors (hold, use, create, needs) | Unscripted: a character makes a ball and others start throwing it at a hoop |
| 7 | Together: poses, gestures, proposals, joint holds | Two characters who like each other play catch, and two others carry a log neither can lift alone |
| 8 | Scale: sleeping bodies, spatial grid, medium tier, far catch-up | 1,000 things and 50 characters run headless at over 10× speed. A village you left 2 game-days ago has visibly changed |

The basketball test in phase 6 is the real check. If nobody wrote "basketball"
anywhere and it happens anyway, the foundation is right.

## Testing emergence

- Every phase adds its proof scenario to `src/e2e_tests.rs`.
- Invariants after each sim run: things don't sink through terrain, held things
  have exactly one holder, and fire and spawn counts stay under their caps.
- Metrics from the event log: new things created per hour, how far things
  travel, how many cause-and-effect chains run longer than three steps, and how
  often characters mention things in conversation. If these stay flat, nothing
  is emerging.
- Keep interesting seeds as regression fixtures under `fixtures/sims/`.

## Risks

- **CPU SDF cost for physics:** the VM evaluates LLM code per query. Limit
  awake bodies, only test nearby solids, and later move collision queries to
  the GPU.
- **Runaway effects** (fire across the whole map, endless spawning): use
  per-region caps, conservation budgets, and rain or decay as natural limits.
- **The LLM tags properties inconsistently:** use a strict vocabulary, give
  examples in the prompt, and run a probe step that checks props against the
  shape (a 10 m "holdable" thing is rejected).
- **Undo vs live state:** undo applies to creations (versioned), not to physics
  or fire. This needs a clear rule.
- **Universe rules that break the world:** probe new rules in a test scene
  before they go live, and cap how fast any rule can spread.
- **Gestures look stiff:** the figure is one SDF with a few parameters, so
  poses are limited. That's acceptable at terminal resolution. Revisit once
  the GPU path has more per-instance data.

## Decisions

1. A universe's bible can add its own properties and rules on top of the
   built-in ones (see *Universe rules*).
2. Characters can do things together, from catch to hugs to carrying a log
   (see *Doing things together*).
3. An admin inspect view toggled with `F2`, plus `pocket inspect` for the agent
   (see *Inspect view*).
4. The world changes at medium distance, with every range and budget editable
   in `SimConfig` (see *Simulation range*).
