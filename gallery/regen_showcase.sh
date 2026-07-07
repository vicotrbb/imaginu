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
"$BIN" showcase examples/elder_sage.json -o gallery/showcase_elder_sage.mp4 --duration 6
"$BIN" showcase examples/elder_sage.json --animation walk -o gallery/showcase_elder_walk.mp4 --duration 4
"$BIN" showcase gallery/recipes/char_hedge_mage.json --animation walk -o gallery/showcase_hedge_mage_walk.mp4 --duration 4
"$BIN" showcase gallery/recipes/fire_wyrm.json --animation slither -o gallery/showcase_fire_wyrm.mp4 --duration 5
"$BIN" showcase gallery/recipes/cave_spider.json --animation crawl -o gallery/showcase_cave_spider.mp4 --duration 4
"$BIN" showcase gallery/recipes/ogre_brute.json --animation attack -o gallery/showcase_ogre_brute.mp4 --duration 4
"$BIN" showcase gallery/recipes/void_horror.json --animation pulse -o gallery/showcase_void_horror.mp4 --duration 4
"$BIN" showcase gallery/recipes/infernal_hydra.json -o gallery/showcase_infernal_hydra.mp4 --duration 5
"$BIN" showcase gallery/recipes/necrotic_lich.json -o gallery/showcase_necrotic_lich.mp4 --duration 5
