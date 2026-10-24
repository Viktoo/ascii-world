# The Infinite Pocket logo

`infinite-pocket-wide.jpg` (3840×2160), `infinite-pocket-banner.jpg` (its
middle half, 3840×1080, the README's header) and `infinite-pocket-square.png`
(2048×2048) are one in-game render, with nothing painted on afterwards:
"INFINITE" over "POCKET" in solid 3D letters in a meadow of Maple Cove
(`worlds/starter`) at 16:54. Owen Fairbanks, the orchardist, stands beside
the T in a straw hat with a basket of apples, and Pumpkin the cat walks left
to right under the CK.

## Get back to it

From the repo root, after `cargo build --release` (needs `sqlite3` and
ImageMagick's `magick`):

```bash
logo/build.sh
```

```bash
logo/render.sh
```

`build.sh` unpacks the starter world into `logo/scene.pocket` (not
committed) and applies every change as SQL; `render.sh` takes the shot and
writes both images. Run as is, they give back these images pixel for pixel.

To walk round the scene: `./target/release/pocket logo/scene.pocket`.

## What is where

- `types/letters.js`: the title, written by `letters.py` (glyph shapes,
  line width 11 m: `python3 logo/letters.py`). Both words are scaled to
  the same width.
- `types/farmer-figure.js`: Owen's body for this shot only, posed and
  dressed in its own code (the game's people use the built-in `figure`).
- `types/sugar-maple.js`, `types/autumn-wildflowers.js`: the maple and
  flowers as they were for this shot. The starter now ships nearly the same
  (its flowers also have a low tuft of leaves at the foot), so `build.sh`
  swaps these back in to keep the shot exact.
- `build.sh`: where everything stands. The letters at (167, 0) face west;
  Owen at (165.6, −6.2); Pumpkin at (163.5, −0.45) facing south (yaw π).
- `render.sh`: the camera: `--at 150,0.2,90 --lift 2 --pitch -5.5
  --time 16.9`, and `--phase` to catch the cat mid-stride.

## Tweaking

- Screen left is +z, screen right is −z; forward (away from the camera)
  is +x. A character's yaw faces it along (sin yaw, cos yaw): −π/2 looks
  at the camera, π walks to screen right.
- To move someone, change their numbers in `build.sh` and run both
  scripts again. For a quick look, render small: `--size 480x135`.
- After changing a type, check it with `./target/release/pocket check FILE`.
  A type whose stored meta is `{}` gets its bottom and top probed on load.
  A plant made of thin stems can probe wrong and sink into the ground: give
  it `{"bottom":…,"top":…}` by hand, as `build.sh` does for the flowers.
- Plants scattered by the world can't be moved one by one. Any placed
  thing keeps scatter out of its footprint (its bounds plus 1.5 m).
- The scene is built on `worlds/starter/starter.pocket.gz`. If the starter
  is remade, the people and places may move: check the shot and adjust
  `build.sh`.
- The shot depends on the renderer's soft shadow sharpness (40 in
  `soft_shadow`, `src/shaders/scene.wgsl`). At the old 10, every flower
  clump threw a shadow the size of a bush.
