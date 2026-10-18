# Multiplayer plan

One player hosts a world, a few friends join it over the local network or
an ngrok tunnel. The host runs the whole simulation and every LLM call and
pays for them. Guests need no key and no world file. A few friends, not an
MMO: up to about 4 guests, trusted, and the host's machine is the server.

Built on the `multiplayer` branch (steps 0–6 below). What differs from the
first design, and what is still missing, is at the end ("As built").

## What the code assumes today

- The traveler is one field, `Sim.player: Actor`, with `ActorId::Player`
  (code `0`, key `"player"`). About 400 `ActorId::Player` sites and 250
  `player.pos` sites, most of them in tests.
- Traveler state is spread out: `Sim.own` (look), `look_pitch`,
  `talking_to`, `hover`, `recent_player`, `player_plan`, and the traveler
  half of `NightState` (`wounds`, `fallen`, `glow`, …) mixed with the world
  half (`walkers`, `phantoms`, `nights`).
- Saved in the `player` table (`CHECK (id = 1)`, also holds the world clock
  `t_game`), kv `player.body`, kv `sim.night`, and `things.holder` /
  `co_holder` = `player` / `worn:player`. `share::reset_travelers` lists them.
- NPC data names the player by code: relationships use `0`, events use
  `"player"`. Memories and belief claims are free text ("the traveler").
- The world is simulated around one point: `dist_to_player()` drives NPC
  detail, physics wake-up, the LLM queue order, region streaming and
  catch-up, night spawns, weather and the audio listener.
- Types are stored as LLM-written source (`types.code`) and compiled on load.

## 1. What goes over the wire

**Transport: WebSocket over TCP**, one connection per guest. Reasons:
ngrok's free plan tunnels HTTP (WebSocket rides on it); its TCP tunnels need
a verified account, and it carries no UDP at all. On a LAN the same socket
works directly. TCP's head-of-line stalls don't matter much for walking
with friends. `tokio` is already a dependency; add `tokio-tungstenite`.
Messages are binary (`postcard` or `bincode` over serde), not JSON, with
positions as `f32`.

**Three kinds of messages, host to guest:**

1. **Join: the world itself, once.** The host makes a guest copy the way
   sharing does (`share::make_copy`, `VACUUM INTO` then clean: no keys,
   spend or settings, and memories, beliefs and goals kept so the inspect
   view shows guests the same as the host). That one file carries
   the types and their code, instances, regions, terrain seed, ground edits
   and species, so the guest builds its `WorldSnapshot` and compiles types
   with the code it already has. It is sent over the same WebSocket in
   chunks with a progress bar and opened in memory only (SQLite
   `deserialize`): nothing is written to the guest's disk, and it is gone
   when the guest leaves. A cleaned world is usually a few MB.
2. **State: snapshots of what moves, 20 times a second.** Each tick the
   host sends each guest the things near that guest that changed since the
   last send (TCP doesn't lose anything, so "since last sent" is enough, no
   acks): every traveler and awake NPC (position, yaw, pose, motion, held
   thing, gesture, face), awake things (position, tilt, state, the props the
   renderer reads), burning or wet cells, and every 2 s the clock and the
   weather. Near is the host's `cfg.medium` range around that guest.
   Far or sleeping things are sent when they change, or at most once a
   second. Something leaving range gets a "gone".
3. **Events: everything that happens once, in order.** A new or changed
   type (name, code, meta: the guest compiles it the way load does), new or
   removed instances, a region done, ground dug or built, someone speaking
   (text and voice tone), log and journal lines, sounds
   that aren't motion (a hit, a shot, a door), the night starting.

Snapshots for moving things, events for the rest. No general diffing of
the database.

**Guest to host:** input, not state. Each frame the guest sends what it
wants (move direction, run, crouch, jump, yaw and pitch, the key actions:
take, use, throw, get in, talk to X) plus its own predicted position (see
below), and typed text: talk lines and `/do`.

## 2. What runs on the guest

