# Increased emergence: bodies, goals, one scorer, beliefs

Agreed 2026-10-03, on branch `increased-emergence`. The aim is a world that
still makes sense in five years, when LLM calls are nearly free: general
primitives that behaviour emerges from, the LLM as the main brain, and
code-written fallbacks kept small and derived from data.

## Already on the branch

- **Property metadata** (`PropMeta`): when a change is news, whether it
  spreads as incidents (and the words for them), harm, what carries over.
  Fire is one entry; a universe's own properties get the same from genesis.
- **`apply`**: work a tool or your hands against something, by the world's
  rules. `force` is set on the tool while it works; "beaten out" is a rule.
- **Foresight** (`Sim::foresee`): characters imagine each tool to hand against
  a trouble and take what pushes it back most. Nothing knows what fire is.
- **Typed aim** instead of reading `doing` text.
- **LLM limits as settings**, a weighted queue, simple minds that plan now and
  then, a state-aware deed cache, properties instead of names for what people
  throw at and pick up, personality as numbers, small talk from what each one
  witnessed.

What that changed so far: a world's own forces (curses, rot) are told, spread
as incidents, avoided and fought (new worlds only: older worlds' properties
have no metadata); people fetch tools their world's rules make useful; pets
and simple animals sometimes plan with the LLM; small talk is about what each
one saw. Fire behaves about as before.

Known regressions to weigh: bare hands save fewer tufts than the old douse
(about 5 of 50 against 12 in the test); shouts are generic ("Careful, the
grass tuft!") rather than written for fire.

## For the agent doing this

Read this whole plan first, then `docs/emergence-plan.md` and
`docs/night-plan.md` for intent. Work one phase at a time, in order.

**Setup.**
- Work on branch `increased-emergence`. If other work is going on in the
  same folder, use a separate git worktree for the branch.
- `main` has moved ahead (the "sounds!" commit). Merge `main` into the
  branch before starting, fix conflicts, and get the tests green.
- Commands: `cargo test --release` (all must pass), `cargo build --release`
  at the end of each phase. A headless run of a real world:
  `target/release/pocket sim COPY.pocket --hours 6 --no-llm --events out.jsonl`.
  Always run it on a **copy** of a file from `~/.pocket/universes/`,
  never on the original.

**Rules of the house.**
- No behaviour keyed on names, tags, event-kind strings or `doing` text:
  properties, taxonomy, typed `Aim`, vocabulary metadata.
- Match the surrounding style: short plain-English doc comments that say
  why, as in the rest of `src/sim`.
- Keep the night system's behaviour (its difficulty table, charges, what
  horrors do) as it is; only move how it is built onto the new primitives.
- The traveler's body is never in the rules pass.
- Every phase ends with its proof test(s) in `src/sim/tests.rs`, the
  measurements below recorded in the commit message, the README updated where it
  describes what changed, and one commit (ending with the Co-Authored-By line the
  harness gives).
- Test helpers already there: `world`, `world_with`, `add_type`, `place`,
  `add_char`, `session`, `record`, `of`, `events`, `dry_spot`,
  `grass_patch`, `clear_scatter`, `plant_grass`, `sound`. The curse test
  (`a_villager_lifts_a_spreading_curse_with_a_charm_nobody_told_them_about`)
  is a good model for a universe property with metadata.

**Where things are.**
- Phase 0:
  - `src/sim/interp.rs` sets properties by name and skips unknown ones (search `vocab.id(k)`);
  - `src/brain.rs` `validate` has the unknown-property check for types, to copy;
  - `src/sim/props.rs` `PropMeta`, `Vocab`, `P_FORCE`;
  - `src/sim/actions.rs` `apply` / `body_props`;
  - the "beaten out" rule is in `src/sim/rules.rs` `builtin_specs`.
- Phase 1:
  - `src/sim/actor.rs` `Actor`, and `src/sim/npc.rs` `Npc` (fields `corruption`, `glow`);
  - `SavedState` in `src/world/characters.rs`;
  - `Species` in `src/world/species.rs` (add `props`);
  - the rules pass `rules_pass` and `Key` in `src/sim/env.rs`;
  - `src/sim/night.rs`: `corruption_of`, `add_corruption` and `night_events` (the event-name checks to remove);
  - gestures with contact in `src/sim/social.rs`;
  - `contact()` in `src/sim/actions.rs`.
- Phase 2:
  - missions in `src/sim/needs.rs`;
  - talk in `src/brain.rs` (`Cmd::Talk`) and `DIALOGUE_RULES` in `src/prompts.rs`;
  - `decide_context` and `on_decision` in `src/sim/npc.rs`;
  - tables in `src/db.rs`, saving in `src/sim/persist.rs`.
- Phase 3:
  - `think` and `counter_trouble` in `src/sim/npc.rs`;
  - `think_animal` in `src/sim/beings.rs`;
  - settings in `src/sim/config.rs`;
  - the planner prompt `DECIDER_TASK` in `src/prompts.rs`.
