# Menus and settings: the plan

Goal: feel like a real game. Run `pocket`, and everything else is buttons in
the window. Mouse first, arrows + Enter + Esc still work everywhere. Few words
on screen. Many players know Minecraft, so the shape follows it.

## How other games do buttons

- Minecraft: one column of wide, same-size text buttons, centred, over a
  slowly turning view of a world. No icons. Settings are grouped buttons that
  open sub-screens ("Video Settings…", "Music & Sounds…").
- Stardew Valley, Terraria: themed frames (wood, stone), icons only on tabs.
- Hades, Valheim, Hollow Knight: plain text menus, theme comes from the font
  and the background, not the buttons.

So: text buttons, themed by shape, colour and font (soft rounded "clay"
pills, warm amber for the main action, Fraunces for titles, Nunito Bold on
buttons). No icons on buttons; the look comes from the moving background.

## Screens

```
pocket ──► (first time, no key) Welcome: what the AI is for, paste a key, it is checked
       ──► Title (Maple Cove behind it, cutting between views)
            ├─ Continue  «last world»         → loading → play
            │  (no worlds yet: Open Maple Cove, Make a new world)
            ├─ Worlds                         → Worlds
            ├─ Play together                  → Together
            ├─ Settings                       → Settings
            └─ Quit

Worlds        list of cards (click to pick, double-click to play)
              [Play] [New world]  ·  [Share] [Delete] [Back]
New world     prompt box · [Random place] · Difficulty ‹normal›
              AI models: World builder / Characters / Everyday choices / Memories (dropdowns, saved to settings)
              [Create world] [Back]
Together      games heard on this network (cards)
              [Join] [Join by address…] [Host a world…] [Back]

In a world, Esc → Paused
              [Back to game]
              [Achievements] [Creations]
              [World options]   (difficulty, creatures, life speed, hunting, new beings)
              [Settings]
              [Open to friends] → on this network / for anyone (ngrok); hosting starts
                                  at once, no reload
              [Invite friends]  (while hosting: address, code, who's here, copy, stop)
              [Save and quit to title]

Settings (same screen from the title and from Paused), tabs:
  Video     detail, smooth frames, shadows, frame-rate cap, view distance
  Sound     sound, volume, voices, voice volume, speak to talk
  AI        provider, API key, model per job, budget, spent (+ reset, details)
  Together  your name, what each guest may spend
  General   show work in progress, key hints, tell me about new releases
  Controls  the keys (read only)
```

## Rules

- Every action on a menu screen is a button. Keys stay for play (WASD,
  e, g, /…) and as quiet shortcuts in menus, not printed in hint bars.
- One small hint line at most per screen.
- An environment variable still wins for its run; its row says "set by env".
- The API key and models live in settings.json, so `pocket` alone is enough.

## The world behind the title

Maple Cove, the starter world (`src/starter.rs`, made from `worlds/starter`),
the same for everyone, so a first-time player sees it too. `panorama` cuts
between the views in `worlds/starter/shots.json` (each as `pocket snapshot`
takes it: `at` x,z,yaw, `lift`, `pitch`, `hour`, and a slow `turn`), 12 s
each with a fade through dark, while its people go about their day. It is
kept on behind the next world's loading screen (where the map would be) and
let go once that world is stepped into. Until it loads (or with no GPU) the
stock backdrop shows (`ui::cosmos`, also behind opening a world and notices): deep space drawn on the CPU. A soft light in the middle sends out a ring every 3 s (a crisp edge, colour fringes, a glow behind and sparks in its wake); each ring brings 2–4 planets made from templates (Earth, Jupiter, ringed giant, Mars, ice giant, moon, water world) recoloured, resized, spun and tilted, lit from the light, and the next ring takes them away. Stars twinkle and drift as if the view turns. `POCKET_NO_PANORAMA=1` shows only it. See docs/starter-world-plan.md for its lifecycle.

## Not done yet

- Icons on tabs.
- Rebinding keys.