- **Rendering**: everything, from a *mirror* `Sim`: a `Sim` that is never
  stepped, only filled from the host's snapshots and events, so
  `Sim::draw` and the renderer work unchanged. Other bodies are drawn 100 ms
  behind and blended between the last two snapshots.
- **Its own movement, predicted.** The guest moves its own body at once
  with the same motion and collision code, and sends where it ended up.
  The host checks it (no faster than running, not into walls) and keeps it.
  When the host moves the guest itself (in a vehicle, carried, fallen,
  pushed, thrown), it says so and the guest follows the host until that
  ends. No rollback: with friends this is enough.
- **Sound**: footsteps and motion sounds come from the mirror locally;
  everything else from events. The listener is the guest.
- **Voices**: NPC speech arrives as text with its voice settings and is
  spoken by the guest's own voice model, so audio never goes over the wire.
  Dictation runs on the guest and sends text.
- **Not on the guest**: the sim step, NPC minds, rules, physics, region
  generation, the LLM, saving, achievements.

## 3. Talking and `/do` from guests

A guest's talk line or `/do` goes to the host as text and joins the host's
LLM queue as coming from that traveler, ordered by distance like any other
request. The host sets, in its settings, **a spending limit per guest per
session** (default $0.50) and **a pace** (at most three requests in flight
per guest, so a quick `/do` isn't stuck behind a slow conjuring). Every LLM call already reports its cost; the host adds it to that
guest's tally in memory (never written to the world). When a guest's limit
runs out, they get a plain line ("the host's limit for you is spent") and
can still walk, take things and see the world. The host's own overall
budget still applies on top and pauses everyone.

## 4. Hosting and joining from the worlds list

- **H** on a world: **Host**. The world opens as usual and the host's top
  line shows: `Hosting · 192.168.1.20:7777 · code lime-otter-42`, with C to
  copy it. The code is a short random secret a guest must give, so nobody
  who finds an ngrok URL can play on the host's key.
- **J** on the worlds list: **Join**. Hosts on the local network show up by
  themselves (a small UDP broadcast, LAN only); pick one and type the code.
  Or paste an address plus code, e.g. `https://abc123.ngrok.app lime-otter-42`.
- **ngrok**: the host installs ngrok and signs in once. If `ngrok` is on the
  PATH, hosting starts `ngrok http 7777` itself, reads the public URL from
  ngrok's local API (`http://127.0.0.1:4040/api/tunnels`) and shows it in
  place of the LAN address. Without ngrok, the line says how to get it.
- **Build check**: the first message from each side carries `db::COMMIT`,
  the world file `FORMAT` and a protocol number. If they differ, the guest
  sees: "The host plays build ab12cd, you have ef34gh. You both need the
  same build to play together." (A `-dirty` build only matches itself.)
- **Name**: each player has a name and a random player token in their own
  settings (not in any world), sent when joining.
- Testing on one machine: `pocket --host <world>` and
  `pocket --join 127.0.0.1:7777 <code>`, alongside the buttons.

## Versions

There are no GitHub releases yet; the only build id is the git commit
(`db::COMMIT`). Friends will play from downloaded builds, so releases are
part of this plan (step 0 and step 6 below).

**Release numbers.** The `version` in Cargo.toml (now `0.1.0`) is the
release. `build.rs` exports `POCKET_RELEASE` next to `POCKET_COMMIT`: the
version when HEAD is at tag `v<version>` and the tree is clean, otherwise
`dev`. `db::RELEASE` sits beside `db::COMMIT`; worlds save
`made_with_release` / `last_opened_with_release` beside the commits. The
worlds list and the settings menu show `0.1.0` or `dev ab12cd` in small
type. Cutting a release: bump the version, commit, tag `vX.Y.Z`, push the
tag (written up in `docs/releases.md`).

**Release builds on GitHub.** A GitHub Actions workflow on a `v*` tag
builds the macOS app (Apple Silicon first), bundles it as `Infinite Pocket.app` in a zip or dmg, and attaches it to a GitHub release with
notes from the commits since the last tag. Signing and notarizing need an
Apple Developer account ($99/year); until then the app is unsigned and the
release notes say how to open it anyway (right-click → Open). Voice models
stay downloaded on first use, not in the bundle.

**Checking for updates.** On start the worlds list asks GitHub once for
the latest release (no key needed, can be turned off in settings) and
shows "0.5.0 is out" with the link. It never updates by itself.

With releases in place:

- **Exact match to start.** A guest runs no sim, but it does compile the
  types' code and draw from the host's numbers (prop slots, instance
  layout), so two releases can disagree in ways that are hard to see.
  Host and guest must have the same release.
- The hello carries the release (`0.5.0`), the commit and the world
  `FORMAT`, and is checked before the world is sent, so a mismatch costs
  nothing.
- **The message says who should update**: "The host has 0.5.0 and you
  have 0.4.2. Update from the releases page to play together." Or,
  when the host is the older one: "The host has 0.4.2, you have 0.5.0: the
  host needs to update." Release builds show the release number. Dev
  builds show the commit, and a `-dirty` build only matches itself.
- **The worlds list says when a new release is out** (one GitHub
  "latest release" request at start, can be turned off), so friends tend
  to be on the same one before they try.
- **The host updating moves the world forward.** A newer release may
  migrate the world file, and an older one then refuses it (the `format`
  check already does). Guests have no file, so they only need the same
  release.
- **Later, if exact matching gets annoying:** a separate `NET_PROTOCOL`
  number that changes only when messages or drawing change, so small fix
  releases can still play together.
- Voice packs are downloaded and used only on each player's own machine,
  so their versions don't need to match.

## 5. Players in the world, and what each keeps

**Who is who.** `ActorId::Player` becomes `ActorId::Traveler(u8)`, a small
per-world number. Traveler `0` is the world's owner. Its code stays `0` and
its key stays `"player"`, so **existing worlds load unchanged and their old
traveler becomes the host**. Other travelers get codes `-1, -2, …` (NPC ids
are positive) and keys `"player:1"`, `"player:2"`. The holder column follows:
`player:1`, `worn:player:1`.

**What NPCs know.** NPCs tell travelers apart by name in what they are told
and remember ("Ana, a traveler, gave you bread"), and by code in
relationships and events. Old memories say "the traveler" and stay with
the host. A shared copy (`reset_travelers`) drops every traveler but keeps
traveler `0` empty for whoever opens it next.

**A new table** `travelers(id, token, name, x, z, yaw, body_json,
wounds_json, last_seen)` takes over from the `player` table, kv
`player.body` and the traveler half of kv `sim.night`. The clock `t_game`
moves to kv. A guest is matched by their token. A migration moves the old
`player` row, `player.body` and the wounds into traveler `0`.

**When a guest leaves** their body goes with them. What they carry and
wear stays theirs (still held by `player:N`, not drawn and not in the world);
their place, look and wounds are saved in their row. When they come back
with the same token they stand where they left, as they were. A guest who
fell at night comes back the way the host does after a fall. Nothing of a
guest's is kept on the guest's own machine except a short list of hosts
they joined.

**When the host leaves** everyone is told and goes back to their worlds
list. The world doesn't run without its host.

**One player's things stay theirs.** Keys, `llm_usage`, settings and
achievements stay on the host's machine (guests get no achievements at
first). The guest copy of the world is made with the same cleaning as
sharing, so none of that is in it either.

