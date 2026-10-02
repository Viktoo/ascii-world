//! Prompt text. The language reference is the stable, cached prefix of every
//! builder call; it must describe exactly what `lang::check` accepts.

pub const LANGUAGE: &str = r#"You write object types for Pocket Universe, a 3D world rendered by raymarching signed distance functions (SDFs) on the GPU. Each object type is a small module in a strict subset of JavaScript. Your code is never run as JavaScript: it is parsed, checked against an allowlist, and translated to WGSL and to a CPU bytecode. Anything outside the subset is rejected.

## Module shape (exactly this)

```js
export const meta = {
  name: "lighthouse",
  bounds: [3, 14, 3],          // half-extents in metres around the local origin (each 0 < b <= 40)
  tags: ["building", "landmark"],
  props: { burns: 0.1, light: 1 },   // optional: what it is made of and does (see Properties)
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

## Properties (meta.props)
Every thing has a few numbers the world's rules act on. Give the ones that matter for this object; the rest default sensibly from tags and size:
__PROPS__
Examples: a wooden hut { burns: 0.4 }; a lantern { light: 1, heat: 120, fragile: 0.6, burns: 0.6, fuel: 0.5 } (oil inside: if it breaks, it burns); a ball { bounce: 0.8, mass: 0.6 }; an apple { edible: 0.4, mass: 0.2 }; a bucket of water { wet: 1, mass: 8 }; a sapling { alive: 1, growth: 0.1, burns: 0.5 }. Only use the property names listed. The world does the rest: fire spreads to what burns, water puts it out, fragile things break when hit hard, living things grow.

## Behaviour (optional): tick, use, touch
An object can act on its own with these optional exports. They run on the CPU a few times a second near people; they never draw anything themselves, but the shape and colour functions can read their state as k.s0 … k.s7.
```js
export const meta = { name: "brass clock", bounds: [0.3, 0.4, 0.2], tags: ["item"], says: ["Tick… tock."], sounds: ["ding"], spawns: [] };
export function tick(s, w, k) {        // s: state s.s0 … s.s7 (numbers, start at 0, saved)
  s.s0 = s.s0 + w.dt;                  // w.dt = seconds since the last tick
  if (s.s0 > 60) { s.s0 = 0; sound(0); }
  w.light = w.hour > 19 || w.hour < 6 ? 0.5 : 0;   // set its own properties
}
export function use(s, w, k, o) {      // someone uses it (o = the thing it is used on, when w.on is 1)
  s.s1 = 1 - s.s1;                     // e.g. open/closed, read in sdf as k.s1
  say(0);
}
export function touch(s, w, k) { if (w.impact > 6) { w.health = w.health - 0.5; } }   // something hit it
```
- w.<field> describes the situation (read only): dt, hour (0–24), age (seconds since it was made), held (1 if someone holds it), near (people within 4 m), speed, ground (1 if resting on the ground), water (1 if in water), impact (touch: hit speed), on (use: 1 if used on something). Every other w.<name> is one of its own properties (read and write); o.<name> is the other thing's property (read and write).
- Effects (statements): say(i) speaks meta.says[i]; sound(i) makes meta.sounds[i]; spawn(i) makes a new meta.spawns[i] (a type name) next to it; transform(i) turns it into meta.spawns[i] (a seed into a sapling, an egg into a chick); remove() makes it vanish. Effects are rate-limited.
- Same language rules as sdf/color (no strings outside meta, loops with literal bounds); `return;` ends early; no return value. Keep them small.

## Rules that the validator checks
- sdf returns a number on every path; color returns a colour on every path.
- Results must be finite everywhere in and around the bounds: guard divisions and sqrt of possibly negative values.
- Keep the distance field well-behaved: when adding noise to a shape, keep the amplitude small (< 0.3 m) and multiply the result by ~0.8.
- sdf may use at most ~8000 operations per call; prefer a few primitives and short loops.
- Tags: use lower-case words. "building", "landmark", "tree", "rock", "bush", "grass", "flower", "prop" … Add "nonsolid" for things the player can walk through. Add "water" for things meant to stand in water.

## Style
Recognisable silhouettes beat fine detail: the world is seen at low resolution. Use colour boldly and consistently with the universe's palette. A building is typically 4–10 m wide and 4–12 m tall; a person is 1.75 m tall.
"#;

pub fn builder_system(bible: &str, props: &[String]) -> String {
    let lang = LANGUAGE.replace("__PROPS__", &props_doc(props));
    format!("{lang}\n## The universe\nEvery object belongs to this universe; match its tone, era, materials and palette:\n{bible}\n")
}

fn props_doc(props: &[String]) -> String {
    let vocab = crate::sim::props::BUILTIN;
    props
        .iter()
        .map(|p| match vocab.iter().find(|(n, _, _)| n == p) {
            Some((n, d, m)) => format!("- {n}: {m} (default {d})"),
            None => format!("- {p} (this universe's own property)"),
        })
        .collect::<Vec<_>>()
        .join("\n")
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
- 3 to 6 biomes. base = mean ground height in metres (-10..30; below 0 makes lakes and coast), amp = hill height in metres (2..50; keep towns, cities and farmland at 2..6 so buildings stand on level ground, and save big hills for wild land), rough 0 (rolling) .. 1 (craggy), ground/ground2 = two ground colours that blend.
- scatter = items per 100 m² by tag (trees 0.1–1.5, rocks 0.1–0.6, bushes 0.2–1, grass 0.5–3). Use the tags of your base types below; "grass" tufts already exist.
- fog: 1 = clear air, up to 3 = misty.

Optionally, in the same JSON object, the universe's own nature: properties and rules beyond the built-in physics (fire, water, breaking, growing already exist; don't repeat them). Only if this universe really has its own forces (magic, curses, rot, radiation, holiness, static, spores…):
  "properties": [ { "name": "cursed", "default": 0, "meaning": "how cursed it is, 0..1" } ],
  "rules": [ { "name": "curses spread by touch", "near": 1.5, "when": "self.cursed > 0.5 && other.cursed < self.cursed", "do": ["other.cursed += 0.05 * dt"] } ]
Rule language: `when` is a condition and `do` a list of assignments (=, +=, -=, *=) on self.<property> or other.<property>; you may use numbers, + - * /, comparisons, && || !, min(a,b), max(a,b), clamp(x,lo,hi), abs(x), dt (seconds), dist (metres apart, with "near"), hour, night (0/1), water (1 when in water), held (1 when held). Without "near" a rule applies to each thing alone; with "near": r (≤ 10 m) to each pair within r. Spread slowly (rates times dt), at most 12 rules.

Optionally, the universe's peoples and beasts beyond plain humans (people always exist; dogs, cats, horses, wolves, goats and deer are built in for earthly worlds). Add species only if this universe has them (elves and orcs, a race of giants, dragons, griffins, lizard folk…), up to 5, in the same JSON object:
  "species": [ { "name": "elf", "plural": "elves", "body": "figure", "size": 1.0, "mind": "sapient", "speech": "words", "social": "village",
                 "diet": { "plants": 0.8, "meat": 0.2 }, "temper": { "bold": 0.4, "wary": 0.6, "playful": 0.5, "tame": 0.5 },
                 "move": { "walk": 1.4, "run": 3.5, "fly": 0, "swim": 0.8 }, "life": { "sleep": [23, 6] }, "mass": 60,
                 "look": { "height": [1.85, 2.0], "build": [0.7, 0.85], "skin": [0.0, 0.25] }, "sounds": [], "description": "tall, quiet forest folk" } ],
  "attitudes": [ { "a": "elf", "b": "orc", "affection": -0.4, "trust": -0.3, "rivalry": 0.3 } ],
  "sizes": { "human": 1.0 }, "traveller_height": 1.75
- body: "figure" (a person's body; its look sliders are height in metres 1.3–2.0, build 0.7–1.4, skin 0–1, shirt and trousers hue 0–1), "quadruped" (a four-legged animal about 1 m tall at size 1; sliders legs, length, ears, hue, shade, all 0–1), or a new body name with "body_description": what it looks like and how it moves (a dragon with wings, a serpent body, a giant beetle); new bodies are written for you. Nothing smaller than a cat (about 0.3 m).
- mind: "sapient" (people: talk, plan, make things), "simple" (clever animals), "instinct" (beasts). speech: "words", "sounds" (noises listed in "sounds") or "none". social: solitary, pair, pack, herd or village. temper and diet values 0–1; size multiplies the body (a quadruped at 1.8 is horse-sized); mass in kg; move speeds in m/s; sleep: [from hour, to hour].
- attitudes: how peoples start out feeling about each other (-0.8..0.8); only the start, people's own history takes over. sizes: multiply a species everywhere ("everyone is a giant": "human": 1.8). traveller_height: the player's own height in metres (only if the world says they are small or big).

2. Then 4 to 8 base object types, each in its own ```js block, following the module rules exactly. These are scattered across the land by the scatter densities (trees, rocks, bushes, flowers, reeds…), so each must be small to medium (bounds under ~8 m) and varied per instance with hash(k.seed). Give each the scatter tag it fills (e.g. "tree", "rock", "bush", "flower") and fitting meta.props (trees and grass burn and are alive).
"#;