- Phase 4:
  - `witness` in `src/sim/mod.rs`;
  - gossip in `src/sim/social.rs` (search "told me");
  - memory retrieval in `src/brain.rs` (search "overlap").
- Measurements: the report of `run_sim_cli` in `src/sim/headless.rs`.

**When to stop and ask.** A change to what the player sees or can do that
this plan doesn't describe; a test that only passes by weakening it; a
measurement that gets worse after a phase.

## Decisions

- The **player's body stays out** of the rules pass: the traveler is never
  cursed, soaked or burnt by rules.
- **No lies**, except what corruption twists: a corrupted character may pass
  on beliefs wrongly; nobody else does.
- **Beliefs are capped per character** (setting `beliefs_per_mind`, default
  200): the least important, least sure and oldest fade first, and repeats
  merge into one row. A world's file stays roughly flat however long it is played.

## Order

0. Vocabulary hygiene (small; makes every later phase sturdier).
1. Bodies have properties (smallest; unlocks curses, weather, corruption on people).
2. Goals as data (builds on missions; makes promises real).
3. One scorer (needs goals as a source of things to do).
4. Beliefs (biggest; best with the rest in place).

Each phase ends with its proof as a test in `src/sim/tests.rs`, a headless
run of a real world with no invariants broken, and a commit.

## 0. Vocabulary hygiene

Properties are capped (a genesis adds at most 8; 64 in all; none appear mid
game), so the risk is not sprawl but meaning drifting and near-miss names.
- **Deed answers are checked** like object types: a property the world
  doesn't know ("curse" in a world of "cursed") is sent back to be fixed,
  never silently dropped (today `interp.rs` ignores it).
- **Aliases, once per world:** a near-miss name is mapped once (exact, then
  known aliases, then one LLM ruling), stored with the world and reused.
- **Units and anchors** in each property's metadata ("0.2 a blessed candle,
  1 a saint's relic"), shown in every prompt that writes values, so scales
  agree across types.
- **Act properties apart:** `force` (and later `kindness`) say what an action
  is doing right now, not what a thing is. They move out of the vocabulary
  into a small set rules can read but nothing generated can write.
- **Built-ins are earned:** none added without a phase that needs it.

**Proof.** A deed answer with "curse: 1" in a "cursed" world is repaired and
takes effect; the alias is reused without asking again; a type cannot set `force`.

## 1. Bodies have properties

**What.** Every character's body gets a property vector like a thing's
(`Npc.props`), started from its species (mass, body heat 36°, plus any
`props` the species declares: a fire spirit burns, a ghost is cold). Saved as
the difference from the species, like things. The player has none.

**Bodies in the rules pass.** Characters within range join the pass as
acting entities (`Key::Actor`), so a world's own rules reach them: a curse
that spreads by touch curses a person, a kiln warms those by it, wading wets.
- A new built-in property `body` (1 for beings). The plant rules
  ("killed by heat", "grows") skip bodies. Universe rules apply to all.
- What someone holds or wears touches them every pass (a pair at gap 0).
- Contact gestures (hug, handshake, kiss) run the pair rules once between the
  two bodies, as `contact()` does for things.

**Act properties.** An action can put a short-lived property on the body
doing it, as `apply` puts `force` on a tool: a warm gesture gives `kindness`
for the moment of contact. Rules do the rest.

**Corruption moves into the vector.** `corruption_of` / `add_corruption`
read and write `props[P_CORRUPT]`. Spreading becomes a built-in rule
(darkness passes on touch, gated by difficulty through a property the
difficulty table sets); kindness easing it becomes a rule on `kindness`.
The checks on event names ("gave", "together_end", gesture names) go.
`glow` becomes body `light` that fades by a rule. Old saves: a saved
`corruption` field is read into the vector.

