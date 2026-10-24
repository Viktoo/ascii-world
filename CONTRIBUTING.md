# Contributing

## Think in primitives

The goal is emergence: small general parts that combine, so interesting things happen.

- Add primitives, not features. Ask what general part would let the AI make the
  thing, and many others.
- Don't overfit. No special cases for one world, prompt or creature.
- New primitives must combine with the existing ones. No code that names
  specific kinds.
- Put details in data the AI writes, not in the engine.

## Before a PR

- `cargo test` after each change.
- `cargo test --release --features slow-tests` before opening the PR.
- Keep tests short: one world, simulate only as long as needed. Mark long ones
  `#[cfg_attr(not(feature = "slow-tests"), ignore)]`.

## World builds

We also take worlds rich in detail and life, as starter worlds or ones that come
with the game for new players. For example:

- Real places (Santa Monica pier, a famous landmark)
- A vibe or feeling (a peaceful Japanese farming village)
- Future and other planets (a Martian colony, a cyberpunk city)
- Settlements with their own characters (a simulated wedding)

To share one, zip its `.pocket` file and post it in a
[GitHub issue](https://github.com/Viktoo/infinite-pocket/issues/new) with
screenshots or videos and what to look out for.
