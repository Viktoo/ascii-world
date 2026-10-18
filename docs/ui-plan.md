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
       ──► Title (a showcase world turning behind it, seen from above the trees)
            ├─ Continue  «last world»         → loading → play
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

Not the player's last world: one showcase world, the same for everyone, so a
first-time player sees it too. It is made once (`~/.pocket/showcase.pocket`,
seed 7) from the game's own land and pieces, with no AI. The camera stands on
a hilltop picked by `panorama::vista`, 14 m up, at 4:30 pm, and turns once in
about two minutes. Until it loads (or with no GPU) the hills of words show.
`pocket showcase` prints where it stands, for `pocket snapshot … --lift 14`.

## Not done yet

- A showcase world written by hand (a lake, a village) instead of seed 7.
- Icons on tabs.
- Rebinding keys.
