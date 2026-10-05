# Weather, clouds and sunsets

Agreed 2026-10-04, built the same day (`src/world/weather.rs`, `src/sim/weather.rs`).
One weather state per world: the sky, the light, the rules, the people and
the sound all read the same moment. The LLM describes each world's climate
once at genesis; the engine runs it. Nothing here knows what rain is: rain,
snow, ash, petals and moon-rain are the same record. No birds: anything
smaller than a cat is below what the screen can show.

## Primitives

1. **Climate** (`look.climate.kinds`, written at genesis). Each weather kind
   is a few numbers: `often`, `hours` [shortest, longest], `clouds`,
   `cloud_color`, `wind`, `fog`, `tint`, `storm` (lightning), `feel` (words),
   and `falls`: what comes down (`what`, `amount`, `color`, `look`: streak,
   flake or mote) and what it does to what it lands on in the open
   (`props`: rain `{ wet: 1 }`, snow `{ wet: 0.4, temp: -2 }`, or the world's
   own properties). Worlds without one get fair, cloudy and grey days with
   nothing falling.
2. **Timeline** (`Weather::at`). Spells walked forward from the world seed
   and the time: deterministic, nothing saved, headless catch-up works.
   Kinds blend into each other over up to two game hours; showers come and
   go within a spell; the ground soaks while it falls and dries after
   (`soak`); clouds drift with the wind. Lightning strikes are hashed from
   the time too (`strikes`, `flash`).
3. **Made weather** (`Spell`, kv `sim.weather`). A deed's answer may carry
   `"weather"` (the interpreter's format): the traveler's `/make it rain`
   (one charge, like any creation), or a character through this world's own
   magic. It holds for its hours, then the climate returns.
4. **Feelings by word** (`species.weather`): `{ "rain": 0.9, "storm": -0.9 }`,
   keyed by a kind's name, what falls, or a plain word (rain, snow, storm,
   wind, fog, clear). Weather it doesn't name: it minds what falls, storms
   and gales. Built-in species have their own (cats hate rain, dogs like snow).
5. **Weather hours** (`species.comes_with`): a being only about in some
   weather (rain snails, a fog wraith), through the same away/back as
   `active` day/night.
6. **Rule inputs** `falling` and `wind` (0 under a roof): universe rules can
   read the weather ("rusts in the rain").

## What it does

- **Sky**: a cloud layer (cover, colour, drift), lit by the time of day;
  cloud shadows sliding over the land; overcast greys the sky and dims the
  sun; haze thickens the fog; lightning flashes sky and land.
- **Sunsets**: a blaze low toward the sun, a rose band higher up, violet
  away from it, clouds lit from beneath (gold toward the sun, pink away),
  the glow lingering after the sun has gone, a wider sun, glitter on water.
  Colours come from the palette's dusk horizon, made richer.
- **What falls** is drawn in three depth layers, placed by direction so it
  stays put as the view turns, hidden behind anything nearer, and not right
  before the eyes under a roof. Wet things darken and shine; water roughens
  with wind and rain.
- **Rules**: things out in the open are drawn toward what falls (rain wets,
  wet doesn't catch fire, and it puts fires out); the land is as soaked as
  `soak` says.
- **People and animals**: the planner hears the weather, where they are in
  it and how they feel ("you dislike this weather"). The scorer offers
  getting out of it (home, the nearest way in, under a tree), staying in
  until it passes, or going out into weather they love. The weather turning
  is news: one line in the log, and those near who mind it think again.
- **Sound**: the wind follows the weather (little indoors); rain patters
  softly, a dull drumming under a roof (snow, ash and motes are silent);
  thunder is a crack when near, then a long roll, late by distance;
  singers keep quiet in heavy rain.
- **Status line** shows the weather's name.

## Trying it

`pocket snapshot FILE --at x,z,yaw --time 18 --weather storm --png out.png`
(the world's own kinds by name, or clear, fair, cloudy, overcast, fog, rain,
storm, snow, ash, motes).

## Not done

- The shore: visible waves running up the beach and a sound shaped like
  them (one clock for both), instead of the steady shore texture.
- Horrors bolder on stormy or foggy nights.
- Wind moving grass and trees.
