# Starter world: Santa Monica, the world every player starts with

Hand-off plan, 2026-10-18. Built on branch `vik/game-ui-settings-overhaul-15ea28`
(menus as buttons; read `docs/ui-plan.md` first).

## Goal

One beautiful, hand-checked world ships with the game: **Santa Monica** —
the pier with its Ferris wheel and rides, tourists and fishermen on the
boards, the beach and its beach-goers, Palisades Park on the bluff above
(palms, the walk people take to watch the sun set over the Pacific), and the
buildings around them, tapering off into ordinary generated land. A real, busy, complex place as the first thing
a player sees says "truly anything is possible here". It is costly to make
once and free for every player after. It does two jobs:

1. **The title screen's backdrop.** The real game runs Santa Monica behind
   the menus near sunset, cutting between a few fixed shots (see step 3):
   people walk the pier, the wheel turns if it can, waves come in, the sun
   goes down over the ocean.
2. **A world new players can step into with one click**, with no wait and no
   generation cost (the AI is then only used for people talking, deciding,
   and `/` actions, capped by the budget setting).

## Why this approach (decided with the user)

- New players have no worlds; the first screen must look great anyway.
- A separate "backdrop renderer" was rejected: it would be a second way of
  drawing the world to maintain, and it would break. The title must be **the
  real game** (same renderer, same sim, same auto detail), so every
  improvement to the game reaches the title for free, and a machine that can
  run the title can run the game.
- Making things is the costly part of a world (in real worlds, new kinds of
  things were $4–11 of $8–16; genesis < $0.50, regions $1–3). Shipping a
  finished town removes that cost for players who just want to try it.

## Facts about generation (as of this branch)

- Land is cut into regions of 256 × 256 m (`world::REGION`).
- Loading a new world waits until genesis and every region within 110 m of
  the player are made (`app/loading.rs`, `SETTLE_DIST`), usually 1–4 regions.
- After that, regions keep being made in the background within
  `settings.region_radius` regions each way (default 2: a 5 × 5 block,
  ~1.3 km), following the player (`App::schedule_regions`). It never
  "finishes"; walking further makes more.
- Without an LLM the land, built-in plants and rocks, and anything already
  made still show; nothing new is made and people don't talk.

## Steps

### 1. Make Santa Monica (one-time, by us)

