# Pocket Universe

An infinite, colour 3D world in your terminal. You walk with the arrow keys, talk to
characters who remember you, and type to create new things. An LLM writes the world
as code while you're in it. Rendering runs on the GPU; the terminal is just one view.

The world is alive between your visits: things have properties (they burn, bounce,
break, grow), simple rules act on those properties, objects can carry their own
behaviour code, and characters have needs, relationships and plans. They pick things
up, carry them home, play catch, carry logs together, hug, gossip, and, with an LLM,
make new things for their own reasons. You, the characters and a test agent all act
through the same small set of verbs.

```bash
export ANTHROPIC_API_KEY=...        # or POCKET_LLM_BASE_URL for any OpenAI-compatible endpoint
pocket new "a rainy coastal valley where the lighthouse keeper vanished"
pocket                              # reopen your last universe
pocket list                         # choose a world with the arrow keys, or press n for a new one
```

## Build

Rust (stable) and a GPU with Metal, Vulkan or DirectX 12. Without a GPU adapter it
falls back to a CPU renderer at reduced resolution (with a warning in the status bar).

```bash
cargo build --release      # → target/release/pocket
```

## Playing

| Mode | Enter with | Keys |
|---|---|---|
| Walk (default) | `Esc` | `↑` `↓` move, `←` `→` turn, `A`/`D` strafe, `Tab` blocks/ASCII, `F1` stats, `F2` inspect, `PgUp`/`PgDn` look, `q` quit |
| Hands | (in walk) | `e` use (what you hold, on what you point at), `g` pick up / put down, `f` throw, `y`/`n` answer someone's question |
| Mouse | (in walk) | point at things; left click: use; right click: pick up / put down; hold the right button and release: throw (longer is harder) |
| Talk | `Enter` when someone is within 4 m and in view | type, `Enter` sends, `Esc` back to walk |
| Do | `:` | anything in words: `:rub the stone on the lantern`, `:carve my name in the door` |
| Create | `/` | `/a a lighthouse on that hill`, `/undo`, `/history`, `/help` |

More commands: `/wave`, `/bow`, `/nod`, `/cheer`, `/dance`, `/sit`, `/hug NAME`,
`/kiss NAME`, `/handshake NAME`, `/highfive NAME`, `/give NAME`, `/say TEXT`,
`/propose NAME catch|carry|dance|walk|…`, `/drop`, `/do TEXT`, `/gesture ANY [NAME]`.
Contact gestures need the other person's consent: characters decide by how they feel
about you. A gesture nobody knows yet (`/gesture salute`) is written once by the LLM as
key poses, kept with the world, and anyone can do it after.

`F2` shows the raw truth about what you point at: a thing's properties, state, origin,
behaviour code and the rules that last fired on it; a character's needs, plan, goal,
relationships, recent decisions and memories.

- Terminals with the kitty keyboard protocol (Kitty, Ghostty, WezTerm, foot, …) report
  key releases, so holding `↑` walks at constant speed and stops the moment you let go.
  Elsewhere every press/repeat event takes one short step.
- Truecolor is used when `COLORTERM` says so; otherwise 256 colours.
- The world saves continuously into one `.pocket` file (SQLite). Copy it to fork a world.
- Universes live in `~/.pocket/universes/` unless you pass `--out FILE`; the debug log
  is `~/.pocket/pocket.log`.

## Configuration

