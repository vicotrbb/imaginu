#!/usr/bin/env bash
# Regenerate every gallery recipe into a temp dir and hash it, so we can prove
# prior kinds stay byte-identical across the whole boss phase. Dungeons write a
# directory; hash the whole tree deterministically (sorted).
set -euo pipefail
BIN="${BIN:-target/release/imaginu}"
OUT="${1:-/tmp/imaginu_baseline.sha256}"
cargo build --release
: > "$OUT"
for f in gallery/recipes/*.json; do
  name="$(basename "$f" .json)"
  if grep -q '"kind"[[:space:]]*:[[:space:]]*"dungeon"' "$f"; then
    d="/tmp/base_$name"; rm -rf "$d"
    "$BIN" dungeon "$f" -o "$d" >/dev/null
    find "$d" -type f | sort | xargs shasum -a 256 | sed "s#$d#dungeon:$name#" >> "$OUT"
  else
    "$BIN" generate "$f" -o "/tmp/base_$name.glb" >/dev/null
    shasum -a 256 "/tmp/base_$name.glb" | sed "s#/tmp/base_$name.glb#$name#" >> "$OUT"
  fi
done
sort -o "$OUT" "$OUT"
echo "wrote $(wc -l < "$OUT") hashes to $OUT"