- **Scope: small and exact.** Only the pier, the beach between it and the
  bluff, Palisades Park on the bluff, and a ring of surrounding buildings
  (Ocean Avenue's front, a few blocks) at true scale. Not the whole city.
- **Edges taper.** Ordinary generation will never be as intricate as this
  core, so the hand-checked area must blend into it: density, height and
  detail fall off towards the edges (fewer, plainer buildings, then the
  biome's own land), so where made-by-hand ends and generated begins doesn't
  show. Plan this transition from the start, not as a fix later.
- Write a strong genesis prompt: Santa Monica, California, today: the pier
  reaching into the Pacific with its Ferris wheel and small amusement park,
  food stands, fishermen at the end; the beach with beach-goers, lifeguard
  towers, volleyball; Palisades Park on the bluff with tall palms and benches
  facing the sunset; the buildings along Ocean Avenue behind; tourists,
  locals, street performers.
- **Real names and logos are fine where they match reality** (the user's
  call): the real businesses on the pier and Ocean Avenue as they are.
  Billboards and anything not mapped to a real one get something made up.
- It is dense: expect more kinds of things than a village, a bigger file,
  more generation cost (maybe $30–60 with iteration) and a heavier frame.
  Check it runs smoothly at the default auto detail.
- Make it fresh with the latest generation (current main). Ignore older
  worlds in the user's folder, such as their LA one: they were made by much
  older builds.
- Generate in a scratch `POCKET_HOME`, never in the user's `~/.pocket`.
- Make the core regions (pier, beach, park, the ring of buildings) and the
  tapering edge around them; beyond that, leave it to ordinary generation
  when players walk there (`pocket sim` / `pocket act` exist for headless
  runs).
- Check views with `pocket snapshot FILE --at x,z,yawDeg --png out.png
  [--time H] [--pitch D] [--lift M]`, and the whole UI with `--shot`
  (see README). Regenerate weak regions or things until it looks right.
- Budget: roughly $15–30 plus iteration.
- Strip the player and spend history before shipping: `share::make_copy`
  already makes a clean copy (no player state, no keys).

### 2. Ship it and the one-click visit

- Ship the file with the game (likely 5–10 MB; check size; consider
  compressing it in the download). It must not depend on network access.
- Add a **"Visit Santa Monica"** button on the welcome card and the title
  (`src/ui/screens.rs`). It copies the town into `~/.pocket/universes` as the
  player's own world (`share::take_in` does most of this) and opens it.
  Visiting twice should open the existing copy, not make a second one
  (or ask).
- The welcome card says plainly what works without a key (walk, look) and
  what needs one (people talking, making things).

### 3. Make the title the real game

- Today the backdrop is `src/panorama.rs`: it loads a seed-7 "showcase"
  world and asks the renderer for frames itself (a small copy of
  `App::request_frame`). **Delete it** at the end of this step, together with
  `Panorama`, `panorama::showcase`, `panorama::vista` and the hidden
  `pocket showcase` command. Keep `ui::title::Backdrop` (the hills of words)
  only as what shows while the town loads, or with no GPU at all.
- Instead, the title runs an `App` on (a read-only or throwaway copy of)
  Santa Monica: player hidden and not simulated as a body, camera fixed at a
  chosen spot facing east, HUD hidden (no log, hints, status bar), sim
  ticking normally, no LLM calls (`llm: None`; the town's people move by the
  base layer), nothing saved back to the shipped file.
- **Shots.** A short list of fixed camera shots, each a spot, a direction and
  a pitch, held ~12 s each, the title fading to dark for about a second
  between them (one world drawn at a time; no cross-fade of two frames).
  For example: Palisades Park bluff looking west at the sunset; along the
  pier towards the Ferris wheel; the beach with people at the water; the pier
  from below on the sand; the shops along Ocean Avenue. Shots stay inside the
  hand-checked core. The shots are data (a list in
  code, or in the world's kv table) so they can be tuned without new code.
  The time of day starts near sunset and runs at the game's own pace.
- The menus (`ui::screens::menus`) then draw over the running App each frame
  instead of over their own screen. `main.rs::session` currently runs menus
  *before* play (`Start::Title` → `ui::screens::menus` → `Start::Play`);
  this becomes "play the title App with the menus open". The pause menu in
  `app/menu.rs` already shows how menus draw over a running world
  (`draw_menu`, `ui.begin/end`, `fill_world`).
- Weak machines: rely on the game's own auto detail (`settings.window_detail`
  AUTO) and its frame-rate cap; maybe cap the title at ~20 fps and pause it
  when the window loses focus. Do not add a separate rendering path.

## Decided

- Past the pre-made area the starter behaves exactly like any other world:
  same world rules, same generation (which costs tokens), same character
  behaviour. Nothing special-cased.
- The starter is Santa Monica: the pier, the beach, Palisades Park and the
  buildings around them at true scale, tapering into ordinary land. Not the
  whole city, not a made-up town.
- Real brand names and logos where they match reality; made-up billboards.

## Decisions still open (ask the user)

- The exact shots and their order (the sunset from Palisades Park is the
  favourite to open on).
- Whether "Play together" should suggest hosting Santa Monica by default.

## Out of scope here

- Windows / Linux / Intel Mac builds (separate task: the release workflow
  only builds Apple Silicon today; see `docs/releases.md`).
- A rotation of several worlds behind the title (considered; the single town
  replaces it).

## Working rules in this repo

- Tests: `cargo test` after each edit (debug, ~4 s); before merging
  `cargo test --release --features slow-tests`. Keep new tests lean (see
  `CLAUDE.md`).
- `app::together::tests::a_guest_sees_the_living_world` sometimes fails
  (a character looks different to host and guest); it predates this work.
- End every change with `cargo build --release`; the user runs the release
  binary at once (their `pocket` alias currently points at this worktree's
  `target/release/pocket`).
- Commit messages: plain prose, no Co-Authored-By trailer, no "Generated
  with" line.
- Write plans and questions to the user as plain text, not button prompts.
