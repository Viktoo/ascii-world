# Sound

Agreed and built 2026-10-03 (`src/audio`). Procedural, directional sound
as a primitive: every sound is made live from a small recipe, placed where
it happens and heard from where the traveler stands and faces. No audio
files anywhere. Nothing here is written for one creature: it is a small
synth kit, a spatializer, a few sources the engine derives from physics,
and one data field the LLM fills for each species.

## Principles

1. **Recipes, not recordings.** A sound is a short call: a few parts with
   numbers. Stored as text (species JSON), rendered at play time.
2. **Never the same twice.** Every play has a seed; each repeat within a
   call varies a little. An individual keeps its own pitch (from its id and
   its size against its kind's).
3. **Physics sets the register, the recipe sets the shape.** Pitch left
   unsaid comes from the body's mass; formants always scale with it; an
   individual's size bends its pitch. One call fits a kitten and a giant.
4. **The LLM describes, the engine keeps it good.** The LLM answers easy
   questions (what kind of sound, what pitch, how rough, how long; for
   things, how hard and dry). Hand-tuned mappings turn those into DSP, and
   every number is clamped, so the worst a bad answer can do is sound like
   the wrong animal.
5. **Sound is output only.** The sim pushes cues (plain data) and never
   hears back. A test proves the same seed makes the same history whether
   or not anything listens.
6. **Text stays.** Every sound still exists as its `sounds` string and log
   line, so muted play, SSH and tests lose nothing.

## The kit (`synth`)

Four kinds of part, plus endless textures:

| kind      | what it is                                                       | covers (e.g.)                                  |
|-----------|------------------------------------------------------------------|------------------------------------------------|
| `voice`   | band-limited glottal source (jitter, period doubling for rough), noise for breath, 4 vowel formants scaled by body, optional tube resonance (nasal) | calls, growls, groans, breath, trunks, whines |
| `whistle` | sine with pitch glide, vibrato, buzz (rough) and breath          | birdsong, hoots, insects, chimes               |
| `noise`   | filtered-noise grains, from the material mapping                 | hiss, rustle, splash, crunch                   |
| `knock`   | five struck decaying modes (wood-like to metal-like by `ring`), click and thud | knocks, snaps, clatter, footfalls, impacts |
| texture   | endless grains whose drive the game sets each frame              | brushing through growth, lapping water         |

A part: `kind`, `hz` (one or a [start, middle, end] glide), `len`, `times`,
`gap`, `vowel` (up to three of a e i o u m n, glided), and 0–1 sliders
`rough`, `breath`, `nasal`, `wobble`, `swell`, `loud`, plus `hard`, `dry`,
`ring` for noises and knocks. A call is 1–8 parts in order. `loud` spans
about 30 dB, so a purr is truly quiet.

**Material, not identity.** Engine sounds don't know "grass" or "leaves";
they read `hard`, `dry`, `ring`, `leafy` from the thing's properties
(alive, burns, wet, fragile, char) and tags, or a type's own `meta.sound`.
Green blades swish softly; dead litter, the same code, crunches. Only
brittle things (fragile) ring by nature; a bell or a pan says `ring` in its
`meta.sound` (conducting heat or magic is no reason to ring).

**Everyday physics decides whether there is a sound at all.** A sound
needs energy going into something that can make one:
- *Two bodies meet:* the softer one damps the strike, the lighter one is
  mostly what is heard, heavy things barely ring, and loudness follows the
  energy (the lighter mass, the speed met squarely). A body walking into
  stone or a trunk is a dull, quiet thud; a pot dropped clinks; a struck
  bell rings.
- *Growth gives way:* walking through or past anything leafy and light
  enough to be pushed (under ten times the walker's mass) is a rustle;
  only dry, dead growth met head-on snaps. Trunks don't rustle.
- *Glancing contact* (sliding along a wall) makes nothing.
- *Fire* is heard wherever things burn: sparse pops and a flickering low
  roar, louder the more burns.

## Directional hearing (`mix`)

The listener is the traveler's head. Every voice is placed each 128-sample
block, values ramped across the block, so turning sweeps a sound smoothly:

- **Distance:** full loudness within 2 m, then 1/d.
- **Left/right:** equal-power pan, interaural delay (≤0.65 ms), head shadow
  (the far ear duller).
- **Front/back:** behind is about 3 dB softer and much duller above
  ~1.5 kHz, so turning to face a sound makes it clearer, not just centred.
- **Air:** highs fade with distance (the room's share too).
- **Room:** one shared, dark outdoor reverb; its send falls far slower than
  the direct sound, so far things sound distant and near things dry.

Acceptance (a test, `behind_then_facing_then_walking_up_gets_louder_and_clearer`):
a call 30 m behind and to the left is in the left ear; turned round, it is
louder, brighter and on the right; walking up, ever louder.

## Where sounds come from

**Engine (no LLM; `foley`, each frame):**
- *Underfoot is the quietest of all:* footsteps and brushing happen all
  the time (a village stands in grass), so they sit just above silence, about
  24 dB under a call, and only within 20 m.
- *Footsteps* for every grounded walker within 20 m: a thud from the body's
  mass and a crunch, squelch or splash from the ground (biome name, wading
  depth). At most one sound per 0.14 s per walker.
- *Brushing* through non-solid leafy scatter: one texture per moving body,
  driven by how much growth is about its legs × speed × body size.
- *Bumps:* `Sim::walk` notices a blocked step, how squarely it was met
  and how fast, and cues the solid with the body that met it (once, not
  again while leaning on it). Brushing past growth feeds the rustle instead.
- *Impacts:* loose things hitting anything over 1.2 m/s.
- *Fire:* burning things and ground within 70 m, gathered into up to four
  8 m spots, each one crackling voice.
- *Beds:* wind (height, openness, gusts), the nearest open water (placed
  where it is), and a dread drone while a harmful being is near in the dark.
- *Chorus:* singers kept near the listener, each a spot and a song: insects
  at night where things grow (pitched by the world's seed), by day the
  world's own small voiced creatures, else birds where trees stand. They
  fall quiet around anything that harms, and stay quiet a while after.

**Calls (the sim's cues):** a being's noise (`beings::noise`, so make_noise
and fleeing cries), a night horror's heard sound and any sign that is a
sound (from where it is), a thing's `sound(i)`. Each plays in the maker's
voice and follows it as it moves.

**Where calls come from:**
- Species get `voice`: one call per `sounds` line, written by the LLM in the
  same JSON (genesis, region, new species, night horror). Built-ins have
  hand-tuned ones.
- Missing or unreadable: guessed from the words ("a low growl", "two sharp
  chirps", "a wet click"), bent by low/high/soft/loud/long/short/two.
- Things: their `sounds` words, at their mass, in their material.

This is data, not a script entry point as first proposed: a few numbers
per sound are easier for a model to get right and impossible to get badly
wrong; the kit stays where quality lives.

## Cost

A crowded test scene (20 calls, 10 brushing textures, 24 singers, wind,
water and dread) renders 4 s in about 0.26 s in release: ~6% of one core.
At most 48 voices are rendered; quieter ones run silently. Inaudible
textures only move their drive. One reverb for everything.

## Checks (tests)

- Placement: left/right, behind/front brightness and loudness, approach;
  far sounds carry more room.
- Every kind at every size: finite, not silent; seeds repeat exactly and
  differ.
- Calls written badly stay in range; species with bad voices still load;
  guesses follow words and body; materials follow properties; `meta.sound`
  is read and checked.
- Hushed singers fall quiet; the crowded scene stays under budget.
- In a world: a dog's noise comes from the dog in its own voice; a living
  shrub walked into isn't struck, and walked past rustles softly; a boulder
  walked into is one dull thud with no ring; fire crackles where it burns;
  a thrown stone is heard where it lands; listening changes nothing.

## Listening

`pocket listen --demo [--out DIR]` renders the acceptance scene, the
materials, every built-in voice, a night, contacts (body on stone, on wood,
into a dry bush; a stone landing, a pot, a bell) and a fire as WAVs.
`pocket listen FILE --species NAME` plays a species' calls.
`pocket listen FILE [--at x,z,yawDeg] [--hour H] [--walk] [--calls] [--verbose]`
stands or walks in a world and records it; `--verbose` prints every voice.

## Settings

`Sound` on/off and `Volume` in the settings screen; `POCKET_NO_SOUND=1`
turns it off for a run. Off by itself when there is no output device (over
SSH sound would play on the host).

## Later

Occlusion (a ridge or wall between: muffled), HRTF for headphones,
murmured speech (a sentence-shaped `voice` in a species' formants), beings
that hear (noise draws or scares them, sneaking is quieter), a local
audio-text model to score written calls, world-made ambient life for the
chorus, seasons and rain shifting materials.
