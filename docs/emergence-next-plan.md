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

## Decisions

- The **player's body stays out** of the rules pass: the traveler is never
  cursed, soaked or burnt by rules.
- **No lies**, except what corruption twists: a corrupted character may pass
  on beliefs wrongly; nobody else does.
- **Beliefs are capped per character** (setting `beliefs_per_mind`, default
  200): the least important, least sure and oldest fade first, and repeats
  merge into one row. A world's file stays roughly flat however long it is played.

## Order

1. Bodies have properties (smallest; unlocks curses, weather, corruption on people).
2. Goals as data (builds on missions; makes promises real).
3. One scorer (needs goals as a source of things to do).
4. Beliefs (biggest; best with the rest in place).

Each phase ends with its proof as a test in `src/sim/tests.rs`, a headless
run of a real world with no invariants broken, and a commit.

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

## Not in this plan (next)

- Shared activities as data (joint plans with roles) instead of the five built in.
- One "pursue and touch" for predators and night beings; one "arrival" for
  births, made beings and horrors.
- Groups and institutions (goals with a group as owner, then roles and norms).