| Variable | Meaning |
|---|---|
| `ANTHROPIC_API_KEY` | Use the Claude API. |
| `ANTHROPIC_WORKSPACE_ID` | Workspace for user-scoped keys (`sk-ant-usr-…`), sent as `anthropic-workspace-id`. |
| `POCKET_LLM_BASE_URL`, `POCKET_LLM_API_KEY` | Any OpenAI-compatible endpoint (Ollama, LM Studio, vLLM…), e.g. `http://localhost:11434/v1`. |
| `POCKET_MODEL_BUILDER` / `_CHARACTER` / `_DECIDER` / `_SUMMARIZER` | Model per role. `POCKET_MODEL` sets all. |
| `POCKET_BUDGET_USD` | Pause generation and dialogue when this session has spent this much. Walking keeps working. |
| `POCKET_REGION_RADIUS` | How many regions ahead to plan (default 2, as specified; each plan is one builder call plus one call per new object type). |
| `POCKET_DECIDER_URL` | Plug in an external decision model (e.g. Jev): it receives the event JSON and returns `{"action", "line"}`. |
| `POCKET_PRICE_IN` / `POCKET_PRICE_OUT` | $/M tokens for models the built-in table doesn't know. |
| `POCKET_FPS` | Frame-rate cap (default 60). |
| `POCKET_NO_GPU=1` | Force the CPU renderer. |
| `POCKET_NO_MOUSE=1` | Don't capture the mouse (keeps the terminal's own text selection). |
| `POCKET_SIM_NEAR` | Full simulation within this many metres of you (default 220). |
| `POCKET_SIM_MEDIUM` | Reduced-rate simulation up to here (default 512): the world keeps changing while you're away. |
| `POCKET_SIM_MEDIUM_HZ` | Ticks per second at medium distance (default 2). |
| `POCKET_SIM_FAR` | `frozen` (default) or `catchup:HOURS`: when you come back, run the missed time quickly with rules only. |
| `POCKET_LLM_PER_MIN` | Budget of LLM decisions per real minute for the living world (default 12; nearest characters first). |
| `POCKET_SIM_MAX_AWAKE`, `POCKET_SIM_MAX_FLAMES`, `POCKET_SIM_MAX_THINGS`, `POCKET_SIM_CHAT_RANGE`, `POCKET_SIM_RULES_HZ`, `POCKET_SIM_BEHAVIOR_HZ`, `POCKET_SIM_MEDIUM_LLM` | Further limits. Each can also be set per universe in its `kv` table as `sim.<name>`. |

Defaults on the Claude API: `claude-opus-5-5` for the builder (region plans and object
code) and for characters, `claude-haiku-4-5` for the fast decider and summariser.
Opus requests opt into server-side refusal fallbacks. The running cost of the session
is in the status bar, and every call is recorded in the `llm_usage` table.

With no key at all, the procedural world still works fully; stories and dialogue are
switched off with a one-line notice.

## How it works

```
main thread ── input, sim tick, culling, compositing, terminal output (one write/frame)
render thread ─ wgpu device: raymarch compute pass → storage buffer → 3 readback buffers
tokio runtime ─ LLM calls (SSE streaming), job orchestration
worker pool ── parse, allowlist, translate, probe (spawn_blocking)
committer ──── placement checks, shader builds, GPU parity, SQLite commit, flip
```

**Rendering** (`src/shaders/*.wgsl`, `src/render/`). One compute invocation per pixel
raymarches a heightfield (fbm + ridged noise blended across biomes) and the visible
instances. The CPU culls instances by view distance and frustum and uploads them; above
64 visible instances an 8 m XZ grid is built and each ray walks it (DDA) to collect a
sorted list of the bounding spheres it crosses. Each march step only evaluates spheres
the ray is inside, behind a tight local-box test. Distant terrain is read from a
camera-centred heightmap (512², rebuilt on the GPU every 24 m), with the exact noise
used near the camera. Shading: sun/moon with soft shadows, sky ambient, AO, distance fog,
sky gradient with stars, and water with Fresnel reflections. One in-game day lasts 20
real minutes.

**Terminal** (`src/term.rs`). Half-block cells (`▀`, foreground = top pixel, background =
bottom) or coloured ASCII. Each frame is diffed against what is already on screen; only
changed cells are sent, colour codes only when they change, inside a synchronized-update
block, in one write. Never clears. Raw mode, alternate screen and keyboard flags are
restored on `q`, Ctrl-C, SIGTERM/SIGHUP or a panic.

**The world** (`src/world/`). 64 m chunks, 4×4-chunk regions. Terrain, biomes and scatter
(trees, rocks, bushes, grass) are pure functions of the seed and are never stored. The
noise and terrain exist twice (Rust and WGSL) and are tested to agree within 1e-3.
The story layer is LLM-written: when the player comes within 2 regions of an unplanned
region, one call plans it (name, mood, lore facts, landmarks, a settlement, characters),
new object types are generated, and the result fades in (dithered) when ready, with a
log line like *"The fog lifts over Pinewood Vale."* Region planning waits for the
universe's look (palette, biomes, base types) from the first call at `pocket new`.

**Object types** (`src/lang/`). The model writes a small module in a strict JS subset:

```js
export const meta = { name: "lighthouse", bounds: [3, 14, 3], tags: ["building", "landmark"] };
export function sdf(x, y, z, k) {
  const tower = cappedCone(x, y - 6, z, 6, 1.6, 1.0);
  const lamp  = sphere(x, y - 12.5, z, 1.1);
  return smoothUnion(tower, lamp, 0.3);
}
export function color(x, y, z, k) {
  if (y > 11.5) return rgb(255, 230, 160);
  return (floor(y / 2) % 2 == 0) ? rgb(220, 60, 50) : rgb(240, 240, 235);
}
```

