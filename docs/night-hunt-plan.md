# Night hunt: night walkers and phantoms

Agreed and built 2026-10-05 (`src/sim/night.rs`, `src/sim/phantom.rs`,
`src/builtin/phantom.js`, tests in `src/sim/tests/night.rs`). Replaces the
old corruption-and-charges night.

## Why

Corruption and charges were quiet, slow and hard to feel. A thing in the dark
that wants to hurt you, and that you can fight back with a sword, is easy to
understand and builds on today's work: health, blows, bodies that stay,
fear of whoever hurts you.

## What goes

1. **Corruption, all of it.** `P_CORRUPT`, `add_corruption`, `twist_line`
   and its prompt line, spread through talk and gifts, fade by day and light,
   the dark-creep look in `render.rs` / `scene.wgsl`, "N corrupted near" in
   the status line, the "talked someone back" achievement, `touch.corruption`.
2. **Charges and the `/` cost.** `spend_charge`, `refund_charge`,
   `gain_charges`, `sync_level`, night and kindness bonuses, `✦` in the
   status line and notes, `touch.charges`, `Level.charges/drain`. Creating is
   free on every difficulty again.
3. **LLM-written horrors.** `horror_brief`, `write_horror`, `write_ahead`,
   `on_species_made`, the `WRITE_WAIT` wait, "night walker 2, 3…" names, and
   `Request::NewSpecies` / `Cmd::NewSpecies` if night is its only user. The
   built-in night walker (`builtin/nightwalker.js` + `fallback_horror`) is the
   one and only kind.
4. **`docs/night-plan.md`** (deleted), and the memory note about it updated.
5. Tests that cover the above (most of the ~46 hits in `sim/tests.rs`).

What stays as is: species hours (`active`), `want`, `shuns`, `moves_unseen`,
arrival out of sight, signs and dread lines, comings and goings at dusk and
dawn, the difficulty setting itself.

## New: the traveler has health

Today only beings have `P_HEALTH`; the traveler has none.

- `player.health` 0..1, saved. Mends slowly (same `MEND_PER_DAY`).
- A coloured health bar in the bottom UI area (green → amber → red as it
  drops), with a short red flash on screen when hit.
- Blows on the traveler go through the same `strike_being` sum (tool weight,
  speed, body size), minus armour (below). Night walker touches take a fixed
  amount.
- **At 0 nothing happens** for now. Death is left for later.

## New: armour counts

Worn layers are only looks now. Add one number: a thing's `protect` (0..1),
set when a layer is made (the builder already knows "armour", "helmet",
"padded coat"). A blow is cut by the sum of worn `protect` (capped ~0.7).
Works for the traveler, villagers, phantoms alike.

## Night walkers (kept, made simpler and harmful)

| difficulty | night walkers | phantoms |
|------------|---------------|----------|
| peaceful   | 0             | 0        |
| easy       | 1             | 1        |
| normal     | 1             | 1        |
| hard       | 2             | 2        |

Every night, all in one table (`LEVELS`). No "some nights", no growth over
time: the growth comes from the phantoms learning.

- Same behaviour: away by day, comes at dusk out of sight, moves only while
  unwatched, stops at the edge of light, signs and sounds when near.
- **Watch cone halved**: `player_watches` uses 0.4 of the view half-width
  instead of 0.8, so it creeps closer while you look a bit to the side.
- **Touch now hurts** instead of draining/corrupting: `touch.hurt` (e.g.
  0.25 of health per touch), then it draws back (`TOUCH_GAP`) as today.
- **Others in the way**: keeps today's `want: traveler`. Its touch now hurts
  whoever it lands on, so villagers and animals in its way can be hurt or
  killed (worlds with `hunting` on). Bodies, fear and witnesses already work.
- **Can be fought**: it has health; a sword blow hurts it. Killed, it falls
  and its body fades at dawn; it does not come back until the next night
  (count stays per night).
- **Sound**: its four fixed sound lines (`fallback_horror.sounds`) already
  map to procedural calls through `audio::call::guess`. No recorded sounds
  exist in the game, so nothing to pre-make; the phantoms get fixed lines the
  same way.

## Phantoms (new)

Characters, not monsters: the same body, limbs, planner, memory and
actions as villagers. They act on their own wants, with one overlay on top:
they want to cause harm. They prefer the traveler, but may hurt anyone,
people or creatures. (Later maybe: attacking buildings, cutting land.)

**Made once per world**, at the first night, as N characters (N by
difficulty, more made if difficulty goes up later). Named in the order they
first came: "the first phantom", "the second phantom", … (told apart, since
each remembers its own nights). A fixed persona (patient, cruel, cunning)
and the species' `drive` line tell its mind what it is. Body: the built-in
`phantom.js`, written from the human figure (so what fits a person fits
it): too tall, too thin, long arms, the head tipped to one side, waxy skin,
sunk eyes with a faint light. Not LLM-written.

