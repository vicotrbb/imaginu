#!/bin/sh
# Regenerate every gallery asset (GLB + PNG preview) from gallery/recipes/,
# then run byte-level validation. Showcase MP4s: see regen_showcase.sh.
set -e
BIN=${BIN:-target/release/imaginu}
for f in gallery/recipes/*.json; do
  name=$(basename "$f" .json)
  "$BIN" generate "$f" -o "gallery/$name.glb" --preview
done
for f in examples/tavern.json examples/archway_bridge.json examples/windmill.json examples/arcane_spire.json examples/elder_sage.json; do
  name=$(basename "$f" .json)
  "$BIN" generate "$f" -o "gallery/$name.glb" --preview
done
exec "$BIN" validate gallery/*.glb
