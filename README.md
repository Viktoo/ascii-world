# Pocket Universe

An infinite, colour 3D world in your terminal. You walk with the arrow keys, talk to
characters who remember you, and type to create new things. An LLM writes the world
as code while you're in it. Rendering runs on the GPU; the terminal is just one view.

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
| Walk (default) | `Esc` | `↑` `↓` move, `←` `→` turn, `A`/`D` strafe, `Tab` blocks/ASCII, `F1` stats, `PgUp`/`PgDn` look, `q` quit |
| Talk | `Enter` when someone is within 4 m and in view | type, `Enter` sends, `Esc` back to walk |
| Create | `/` | `/a a lighthouse on that hill`, `/undo`, `/history`, `/help` |

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

**Characters** (`src/world/characters.rs`, `src/brain.rs`). Built from one parametric
figure type. A state machine on the sim tick (wander near home, sleep at night, face
the player when talked to, walk up when they have something to say) with no LLM per
tick. Events (the player comes near, a building appears nearby) go to a pluggable
decider. Every exchange is stored as a memory, with a rolling summary refreshed by the
summariser; the dialogue prompt is bible + persona + summary + the most relevant and
most recent memories + region facts + what is around right now.

**Storage** (`src/db.rs`). The tables from the spec (`universe`, `versions`, `types`,
`instances`, `regions`, `characters`, `memories`, `summaries`, `player`, `llm_usage`),
plus a small `kv` table for the spawn point and current version. Only LLM-written source
is stored; WGSL and bytecode are rebuilt on load.

## Tools

```bash
pocket snapshot FILE --at 0,0,90 --size 120x40 [--ascii] [--mono] [--time 21]   # one frame as text
pocket describe FILE --at 0,0,90                                                # visible things as JSON
pocket bench FILE --distance 2000 --size 250x70                                 # scripted walk, frame stats
pocket gpubench FILE --at 0,0,90 --size 250x61                                  # GPU time for one view
pocket check my_type.js                                                         # validate an object type
pocket selftest                                                                 # GPU/CPU parity + validator
```

## Tests

```bash
cargo test --release                          # 30 tests, ~3 s
cargo test --release -- --ignored --nocapture # + 1 km walk with live generation, compile-time scaling
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

The LLM-dependent tests use a scripted model in-process (no network).

## Limitations

- The HD window view is not built yet, but the renderer takes any resolution and the same
  shaders would drive a surface.
- Story regions that fail to generate are retried next session, not in the same one.
- Bounding volumes come from each type's probed extent; a very thin feature hanging
  below its probed bottom could be clipped.