It is never executed as JS. The pipeline is:

1. **parse** (oxc, errors with line numbers)
2. **allowlist** (every node checked while lowering to a typed IR; anything not
   explicitly allowed is rejected: `while`, unbounded or variable `for`, recursion,
   helper functions, closures, `new`, `this`, `import`, strings outside `meta`,
   unknown identifiers, …)
3. **translate** to WGSL and to a verified register bytecode with a fuel limit
4. **probe** on the CPU (2,000 points: finite values, valid colour, fuel under budget,
   non-empty, inside bounds)
5. **placement** (on the terrain, not floating, not overlapping the player, characters,
   the spawn point or other instances)
6. **GPU build** (naga-validated, compiled on a background thread) and a GPU/CPU parity
   probe

Steps 1, 2, 4 and 5 feed exact errors back to the model for up to two repairs. Only
then is a new version committed to SQLite and the renderer flipped. The world snapshot
and its pipeline travel together in every frame request, so a frame never sees a
half-applied version. `/undo` reverts the last creation using the cached pipeline.
The full language reference the model sees is in `src/prompts.rs`; `pocket check
file.js` runs steps 1–4 on a file.

**Behaviour and properties** (`src/lang/`, `src/sim/props.rs`). A type can also say
what it is made of and what it does on its own:

```js
export const meta = { name: "oil lantern", bounds: [0.12, 0.2, 0.12], tags: ["item", "light"],
  props: { mass: 1.2, light: 1, heat: 150, fragile: 0.7, burns: 0.6, fuel: 0.5 },
  says: ["The flame steadies."], sounds: ["clink"], spawns: ["ash"] };
export function tick(s, w, k) { s.s0 += w.dt; if (w.water > 0) { w.light = 0; sound(0); } }
export function use(s, w, k, o) { s.s1 = 1 - s.s1; if (w.on > 0) { o.temp += 100; } }
export function touch(s, w, k) { if (w.impact > 6) { w.health -= 0.5; } }
```

Behaviour runs on the CPU bytecode VM with a fuel limit; it changes its 8 state slots
(`s.s0`…`s.s7`, which the shape reads as `k.s0`…`k.s7`, so a door can open or a fruit
ripen) and properties (`w.<prop>`, `o.<prop>` for the thing it is used on), and queues
effects (`say`, `sound`, `spawn`, `transform`, `remove`) that the world applies with
caps. The probe runs behaviour through typical situations and rejects code that is too
expensive or produces NaN. Property names must be ones the universe knows.

**The living world** (`src/sim/`). One `Sim` holds everything that changes: actors
(the player and the characters, with the same body), live things, the rules and the
characters' minds. It runs in the game, headless in `pocket sim`, and under `pocket act`.

- *Live layer.* Most of the world is static and versioned (placed objects, procedural
  scatter). A thing joins the live layer when it is touched, thrown, set alight or has
  behaviour; the static copy is hidden. Live things are saved continuously.
- *Toy physics.* Gravity, bounce, friction and grip on slopes, buoyancy in water. Things
  are spheres against the terrain and the signed distance fields of nearby solids, so a
  ball bounces off any shape the LLM wrote. Things at rest sleep. A thing falling through
  an opening (a hoop, a well) is noticed, whatever the shape.
- *Properties and rules.* 18 built-in properties (`mass`, `bounce`, `burns`, `temp`,
  `wet`, `light`, `edible`, `alive`, `fragile`, `fire`, `fuel`, `heat`, `char`,
  `health`, `growth`, …). Rules are data (`src/sim/rules.rs`), applied a few times a
  second to live things, changed scatter cells and placed objects next to something
  happening: heat spreads and ignites what burns, fire consumes fuel and chars, water
  soaks and douses, living things grow, broken lamps spill burning oil. A universe's
  genesis can add its own properties and rules (a curse that spreads by touch); they
  are tested in a small scene first and rejected if they blow up or spread to
  everything at once. Burnt and eaten plants grow back after a day or so.
- *Looks.* Charred, wet, glowing and highlighted are generic per-instance effects in the
  shader; fires get animated flames and up to 8 point lights light the night.
- *The interpreter.* When an action falls outside the rules and the things' own code,
  the LLM decides what happens, but must answer as property changes, state, new things
  (written by the builder if new) and removals. Answers are cached by (what, with what,
  on what), so the same cause gives the same effect.
