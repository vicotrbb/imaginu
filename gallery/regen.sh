#!/bin/sh
# Regenerate every gallery asset (GLB + PNG preview) from gallery/recipes/,
# then run byte-level validation. Showcase MP4s: see regen_showcase.sh.
set -e
BIN=${BIN:-target/release/imaginu}
for f in gallery/recipes/*.json; do
  name=$(basename "$f" .json)
  # Dungeons are navigable spaces: build the loadable single GLB, but use the
  # ceiling-less overview as the readable preview thumbnail (a roofed --preview
  # would just show closed boxes).
  if grep -q '"kind"[[:space:]]*:[[:space:]]*"dungeon"' "$f"; then
    "$BIN" generate "$f" -o "gallery/$name.glb"
    "$BIN" dungeon "$f" -o "/tmp/gallery_$name" --overview
    cp "/tmp/gallery_$name/overview.png" "gallery/$name.png"
  else
    "$BIN" generate "$f" -o "gallery/$name.glb" --preview
  fi
done
for f in examples/tavern.json examples/archway_bridge.json examples/windmill.json examples/arcane_spire.json examples/elder_sage.json; do
  name=$(basename "$f" .json)
  "$BIN" generate "$f" -o "gallery/$name.glb" --preview
done
exec "$BIN" validate gallery/*.glb
