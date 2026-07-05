#!/bin/sh
# Loop-perfect showcase MP4s (requires ffmpeg).
set -e
BIN=${BIN:-target/release/imaginu}
"$BIN" showcase gallery/recipes/terrain_island.json -o gallery/showcase_island.mp4 --duration 6
"$BIN" showcase gallery/recipes/crystal.json -o gallery/showcase_crystal.mp4
"$BIN" showcase examples/windmill.json -o gallery/showcase_windmill.mp4
"$BIN" showcase examples/tavern.json -o gallery/showcase_tavern.mp4 --duration 6
"$BIN" showcase gallery/recipes/terrain_river_valley.json -o gallery/showcase_river_valley.mp4 --duration 6
"$BIN" showcase gallery/recipes/char_mage.json --animation dance -o gallery/showcase_dance.mp4 --duration 4