- *Minds* (`src/sim/npc.rs`). Needs (hunger, tiredness, loneliness, boredom, curiosity)
  drift; traits come from the persona's words. When idle, a character scores a few
  options (eat, rest, seek company, play, look at something new, flee a fire, gather
  loose things home, toss stones, wander) and turns the best into a plan of shared
  actions. For big moments (the player comes near, something appears, boredom, a gift)
  the LLM writes a goal and steps, within a per-minute budget, nearest first.
- *Life together* (`src/sim/social.rs`). Relationships (affection, trust, rivalry,
  family, partners) are seeded from the personas and change with what people do.
  Proposals and shared activities are built from primitives: catch is throw plus catch,
  carrying a log is two holds on something too heavy for one, a hug is a contact gesture
  both agree to. Characters who meet talk (through the LLM if you can hear them,
  otherwise in a few plain words about what they saw) and pass on their most important
  memories.
- *Range.* Full simulation near you, a reduced rate at medium distance, frozen or
  caught up (rules only) further out. Every limit is a setting.

**Characters** (`src/sim/npc.rs`, `src/brain.rs`). Built from one parametric figure type
with poses (arms, lean, nod, crouch) driven by the simulation. Every exchange is stored
as a memory, with a rolling summary refreshed by the summariser; the dialogue prompt is
bible + persona + summary + the most relevant and most recent memories + region facts +
what is around right now.

**Storage** (`src/db.rs`). The tables from the spec (`universe`, `versions`, `types`,
`instances`, `regions`, `characters`, `memories`, `summaries`, `player`, `llm_usage`),
plus a small `kv` table for the spawn point and current version, and the live world:
`things`, `cells` (changed scatter), `spent_cells`, `relationships`, `vocab` and `rules`
(the universe's own), `interp_cache`, `events` (the latest 5,000) and `origins` (who
made what). Only LLM-written source is stored; WGSL and bytecode are rebuilt on load.
Opening an older world adds the new tables and built-in types.

## Tools

```bash
pocket snapshot FILE --at 0,0,90 --size 120x40 [--ascii] [--mono] [--time 21]   # one frame as text
pocket describe FILE --at 0,0,90                                                # visible things as JSON
pocket bench FILE --distance 2000 --size 250x70                                 # scripted walk, frame stats
pocket gpubench FILE --at 0,0,90 --size 250x61                                  # GPU time for one view
pocket check my_type.js                                                         # validate an object type
pocket selftest                                                                 # GPU/CPU parity + validator
pocket sim FILE --hours 24 --seed 5 --events out.jsonl                          # run the world headless, fast; report what emerged
pocket sim --replay out.jsonl --kinds ignited,caught                            # read a run back
pocket sim FILE --hours 3 --seed 5 --verify                                     # same seed twice: same history?
pocket act FILE --as Mara '{"do": "hold", "target": {"name": "stick"}}'         # act as anyone (walks there first)
pocket act FILE --stdin                                                         # one JSON command per line
pocket inspect FILE npc:3 | thing:12 | instance:7 | cell:10,20 | NAME | --look  # the raw truth, as JSON
```

`pocket sim` works on a private copy unless you pass `--save`, uses the LLM if one is
configured (`--no-llm` to run on rules alone), and prints metrics: events by kind,
things made per hour, how far things travelled, the longest cause-and-effect chain,
how often people talk about things, how much affection changed, and broken invariants.
If these stay flat, nothing is emerging.

Actions (for `pocket act`, LLM plans and the game) are JSON with a `do` field: `move`,
`turn`, `goto`, `hold`, `drop`, `place`, `throw`, `use`, `eat`, `do`, `create`, `say`,
`gesture`, `propose`, `answer`, `give`, `follow`, `wait`, `sleep`, `wake`, `go_home`.
Targets are `{"thing": ID}`, `{"actor": "player" | ID}`, `{"instance": ID}`,
`{"cell": [X, Z]}`, `{"point": [X, Y, Z]}` or `{"name": "ball"}` (the nearest match).

## Tests

```bash
cargo test --release                          # 58 tests, ~8 s
cargo test --release -- --ignored --nocapture # + 1.5 km walk with live generation, compile-time scaling, rendered PNGs
```

How the acceptance criteria are covered:

| Criterion | Where |
|---|---|
| Spawn and walk before story content exists | `pocket new` shows the first frame in under 0.1 s (plus the terminal's keyboard-protocol reply); genesis runs in the background |
| ≥ 60 fps at 120×40, ≥ 30 fps at 250×70, GPU < 2 ms | `pocket bench` / `gpubench`; F1 overlay shows fps, worst frame, GPU time (timestamp queries) |
| 2 km walk without a frame over 50 ms, regions keep appearing | `app::tests::long_walk_with_generation_has_no_hitches` (ignored, long) and `pocket bench` |
| Generation never blocks rendering or input | same test: 35+ regions and pipeline rebuilds during the walk |
| Malicious fixtures rejected at the allowlist step | `fixtures/malicious/*`, `lang::tests::malicious_fixtures_rejected_at_allowlist` |
| Broken fixtures repaired or rejected | `fixtures/broken/*`, `lang::tests::*`, floating placement + syntax repair in `e2e_tests` |
| CPU/GPU parity within 1e-3 | `render::tests::gpu_parity_types_and_terrain`, `pocket selftest` |
| Atomic flip, exact and instant undo | `WorldSnapshot::check_consistent` on every flip in `e2e_tests`; undo latency asserted |
| `/a lighthouse on that hill` while walking | `app::tests::talk_create_undo_via_keys`, `e2e_tests` |
| Character remembers after a restart | `e2e_tests` (reopens the file, checks the prompt and the reply) |
| Can't walk through solids; sliding | `world::collide::tests` |
| Terminal restored after q / Ctrl-C / panic | verified in a pty: raw mode off, echo on, alternate screen left, cursor shown |
| Copy a `.pocket` file → identical world that diverges independently | `e2e_tests` |
| Constant-speed held keys with keyboard enhancement | `app::tests::held_key_moves_at_constant_speed_and_stops_on_release` |
| Same seed, same history | `sim::tests::same_seed_same_history`, `pocket sim --verify` |
| The agent drives the player and a character through identical calls | `sim::tests::agent_drives_player_and_character_the_same_way` |
| A thrown stone bounces off an LLM-made lighthouse and comes to rest | `sim::tests::thrown_stone_bounces_off_a_lighthouse_and_sleeps` |
| A dropped lantern breaks, spills fire, burns the grass and a hut, stops at water | `sim::tests::dropped_lantern_starts_a_fire_that_spreads_to_a_hut_and_stops_at_water`, `a_firebreak_wider_than_the_heat_reach_stops_fire` |
| A universe's own rule (a spreading curse); explosive rules rejected | `sim::tests::universe_rules_spread_cursed_and_explosive_rules_are_rejected`, `brain_round_trip_rules_chat_and_new_types` |
| Behaviour code: an acorn on the ground becomes a sapling, which grows | `sim::tests::an_acorn_on_the_ground_becomes_a_sapling_and_grows` |
| The interpreter decides, and the second time the cache does | `sim::tests::interpreter_lights_the_lantern_and_caches_the_answer` |
| Unscripted: a bored character makes a ball; it is thrown at (and through) a hoop | `sim::tests::a_made_ball_ends_up_thrown_at_a_hoop`, `a_ball_thrown_at_a_hoop_goes_through` |
| Catch between people who love each other; two carry a log; a hug; a kiss refused | `sim::tests::people_play_catch_carry_together_and_hug` |
| 1,000 things and 50 people at over 10× real time | `sim::tests::a_thousand_things_and_fifty_people_run_fast` (~29×) |
| A village left two game days ago has changed | `sim::tests::a_village_left_for_two_days_has_changed` |
| Live things, cells, relationships survive a restart; eaten plants grow back | `sim::tests::live_things_cells_and_relationships_persist`, `eaten_plants_stay_gone_then_grow_back` |
| Keys and mouse: pick up, throw, F2, free-text do | `app::tests::grab_throw_inspect_and_do_with_keys_and_mouse` |

The LLM-dependent tests use a scripted model in-process (no network).

## Limitations

- The HD window view is not built yet, but the renderer takes any resolution and the same
  shaders would drive a surface.
- Story regions that fail to generate are retried next session, not in the same one.
- Bounding volumes come from each type's probed extent; a very thin feature hanging
  below its probed bottom could be clipped.
- The terrain makes lakes and coasts, not brooks: water stops fire because no fuel
  stands in it and heat reaches about 6 m.
- Physics is deliberately a toy: spheres against shapes, no stacking or joints.
- Characters' figures are one shape with a few pose channels; gestures are stylised.
- The LLM paths (plans, interpretation, overheard talk, new types on demand, universe
  rules at genesis) are tested with a scripted model; tune the prompts in
  `src/prompts.rs` against a real one with `pocket sim FILE --hours 1`.
