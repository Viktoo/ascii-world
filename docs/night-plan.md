# Night, corruption and the power of creation

Agreed 2026-10-02, built the same day (`src/sim/night.rs`). One conflict instead of quests: something comes at night
for the traveler, because the traveler can make things out of nothing.
Peaceful worlds (expected to be most) get the same primitives with the
danger turned off. Nothing here is written for one monster: it is species
fields, a prop and a few numbers per difficulty.

## Primitives

1. **Hours** (`species.active`: `always` | `day` | `night`). Outside its
   hours a being is *away*: not drawn, not simulated, not met. It slips away
   and comes back only out of the traveler's sight. Peaceful worlds get
   owls, moths and fireflies from the same field.
2. **Want** (`species.want`: a species name, `traveler` or `anyone`). What
   it goes after, reusing the chase. A being that wants something returns
   from being away near it, just out of sight.
3. **Touch** (`species.touch`: `props`, `needs`, `charges`, `corruption`).
   What contact does, once per short cooldown. A firefly's touch can make you
   glow; a horror's drains charges and corrupts.
4. **Corruption** (a prop, 0..1, on characters and things). Shows as the dark
   creeping over it (blackened, a dull red under it). Over 0.3, one short line joins that character's prompts (talk,
   plans, the traveler's creations): their nature, words and work twist.
   Fades slowly by day, faster in light; spreads (on some difficulties)
   through talk and gifts; talking someone back with kindness lowers it.
5. **Charges** on the power of creation. Each `/` creation costs one. Dawn
   tops up to the starting number; more earned above it is kept. Earned by
   getting through a night and by being thanked.

## Difficulty (per world, kv `difficulty`)

|                                  | Peaceful | Easy           | Normal           | Hard                       |
|----------------------------------|----------|----------------|------------------|----------------------------|
| charges (start, top-up at dawn)  | ∞        | ∞              | 24               | 12                         |
| night horrors                    | none     | some nights, 1 | every night      | every night, more over time|
| horror touch                     | –        | corruption     | −1, corruption   | −4, corruption             |
| corruption spreads               | no       | no             | slowly           | yes                        |
| twisting prompt                  | off      | on             | on               | on                         |

All numbers live in one table (`sim::difficulty`).

## Night horrors

Made once per world by the LLM through the normal species path (a strange
body, `active: night`, `want: traveler`), then kept as characters: they
are away by day and come back at dusk. Hard adds one more every few nights
up to a cap. Behaviour, all cheap:

- moves only while the traveler isn't looking at it;
- stops at the edge of light (`light` over 0.5 nearby) and waits;
- lights dim and animals flee as it nears; people fall quiet;
- ambient lines in the log (“something is breathing beyond the trees”);
- what it touched darkens.

At night people gather near light to sleep.

## Status line

`night in 3m · ✦ 18 · 2 corrupted near`, parts hidden when they
don't apply (Peaceful shows nothing new but the hour).

## Achievements

First night survived · Talked someone back · A night without a touch.

## Order

1. Hours and want and touch (all modes). 2. Difficulty. 3. Charges.
4. Corruption. 5. Horrors. 6. Status line and achievements.
