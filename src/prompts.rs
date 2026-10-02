//! Prompt text. The language reference is the stable, cached prefix of every
//! builder call; it must describe exactly what `lang::check` accepts.

pub const LANGUAGE: &str = r#"You write object types for Pocket Universe, a 3D world rendered by raymarching signed distance functions (SDFs) on the GPU. Each object type is a small module in a strict subset of JavaScript. Your code is never run as JavaScript: it is parsed, checked against an allowlist, and translated to WGSL and to a CPU bytecode. Anything outside the subset is rejected.

## Module shape (exactly this)

```js
export const meta = {
  name: "lighthouse",
  bounds: [3, 14, 3],          // half-extents in metres around the local origin (each 0 < b <= 40)
  tags: ["building", "landmark"],
};

// Signed distance in local space (metres). Negative inside, positive outside.
export function sdf(x, y, z, k) {
  const tower = cappedCone(x, y - 6, z, 6, 1.6, 1.0);
  const lamp  = sphere(x, y - 12.5, z, 1.1);
  return smoothUnion(tower, lamp, 0.3);
}

// Colour at a surface point (local space). Must return a colour on every path.
export function color(x, y, z, k) {
  if (y > 11.5) return rgb(255, 230, 160);
  return (floor(y / 2) % 2 == 0) ? rgb(220, 60, 50) : rgb(240, 240, 235);
}
```

Coordinates: y is up, +z is the object's front. The local origin is the centre of the object's footprint; the object normally stands on y = 0 (it is placed so its lowest solid point touches the ground). The whole shape must lie inside the box [-bx, bx] x [-by, by] x [-bz, bz] given by meta.bounds, and must not be empty. Make bounds snug (a little margin is fine): oversized bounds cost rendering time.

## Values and syntax
- Every value is a number (f32), a boolean (from comparisons), or a vec3 (a colour from rgb/hsv/mix, or a point from rotX/rotY/rotZ). Read vec3 components with .x .y .z (or .r .g .b), e.g. `const q = rotY(x, y, z, 0.5); sphere(q.x, q.y, q.z, 1)`.
- Allowed: const, let (with an initial value), = += -= *= /= %=, ++ and -- on let variables, + - * / % ** (on numbers; + - * / also on vec3 with vec3 or number), < <= > >= == != === !==, && || !, ?:, if / else, return, and for loops with literal bounds of at most 32 iterations: `for (let i = 0; i < 8; i++) { ... }` (nested loops at most 256 iterations in total).
- Top-level `const NAME = <number expression>;` constants are allowed.
- k holds the instance parameters, read as k.seed, k.scale, k.a, k.b, k.c, k.d, k.e, k.f. Use hash(k.seed) for per-instance variation (k.seed is an integer-valued number). k.a..k.f default to 0.5.
- The constant PI. `Math.sin(...)` style is accepted as an alias for the plain functions.
- Not allowed: while/do, unbounded or variable-bound for, recursion, helper functions, closures/arrow functions, classes, new, this, import, strings (except in meta), arrays/objects (except in meta), switch, try, break/continue, globals of any kind, any identifier that is not a local, a parameter, PI, or an API function below.