## 6. Steps

**Step 0: release numbers.** `POCKET_RELEASE`, saved in worlds, shown on
the worlds list and in settings, `docs/releases.md`. Small, and the hello
in step 1 needs it.

**Step 1: two people see each other walk.** Smallest playable piece, with
no change yet to how the sim thinks about the player.

- `src/net`: WebSocket host and client, hello with build check and code,
  the world file sent on join.
- The guest opens the received copy, builds the snapshot and a mirror
  `Sim` (never stepped), and moves its traveler locally.
- Both sides send their traveler's position, yaw, motion and look 20
  times a second. The host relays each to the others.
- Both draw the other travelers as bodies, the way NPC bodies are drawn,
  with the look they chose (third person for others only, which the hands
  code already almost does).
- `--host` / `--join` flags only, no buttons yet. In step 1 the guest's world
  is still: NPCs stand where the file had them, nothing burns.
- Tests: a host and a guest in one test process over loopback: the build
  check refuses a mismatch, a guest's walk shows on the host and the other
  way round.

**Step 2: guests in the world.** The real change: `Sim.player` becomes a
list of travelers (with `sim.host()` for the old uses), the `travelers`
table and migration, `dist_to_player` becomes "nearest traveler", regions
load around each traveler, the night hunts all of them, and NPCs see, talk
to and remember them by name.