**Hours**: `active: night`, fade at dawn, come back at dusk out of sight,
like the night walker. While away they are not simulated, but keep
everything: memories, relations, fear, held tool, worn layers.

**Goal**: a fixed drive in their prompt and scorer, "you want the traveler
hurt and dead", in place of needs like food and sleep. Their plans use the
normal actions:

- `Hold` / `Wear` anything lying around that helps (a knife, a stick,
  a pot as a helmet), `Use`/strike with it.
- `Create`: 1 creation per phantom per night (a spear, a trap, armour),
  plus anything they find or steal.
- Find and steal: take from the ground, the dead and the sleeping.
- Talk: they may speak to the traveler (taunts, lies), short lines only.

**Memory is the difficulty curve.** Lore: they can't really die, they only
come back. They keep everything, like every character already does: all
memories stay in the save (`memories` table), relations, fear, held tool,
worn layers. Nothing is thrown away at dawn.

Characters already pick the most relevant and recent memories into each
prompt (top 6 by match/importance + last 6) plus a rolling summary, so a long
history costs nothing extra. Two small additions:

- **Hunt memories rank high.** Being hurt, killing, what the traveler fought
  with, where they sleep, what gear helped: written with high importance so
  they win the pick.
- **Dawn lesson.** At dawn each phantom writes one extra high-importance
  note: "what worked tonight, what to do next time". Its own summary of the
  night, on top of all the raw memories (not instead of them).
- **Same recall as every character.** No per-difficulty change.

So over nights they come better armed, better armoured, from the side you
don't watch, or wait where you sleep. No extra "learning" code: it's
memory + planner.

**Fighting them**: normal health and blows. Killed, the body stays till
dawn and drops what it held and wore (loot!). It comes back next night at
full health, remembering everything, including being killed and by what,
without the gear it dropped.

**Scary, cheap**: they don't show on the screen edge dread lines by name;
they walk, not glide; they stand still and stare when seen at a distance;
villagers fear them on sight (existing fear, seeded at first meeting).

## Status line

`night in 3m` by day; at night `2 walkers · 1 phantom near`, parts hidden
when not true. Health is the bar, not text. Achievements: first night survived, first
phantom killed, a night without a wound.

## Built as

- Reflexes (`Sim::hunt`, every second or so): unarmed, take up the best
  striking tool within reach; with room to spare, put on what protects;
  at hand, strike with what it holds or claw (`touch.hurt` 0.07); else go
  after its prey. Far off with a plan of its own, its plan runs. The
  planner is asked at dusk ("plan tonight's hunt"), when struck, and now and
  then while hunting from afar.
- The dark's own fear nothing and never run; villagers fear them on sight.
- Status line counts what is near; health is the bar at the bottom right,
  and the view's edges flash red when hurt.
- Achievements: Back Into the Dark (strike down a walker), Not for Long
  (strike down a phantom).
- Traveler falling (added the same day): at 0 health the traveler falls,
  drops what they held, and lies there (the view drops and rolls, YOU DIED
  over a dark red view of the world going on). Witnesses remember and their
  planners are asked; the killer remembers; the dark looks elsewhere. Enter
  wakes them at the world's spawn, next morning, whole. A night you fell in
  isn't "got through".
- Phantoms' planner is asked at the planner's pace whenever they are not
  at hand (like anyone's), not only at dusk and when struck.
- How it happened: every hurt carries a short phrase of what it was, as it
  would end "they were …" ("struck by the first phantom with the rusty
  sword", "broken by a fall of 7 m from the wooden hut", or what the
  interpreter's `how` says for a deed). It is on the death screen, in the
  "died" news and in every witness's memory.
- Not done: taking armour off the dead, attacking buildings.

## Order (as planned)

1. Remove corruption, charges, LLM horrors, old plan (one commit, tests green).
2. Traveler health and the health bar.
3. `protect` on worn layers.
4. Night walker: counts table, halved watch cone, hurt touch, can be killed,
   may strike others in its path.
5. Phantoms: creation, night hours, hunt drive, gear use, create cap.
6. Hunt memory importance, dawn lesson (recall the same as every character).
7. Achievements, tuning pass on easy/normal/hard.

## Decided (2026-10-05)

- Traveler at 0 health: nothing happens yet; death later.
- Killed phantoms come back next night and remember everything.
- Phantoms ignore light.
- One built-in phantom figure (not LLM-written).
- Night walker targeting stays as is; phantoms prefer the traveler but harm
  anyone.
- 1 creation per phantom per night, plus find and steal.