**What the engine reads off a body.** Harm (by `PropMeta.hazard`) adds to
fear and pain; heat and wetness to comfort (a need later, see 3); corruption
twists as now. Crossings on bodies are told like any other ("Oda fell under
the curse"), except plant ones.

**Proof.** A cursed idol passed hand to hand curses whoever holds it; a
villager takes off a cursed cloak (by harm, no code for curses); corruption
spreads by a hug on Hard and eases with kindness, with no event-name checks;
a kiln warms the people by it and kills nobody.

## 2. Goals as data

**A goal row:** owner, want, why, priority, deadline, progress, status,
promised to (if anyone), steps so far.
- **Want** is a checkable condition where it can be: "a thing of kind K is in
  my hands", "thing T is at place P", "incident I is over", "affection with
  X ≥ 0.6", "someone has a Y"; otherwise free text the LLM judges.
- **Sources:** the persona at start (the LLM writes initial goals), missions
  (today's `needs.rs` place / thing / someone / time become goal kinds),
  **promises in talk** ("I'll make you a ball" becomes a goal with a
  promisee; the prompt rule "what you agree to do you really do" goes),
  incidents (pushing one back is a goal), and the planner's longer aims.
- **The sim checks conditions** each few seconds: met is done (remembered,
  thanks given), past its deadline or hopeless is given up (remembered,
  trust drops with a promisee). This is the missing "did it work" loop.
- Goals are in the planner's context and are a source of things to do for
  the scorer (3). Groups, later, are goals with a household or village as owner.

**Proof.** Asked for a ball, a character promises, makes it over time and
hands it over; the goal closes itself. A promise not kept by its deadline is
dropped, remembered, and costs trust.

## 3. One scorer, the LLM as main brain

**Things to do (affordances).** Each candidate is a verb, a target, how much
it relieves each need, and a cost (distance, time, risk). They come from:
- things, by properties (food for hunger, toys and marks for fun, new things
  for curiosity, harm as a negative);
- people (talk, joint activities), weighted by the relationship;
- places (home and shelter for tiredness at night, cover from harm);
- incidents (push back, by foresight; or watch);
- goals (the next step toward each, by priority).

**Scoring.** Sum of need × relief × trait weight, minus cost; a weighted
random pick from the world's seeded random numbers (variety, but a seed
replays the same). Harm and threats are very high scores, not fixed rungs.
Species and mind only decide which verbs a body has (no hands: carry in the
mouth; no words: no talk). People and animals use the same scorer.

**The one table of hand design.** Which verb relieves which need, by which
property (eat: hunger by `edible`; play: fun by `toy`; rest: fatigue;
talk: social by the relationship). Everything else follows from data.

**The LLM as main brain.** When there is an LLM and its cooldown allows, the
planner gets the top 5–8 things to do as a menu of what is really there, and
may still choose freely. Between its decisions, and with no LLM at all, the
scorer is the brain. `think` and `think_animal` go.

**Migration.** First wrap today's menu items as affordances; run old and new
side by side behind a setting (`scorer`) and compare in headless runs (needs
met, variety of activity, nothing stuck); then remove the old ladders.

**Proof.** A day in a test village and in the user's real world: needs stay
met, activities are as varied as before or more, a fire is still fought, a
pet still follows, and no test that passed before fails.

## 4. Beliefs

**A belief row:** holder, about (a thing, person, place or kind), claim
(structured where it can be: an event id, a property and value, where
something is; plus text), source (saw / told by X / guessed), how sure, when.
- **Seeing makes beliefs:** `witness()` writes structured rows besides the
  memory text.
- **Gossip passes beliefs on**, marked "told by X" and a little less sure each
  hop. Honest people pass on what they believe, which can be wrong (stale,
  misheard): rumours emerge without lies. **Only corruption lies:** a
  corrupted teller may twist a claim (blame the wrong one, claim what didn't
  happen), with the twist marked in the row for inspection.
- **Use:** the planner, talk and the scorer look up beliefs about what is in
  view or named: "where is the ball" goes where they believe it is, which can
  be stale; people disagree about who started the fire.
- **Cap and fading** (`beliefs_per_mind`, default 200): importance × sureness
  × recency decides what stays; a repeat raises sureness instead of adding a
  row. Memory text stays as the story; beliefs are what can be asked.

**Proof.** Two villagers who saw different parts of a fire hold different
causes; one who only heard of it believes the teller, less surely; a
corrupted teller spreads a false cause and those who saw it themselves don't
take it up. A long headless run keeps belief rows under the cap.

## Measuring emergence

Each phase must show more stories, not just more activity. A headless run of
a test village and of a real world, before and after, reports:
- distinct kinds of events per game day;
- cause chains three or more steps long (incident causes, goals that needed
  someone else, beliefs that changed a plan);
- beliefs held differently by different people (phase 4);
- goals that cross between characters (phase 2);
- needs kept met, and nobody stuck.

`pocket sim` gains these numbers; the phase's commit records them.

## LLM cost

The number of calls is set by `llm_per_min` and none of this raises it;
what changes is who gets the calls and how long the prompts are. Decisions
are about 1% of a world's spend today (types and regions are about 90%).
- Bodies: no calls.
- Goals: promises ride on the talk reply already made; starting goals on the
  region call; the sim checks most goals itself; free-text goals get at most
  one LLM check, at their deadline.
- Scorer: the menu adds about 200 tokens a decision; with no LLM it is the
  whole brain.
- Beliefs: about 300 tokens of retrieval a decision or talk; passing them on
  is pure sim, and so is corruption twisting them.

Expected: decision prompts 30–60% longer by the end, a few percent on the bill.

## Not in this plan (next)

- Shared activities as data (joint plans with roles) instead of the five built in.
- One "pursue and touch" for predators and night beings; one "arrival" for
  births, made beings and horrors.
- Groups and institutions (goals with a group as owner, then roles and norms).