**Step 3: guests see the live world.** State snapshots and events to the
mirror, so NPCs move, fires burn, the clock and weather match, and new
types and regions arrive. Guest actions (take, throw, use, get in) go to
the host.

**Step 4: guests talk and `/do`**, with the per-guest limit and pace.

**Step 5: the buttons.** H and J on the worlds list, LAN discovery, ngrok
started by the game, the address line with copy.

**The public name is Infinite Pocket.** Releases, the app bundle, the
window title, the GitHub release page and player-facing messages ("made by
a newer Infinite Pocket") use it. World files stay `.pocket`, the binary
and command stay `pocket`, and the `POCKET_*` names stay. The few "Pocket
Universe" strings in `src` change in step 0, except inside LLM prompts if
changing them would throw away cached results.

**Step 6: releases on GitHub.** The Actions workflow that builds and
attaches the macOS app on a tag, the update check on the worlds list, and
the first tagged release, so a friend can download the same build and join.

Later: signing and notarizing, Windows and Intel builds, guests'
achievements, kicking a guest, guest voices heard by others.

## Decided

- Guests never get the world as a file: it lives in their memory while
  they play.
- In the game everyone sees the same: the same log and journal, the same
  NPC memories in the inspect view. A line about a player says "you" to
  that player and their name to everyone else.
- There is no pause today (the sim ticks while the settings menu is open),
  so nothing changes there.

## As built

- **Code**: `src/net` (wire messages, host, guest, the world's tables kept
  the same `rows`, local network `discover`, `ngrok`, `update`),
  `src/app/together.rs` (hosting and joining in the game), `src/sim/travelers.rs`
  (other travelers as bodies in the world), `src/sim/mirror.rs` (the live
  world as a guest sees it).
- **Traveler 0 stays in the old places** (the `player` table, kv
  `player.body`, the traveler half of `sim.night`), so no migration was
  needed; guests are rows of `travelers`. `ActorId::Player` is "this game's
  traveler" on both sides: on a guest's copy the host is `Traveler(0)`.
- **Movement**: a guest moves themselves and the host keeps it (a step over
  25 m is put back). No prediction beyond that is needed for a few friends.
- **The live world** goes as changes per guest at 20 Hz (things, beings,
  burning ground, flashes, conjurings, weather, the guest's own wounds); the
  world's tables (types, instances, regions, people, species, made ground)
  are watched on the host's file and sent as rows; the guest's copy makes
  its world anew from them in the background.
- **Who pays**: each LLM request carries the guest it is for (a tokio task
  local), so its cost counts against what that guest may spend. Requests a
  guest's deed sets off later (a new kind of thing to write) are the host's.
- **Talk between players**: `/say` (not in the first design).
- **The internet** is opt-in: `I` on the host screen (or `pocket host
  --ngrok`); plain hosting is the local network only.

Not yet:

- Guests can't drive vehicles or ride (their deeds go to the host, but
  steering is the host's own keys).
- Notes about places far from the host but near a guest are told as the host
  hears them (near the host).
- A guest's own creations are recorded as made by "the traveler".
- Guests earn no achievements; nobody can be sent away by the host yet.
- The ngrok tunnel was tested against its log format, not live.