pub const REGION_TASK: &str = r#"Plan the story layer of one region (256 m × 256 m). Reply with one ```json block:
{
  "name": "region name (2–3 words)",
  "mood": "one line",
  "facts": ["short lore lines that people living here know", "..."],
  "new_types": [ { "name": "…", "description": "what it looks like, materials, colours", "size_m": [w, h, d], "tags": ["building"], "props": { "burns": 0.4 } } ],
  "landmarks": [ { "type": "type name", "x": 0-256, "z": 0-256, "rot": degrees, "scale": 1.0, "why": "why it is here" } ],
  "settlement": { "name": "…", "x": 0-256, "z": 0-256, "buildings": [ { "type": "type name", "dx": metres, "dz": metres, "rot": degrees } ] },
  "characters": [ {
      "name": "…", "age": 30, "appearance": "…",
      "look": { "height": 1.75, "build": 1.0, "skin": 0.0-1.0, "shirt_hue": 0.0-1.0, "trousers_hue": 0.0-1.0 },
      "personality": "…", "goals": "…", "voice": "how they speak",
      "home": "where they live", "home_x": 0-256, "home_z": 0-256,
      "relationships": ["Name: relation"], "species": "human", "variety": "", "layers": [] } ],
  "creatures": [ { "species": "goat", "count": 4, "x": 0-256, "z": 0-256, "names": [], "owner": "Name of their person, or empty", "description": "…" } ],
  "new_species": [ ],
  "varieties": [ ]
}
Rules:
- Coordinates are local to the region: x and z from 0 to 256.
- 0–3 landmarks, 0–1 settlement (with 1–6 buildings), 0–6 characters, 0–3 new_types. Empty regions are fine sometimes: wilderness has value.
- Reuse existing types by exact name when they fit; only invent new_types the region really needs (a settlement needs at least one building type).
- Put things on dry land (see the terrain notes), settlements on gentle ground, landmarks where they would be seen.
- Give each character a trade or daily work in "goals" (what they make, mend or tend, and something they want to make or improve), e.g. "mends the fishing nets; wants to build a proper boat".
- Characters live near the settlement or a landmark. home_x/home_z is where they stand by day: a spot a few metres outside their house (never the building's own coordinates). Give them distinct voices, goals and relationships with each other ("Name: relation", e.g. "Ola: daughter", "Bren: rival", "Tam: husband"): families, couples, friends and rivals make a village come alive. Weave in the region facts and the neighbouring regions.
- Small things people use (tools, balls, food, lamps) are welcome as new_types too, placed as landmarks near where they belong.
- Characters are people by default; give "species" (one of the species listed below) for anyone else who talks and plans (an elf, an orc). Their "look" uses their body's sliders.
- creatures: 0–4 groups of beings that don't talk (herds by farms, a dog or cat with its person, wild packs, a beast in its lair), from the species listed below. Pets and working animals name their "owner" (a character of this plan). Wilderness may have wild herds or predators; villages, pets and livestock.
- Clothing and gear that shows (armour, cloaks, robes, hats, a saddle or collar for an animal) are layers: new_types with tags ["layer"], "fits": the body they are worn on ("figure" for people, "quadruped" for four-legged animals, or a species' own body) and their props (armour { mass: 12 }, a wool cloak { burns: 0.7, mass: 2 }). A character's "layers" lists what they wear (at most 3, by type name).
- varieties: what a people looks like in this place, when it differs: { "species": "human", "name": "warrior", "look": { "build": [1.1, 1.35] }, "layers": ["iron breastplate"] }. Give a settlement a "variety" to dress and shape everyone living there (a warrior village, monks in robes, hill folk); a character's own "variety" overrides it.
- new_species: at most one, only if this region really holds a people or beast the universe doesn't have yet, in the species format of genesis (with "body_description" if no existing body fits).
"#;

pub const TYPE_TASK: &str = "Write this object type as one ```js block containing the complete module.";

/// Extra rules for a layer: it is drawn in the body's frame and pose.
pub fn layer_note(body: &str, body_source: &str) -> String {
    format!(
        r#"This is a layer (clothing, armour or gear) worn on the "{body}" body: add `fits: "{body}"` to meta and keep "layer" in its tags. It is drawn in exactly the body's local frame with the same k (look sliders k.a … k.e, walk phase k.f, pose roles k.s0 … k.s7), so follow the body's own maths for where its parts are in every pose (scale with its height, lean with its torso, swing with its legs). Make the layer a thin shell just outside the body's surface where it covers it (about 1–3 cm), not a solid block, and only where it is worn. Its bounds should match the body's bounds.
The body's code:
```js
{body_source}
```"#
    )
}

/// Writing the body of a species: a type with `meta.body` that answers to
/// pose roles, drawn at its natural size.
pub fn body_task(name: &str, description: &str, species: &str) -> String {
    format!(
        r#"Body type to write: "{name}"
What it looks like and how it moves: {description}
The species that lives in it: {species}

A body is an object type that beings live in. Besides the usual module rules it has `meta.body` and its shape moves with a pose:
- Draw it at its natural size in metres, standing on y = 0, facing +z. Give it the tags ["body"].
- meta.body = {{ height: m, eye: m (eye height), radius: m (footprint radius for walking), reach: m, grip: [x, y, z] (where it holds a thing: hand, mouth or claw), seat: [x, y, z] (only if it can be ridden: where a rider sits), roles: [...], gait: "biped" | "quad" | "slither" | "hover", flies: true|false, arms: false, look: {{ name: [lo, hi], ... }} }}
- Pose roles, read as k.s0 … k.s7 (each 0 at rest; give each the meaning that fits this body and list only the ones it answers to in roles): k.s0 / k.s1 raise_l / raise_r (raise a left / right limb or wing, 0 … 1), k.s2 / k.s3 reach_l / reach_r (reach forward or wrap around someone, 0 … 1: a hug), k.s4 lean (bend forward, radians -0.3 … 0.6), k.s5 head (head down, -1 … 1), k.s6 crouch (0 standing … 1 lying down), k.s7 spread (tail, wings or frill, -1 … 1). Use "raise" / "reach" in roles for both sides. Every listed role must visibly move the shape.
- k.f is its walk phase in radians (swing legs, undulate a tail with sin(k.f)).
- look: up to 5 sliders, read as k.a … k.e in that order, each between its lo and hi (colours, horn length, proportions); vary them per being. Use hash(k.seed) for small details.
- Keep it cheap: about 12–18 primitives. Stay inside meta.bounds in every pose.

Write it as one ```js block containing the complete module."#
    )
}

/// Rewriting one thing's shape: the current module, the change, and where.
/// `with`: another thing to work into it (name, module, its size relative to this one).
pub fn edit_task(name: &str, source: &str, change: &str, spot: &str, cuts: &[[f32; 4]], with: Option<&(String, String, f32)>) -> String {
    let spot = if spot.is_empty() { "not given".to_string() } else { spot.to_string() };
    let cuts = if cuts.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = cuts
            .iter()
            .map(|c| if c[3] > 0.0 { format!("a sphere of radius {:.2} at ({:.2}, {:.2}, {:.2})", c[3], c[0], c[1], c[2]) } else { format!("a cube of half-size {:.2} at ({:.2}, {:.2}, {:.2})", -c[3], c[0], c[1], c[2]) })
            .collect();
        format!("\nPieces already cut out of it (subtract these too, so they stay): {}.", list.join("; "))
    };
    let with = match with {
        Some((wname, wsrc, rel)) => format!(
            "\n\nWork this other thing into it, at the touched spot: a {wname}. In the object's units it is {rel:.2} times the size its own module draws it, so scale its shapes by {rel:.2}. Take its shape and colours from its module:\n```js\n{wsrc}\n```"
        ),
        None => String::new(),
    };
    format!(
        "Change this existing object. Here is its current module:\n```js\n{source}\n```\n\nThe change: {change}\nWhere it was touched (in this module's own coordinates, the ones sdf receives): {spot}{cuts}{with}\n\nRewrite the whole module with the change made. Keep everything else the same: the same coordinate frame and origin, the same look and colours, roughly the same bounds (grow them only if the change needs it), the same props and tags. Set meta.name to exactly \"{name}\". Reply with one ```js block containing the complete module."
    )
}

pub const CREATE_TASK: &str = r#"Someone is making something in the world by asking for it. You see what they see (JSON below). Decide what to build and where.

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
- Give a new type fitting meta.props (a ball bounces, bread is edible, a lamp gives light, a wooden thing burns). Small things (under ~1 m) can be picked up and used.
"#;

pub fn repair(errors: &str) -> String {
    format!("That failed validation:\n{errors}\n\nFix every problem and reply again in the same format, with the complete corrected answer.")
}

pub const DIALOGUE_RULES: &str = "You are a character in Pocket Universe, a small living world. Stay in character. Speak in your own voice, in 1–3 short sentences (this is a terminal; keep it brief). No stage directions, no lists, no markdown. You remember earlier conversations with the traveller (the player) from your memories below; refer to them naturally when relevant. Never state clock times; speak of when things happened loosely, as a person would (\"just now\", \"earlier\", \"yesterday\"). You only know what your character would know. If you are asked about things outside your world, respond as your character would.";

pub const DECIDER_TASK: &str = r#"You decide what a character in a small simulated world does next, given an event and what they know. Reply with one JSON object only:
{"goal": "a few words", "say": "what they say now (one short sentence in their voice) or null", "steps": [ ... ]}
Steps are actions, carried out in order (walking there first when needed). Use names of things and people you were told about:
  {"do": "goto", "target": "Mara"}            {"do": "hold", "target": "ball"}        {"do": "drop"}
  {"do": "throw", "at": "hoop"}                {"do": "give", "to": "Ola"}             {"do": "eat", "target": "apple"}
  {"do": "use", "target": "lantern", "on": "woodpile"}                                {"do": "say", "text": "…", "to": "Ola"}
  {"do": "gesture", "kind": "wave|bow|nod|point|cheer|shrug|dance|sit|handshake|high_five|hug|kiss", "to": "Ola"}
  {"do": "propose", "to": "Ola", "activity": "catch|carry|dance|walk|hug|…", "with": "ball"}   (doing something together)
  {"do": "do", "text": "carve a notch in the door"}   (anything else, in words, including making something new: "make a wooden ball"; only when it really fits who they are)
  {"do": "follow", "target": "the traveller"}  {"do": "wait", "secs": 5}              {"do": "go_home"}
Keep plans short (1–5 steps), in character, and grounded in what is actually around them. If nothing is worth doing, reply {"goal": "", "steps": []}.
For an event "player_near", a plan may simply be [{"do": "goto", "target": "the traveller"}] with "say" set, or nothing."#;

pub const SUMMARY_TASK: &str = "Update this character's private memory summary. Write at most 120 words in the first person, covering what they know and feel about the traveller (the player), promises, recurring topics, and notable things they witnessed. Keep the important older points. Plain text only.";

pub const INTERPRET_TASK: &str = r#"You are the physics and common sense of a small simulated world. Someone says, in words, what they do or make. Decide what happens, as changes to things, not just words. Reply with one JSON object only:
{
  "narration": "one or two sentences: what visibly happens",
  "changes": [ { "target": "held" | "target", "props": { "<property>": number }, "state": { "s0": number } } ],
  "create": [ { "name": "new thing", "replace": "held" | "target" | null, "description": "looks, materials", "size_m": [w, h, d], "props": { "<property>": number } } ],
  "remove": [ "held" | "target" ],
  "make": [ { "text": "what to make and where, in words, e.g. a stone well by the path" } ],
  "cut": [ { "target": "target", "size_m": 0.25, "shape": "round" | "square" } ],
  "reshape": [ { "target": "target", "name": "a new name for the changed thing, different from its current name, e.g. slate cottage with the door open", "change": "what changes about its shape, precisely", "with": "held" | null } ],
  "say": "a few words the actor says, or null",
  "cache": true
}
- "held" is what the actor holds; "target" is what the middle of their view points at (a thing with its distance, or the ground or a far point). Use only property names from the list. Set "cache": false if the result depends on chance or the moment.
- Prefer small, plausible results. Things can be made from things (carving wood makes a carving; replace the wood). If nothing would happen, say so in the narration and change nothing.
- Decide from the words whether they change what they point at or make something new; the target is only a hint. "Make a stool", "a lighthouse on that hill", "build a fence here" are "make" (it is placed where they look), even when a thing is pointed at. "Fix the roof", "open it", "remove the cover", "add this stick to the wall" change the target. "Carve a bowl from this log" makes from it: "create" with "replace": "target". "make" is for things that stand in the world (buildings, furniture, structures, landmarks); "create" is for small loose things that come from the deed (a piece that comes off, a carving, a spark).
- The world only shows what your changes do, never what the narration says. If the target visibly changes form (something on it is removed, opened, broken off, added, bent, dug), you must change its shape with "cut" or "reshape"; props alone change nothing you can see.
- Changing a thing's shape happens where it was touched ("touched_at", in the thing's own coordinates):
  - "cut": take a piece out (punch a hole, dig, bite, chip, carve a notch). size_m is the radius in metres; a cut deeper than a wall is thick goes right through. It shows at once; the engine places it, you only say how big.
  - "reshape": change the shape itself: remove a part of it (the door cover, a roof tile, a branch), open or bend it, make the roof a dome, add a chimney. With "with": "held", work the held thing into it (add the stick to the wall, mount the wheel on the boat, hang the lantern on the post): it becomes part of the target and is used up. Describe the change precisely, including where.
- Taking a piece off a thing is two changes: "reshape" the target without the piece, and "create" the piece as a new thing (it lands nearby)."#;

pub const CHAT_TASK: &str = "Two characters in a small living world meet and talk briefly, in their own voices, about what is on their minds (what they saw, what they are doing, each other). 2 to 4 short lines, plain speech, no stage directions. Reply with one JSON object only: {\"lines\": [{\"who\": \"Name\", \"text\": \"…\"}]}";

pub const GESTURE_TASK: &str = r#"You animate a simple body in a small 3D world (a person, unless the body is described below). Write the gesture named below as a few key poses. Reply with one JSON object only:
{"duration": seconds (0.5–8), "contact": false, "distance": metres to the other person (0.4–3; only matters with contact), "intimacy": affection needed to agree, -1..1 (contact gestures only),
 "frames": [{"t": 0, "pose": {}}, {"t": 0.3, "pose": {"raise_r": 0.6, "reach_r": 0.2}}, …, {"t": 1, "pose": {}}]}
Pose roles (all default 0, the body at rest). For a person: raise_l / raise_r: left / right arm raised sideways, 0 down … 0.5 level … 1 straight up. reach_l / reach_r: arm reaching forward, 0 … 1 straight ahead. lean: bend forward at the hips in radians, -0.3 … 0.6. head: head tipped down, -1 … 1. crouch: 0 standing … 1 crouching. spread: a tail, wings or frill (-1 … 1), for bodies that have one. Other bodies give each role their own meaning (described with the body); use only the roles that body has.
Start and end at rest ({}), 3 to 8 frames, t from 0 to 1. Set "contact": true only for things done touching another (an embrace, a dance hold, a forehead touch)."#;
