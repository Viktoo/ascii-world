#!/bin/sh
# Renders the logo from logo/scene.pocket (see build.sh) into logo/:
# infinite-pocket-wide.jpg (3840x2160), infinite-pocket-banner.jpg (its middle
# half, 3840x1080, the README's header) and infinite-pocket-square.png (2048x2048).
# Run from the repo root after `cargo build --release`. Needs ImageMagick.
set -e
cd "$(dirname "$0")/.."
CAT=$(sqlite3 logo/scene.pocket "SELECT id FROM characters WHERE json_extract(persona_json, '\$.name') = 'Pumpkin'")
TMP=$(mktemp -d)
# Camera at (150, 0.2) facing east (90°), 2 m above eye height, tipped 5.5°
# down, at 16:54; the cat caught mid-stride.
./target/release/pocket snapshot logo/scene.pocket --at 150,0.2,90 --size 960x270 \
  --time 16.9 --lift 2 --pitch -5.5 --phase "$CAT=1.0" --png "$TMP/shot.png"
magick "$TMP/shot.png" -quality 92 logo/infinite-pocket-wide.jpg
magick "$TMP/shot.png" -gravity center -crop 3840x1080+0+0 +repage -quality 92 logo/infinite-pocket-banner.jpg
magick "$TMP/shot.png" -crop 2160x2160+840+0 +repage -resize 2048x2048 logo/infinite-pocket-square.png
rm -r "$TMP"
echo "wrote logo/infinite-pocket-wide.jpg, logo/infinite-pocket-banner.jpg, logo/infinite-pocket-square.png"