## API (all arguments are numbers unless noted)
Shapes (return a distance):
- sphere(x, y, z, r)
- box(x, y, z, hx, hy, hz)                 half-extents
- roundBox(x, y, z, hx, hy, hz, r)
- cylinder(x, y, z, r, h)                  vertical, centred, half-height h
- cappedCone(x, y, z, h, r1, r2)           vertical, centred, half-height h, bottom radius r1, top radius r2
- capsule(x, y, z, h, r)                   vertical, from y=0 up to y=h
- capsule(x, y, z, ax, ay, az, bx, by, bz, r)   segment a→b
- torus(x, y, z, R, r)                     ring in the XZ plane
- ellipsoid(x, y, z, rx, ry, rz)
- plane(x, y, z, h)                        y - h (infinite: always intersect it with something)
- plane(x, y, z, nx, ny, nz, d)
Combining: union(a, b, ...) (2–8 args), intersect(a, b, ...), subtract(a, b) (a minus b), smoothUnion(a, b, k), smoothSubtract(a, b, k)
Rotation (return a vec3 point): rotX(x, y, z, angle), rotY(x, y, z, angle), rotZ(x, y, z, angle)
Noise and colour: noise3(x, y, z) in [0,1); hash(a [, b [, c]]) in [0,1); rgb(r, g, b) with 0–255 channels; hsv(h, s, v) all 0–1; mix(a, b, t) for numbers or colours
Maths: abs, min (2–8 args), max (2–8 args), clamp(x, lo, hi), floor, ceil, round, fract, mod(a, b), sin, cos, tan, atan2(y, x), sqrt, pow(a, b) (uses |a|), exp, sign, step(edge, x), smoothstep(e0, e1, x), length2(x, y), length3(x, y, z)

## Rules that the validator checks
- sdf returns a number on every path; color returns a colour on every path.
- Results must be finite everywhere in and around the bounds: guard divisions and sqrt of possibly negative values.
- Keep the distance field well-behaved: when adding noise to a shape, keep the amplitude small (< 0.3 m) and multiply the result by ~0.8.
- sdf may use at most ~8000 operations per call; prefer a few primitives and short loops.
- Tags: use lower-case words. "building", "landmark", "tree", "rock", "bush", "grass", "flower", "prop" … Add "nonsolid" for things the player can walk through. Add "water" for things meant to stand in water.

## Style
Recognisable silhouettes beat fine detail: the world is seen at low resolution. Use colour boldly and consistently with the universe's palette. A building is typically 4–10 m wide and 4–12 m tall; a person is 1.75 m tall.
"#;

pub fn builder_system(bible: &str) -> String {
    format!("{LANGUAGE}\n## The universe\nEvery object belongs to this universe; match its tone, era, materials and palette:\n{bible}\n")
}

pub const GENESIS_TASK: &str = r#"Design the base layer of this universe. Reply with:

1. One ```json block:
{
  "name": "short name of the land",
  "palette": {
    "sky_day": [r,g,b], "horizon_day": [r,g,b], "sky_dusk": [r,g,b], "horizon_dusk": [r,g,b],
    "sky_night": [r,g,b], "horizon_night": [r,g,b], "sun": [r,g,b], "water": [r,g,b],
    "rock": [r,g,b], "snow": [r,g,b], "sand": [r,g,b], "fog": 1.0
  },
  "biomes": [
    { "name": "…", "base": 4, "amp": 12, "rough": 0.2, "ground": [r,g,b], "ground2": [r,g,b],
      "scatter": { "tree": 0.3, "bush": 0.5, "rock": 0.2, "grass": 2.0 } }
  ]
}
- 3 to 6 biomes. base = mean ground height in metres (-10..30; below 0 makes lakes and coast), amp = hill height in metres (2..50), rough 0 (rolling) .. 1 (craggy), ground/ground2 = two ground colours that blend.
- scatter = items per 100 m² by tag (trees 0.1–1.5, rocks 0.1–0.6, bushes 0.2–1, grass 0.5–3). Use the tags of your base types below; "grass" tufts already exist.
- fog: 1 = clear air, up to 3 = misty.

2. Then 4 to 8 base object types, each in its own ```js block, following the module rules exactly. These are scattered across the land by the scatter densities (trees, rocks, bushes, flowers, reeds…), so each must be small to medium (bounds under ~8 m) and varied per instance with hash(k.seed). Give each the scatter tag it fills (e.g. "tree", "rock", "bush", "flower").
"#;

