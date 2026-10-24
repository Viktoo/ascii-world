![Infinite Pocket: 3D letters in an autumn meadow, an orchardist with a basket of apples beside them and a white cat walking past](logo/infinite-pocket-banner.jpg)

# Infinite Pocket

**Infinite Pocket is a creative game powered by an AI harness for creating and interacting with 3D digital worlds.** 

Type what you want and it's there: `/a lighthouse on that hill`, `/a griffin that lets me ride it`, `/rub the stone on the lantern`. The AI writes it as code, on the spot, and it becomes part of the world for good.

The people in it remember you. They have homes, needs, friends and plans. They gossip, carry logs together, play catch, and make things of their own. Dogs fetch, goats bolt from wolves, fire spreads through dry grass and stops at a green tree. At night something comes for you.

Then invite a friend into your world and build it together.

## Install (Mac with Apple Silicon)

```bash
brew install viktoo/tap/pocket
```

```bash
pocket
```

You start in **Maple Cove**, an autumn village that's ready to play. To make new things and talk to people, add an Anthropic API key in **Settings → AI** (or any OpenAI-compatible endpoint). Without a key you can still walk around and explore.

Windows and Linux are coming.

## Play


|                        |                                                  |
| ---------------------- | ------------------------------------------------ |
| `W` `A` `S` `D`, mouse | walk and look (click the view to grab the mouse) |
| `Enter`                | talk to whoever is in front of you               |
| `/`                    | make or do anything, in words                    |
| `e` `g` `f`            | use, pick up, throw                              |
| `Esc`                  | pause, settings, play together                   |




## Multiplayer

Host a world and your friends join it, on the same network or over the internet with ngrok. See [Multiplayer](docs/multiplayer.md).

## Build worlds with a coding agent

Everything in the game is code the AI can read and write, and the game can check its own work. `pocket` takes screenshots, describes what's in view, and runs a world headless, so a coding agent (Claude Code, Codex, Cursor…) can build a place, look at it, and fix it:

```bash
pocket snapshot my.pocket --at 0,0,90 --time 17       # a view from anywhere
pocket my.pocket --shot out.png --after 8             # a real frame of the game
pocket describe my.pocket --at 0,0,90                 # what's in view, as JSON
pocket sim my.pocket --hours 24                       # run a day and see what happened
```

The [full guide](docs/guide.md) has every command, setting and how it all works.

## Build from source

```bash
brew install cmake
```

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

```bash
cargo install --locked --git https://github.com/Viktoo/infinite-pocket
```

Tests and releases: [docs/guide.md](docs/guide.md#tests), [docs/releases.md](docs/releases.md).