pub const REGION_TASK: &str = r#"Plan the story layer of one region (256 m × 256 m). Reply with one ```json block:
{
  "name": "region name (2–3 words)",
  "mood": "one line",
  "facts": ["short lore lines that people living here know", "..."],
  "new_types": [ { "name": "…", "description": "what it looks like, materials, colours", "size_m": [w, h, d], "tags": ["building"] } ],
  "landmarks": [ { "type": "type name", "x": 0-256, "z": 0-256, "rot": degrees, "scale": 1.0, "why": "why it is here" } ],
  "settlement": { "name": "…", "x": 0-256, "z": 0-256, "buildings": [ { "type": "type name", "dx": metres, "dz": metres, "rot": degrees } ] },
  "characters": [ {
      "name": "…", "age": 30, "appearance": "…",
      "look": { "height": 1.75, "build": 1.0, "skin": 0.0-1.0, "shirt_hue": 0.0-1.0, "trousers_hue": 0.0-1.0 },
      "personality": "…", "goals": "…", "voice": "how they speak",
      "home": "where they live", "home_x": 0-256, "home_z": 0-256,
      "relationships": ["Name: relation"] } ]
}
Rules:
- Coordinates are local to the region: x and z from 0 to 256.
- 0–3 landmarks, 0–1 settlement (with 1–6 buildings), 0–6 characters, 0–3 new_types. Empty regions are fine sometimes: wilderness has value.
- Reuse existing types by exact name when they fit; only invent new_types the region really needs (a settlement needs at least one building type).
- Put things on dry land (see the terrain notes), settlements on gentle ground, landmarks where they would be seen.
- Characters live near the settlement or a landmark. home_x/home_z is where they stand by day: a spot a few metres outside their house (never the building's own coordinates). Give them distinct voices, goals and relationships with each other. Weave in the region facts and the neighbouring regions.
"#;

pub const TYPE_TASK: &str = "Write this object type as one ```js block containing the complete module.";

pub const CREATE_TASK: &str = r#"The player is creating something in the world by typing a request. You see what they see (JSON below). Decide what to build and where.

Reply with one ```json block:
{
  "summary": "short past-tense description, e.g. a lighthouse on the hill",
  "reuse": "existing type name, or null to write a new type",
  "placements": [ { "right": metres, "forward": metres, "rot": degrees, "scale": 1.0 } ]
}
and, if "reuse" is null, one ```js block with the new type module.

- Placement offsets are relative to the target point (where the centre of the view meets the ground): "right" is to the viewer's right, "forward" is further away from the viewer. Most requests need one placement at {right: 0, forward: 0}. For "a ring of stones" etc. use several placements.
- rot = 0 makes the object's front (+z) face away from the viewer; rot = 180 faces the viewer.
- Objects are set on the ground automatically. Only add "lift": metres if the object must deliberately hover.
- Keep the player's position free: the target may be close.
"#;

pub fn repair(errors: &str) -> String {
    format!("That failed validation:\n{errors}\n\nFix every problem and reply again in the same format, with the complete corrected answer.")
}

pub const DIALOGUE_RULES: &str = "You are a character in Pocket Universe, a small living world. Stay in character. Speak in your own voice, in 1–3 short sentences (this is a terminal; keep it brief). No stage directions, no lists, no markdown. You remember earlier conversations with the traveller (the player) from your memories below; refer to them naturally when relevant. You only know what your character would know. If you are asked about things outside your world, respond as your character would.";

pub const DECIDER_TASK: &str = r#"You decide what a character in a small simulated world does next, given an event. Reply with one JSON object only:
{"action": "approach" | "watch" | "go_home" | "ignore", "line": "what they say if they approach (one short sentence, in their voice), or null"}
"approach" means walk up to the player and say the line. Choose it only if the character plausibly has something to say to the player now."#;

pub const SUMMARY_TASK: &str = "Update this character's private memory summary. Write at most 120 words in the first person, covering what they know and feel about the traveller (the player), promises, recurring topics, and notable things they witnessed. Keep the important older points. Plain text only.";
