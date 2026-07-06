# Contributing to imaginu

Thanks for your interest! imaginu is a procedural 3D asset compiler written in
pure Rust. This guide covers the dev loop and the two rules that are
**non-negotiable**: determinism and the render-and-look quality bar.

## Dev setup

You need a recent stable Rust toolchain (edition 2024, `rustc >= 1.87`):

```sh
git clone https://github.com/vicotrbb/imaginu
cd imaginu
cargo build
cargo test
```

`ffmpeg` on your `PATH` is **optional** — it is only needed for video output
(`imaginu showcase` and world `--flyover`). Everything else, including PNG
previews, is self-contained with zero C dependencies.

## Before you open a PR

All four must be clean:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps          # keep it warning-free
```

CI runs exactly these, plus a determinism job (see below), on Linux and macOS.

## Rule 1 — Determinism is sacred

The same recipe and seed **must** produce byte-identical GLB, across processes
and platforms. This is the core guarantee that makes worlds tile seamlessly and
makes agent output reproducible.

- Never introduce process/time/address-dependent state into generation.
- Beware platform float differences (there is a documented macOS-ARM float
  heisenbug in the history, fixed with f64 gradients + `black_box`). If you
  touch math in a hot path, verify byte-identical output before/after.
- Quick local check:

  ```sh
  imaginu generate gallery/recipes/tree_oak.json -o a.glb
  imaginu generate gallery/recipes/tree_oak.json -o b.glb
  cmp a.glb b.glb   # must be silent
  ```

## Rule 2 — Render and look

Any change to a generator must be **verified by rendering and looking** at the
result, scored against the 6-point rubric in [`docs/EVALUATION.md`](docs/EVALUATION.md).
Don't claim a visual quality you haven't viewed.

```sh
imaginu generate '{"kind":"tree","style":"oak"}' -o tree.glb --preview
# open tree.png and actually look at it
```

## Regenerating the gallery

The `gallery/` directory holds the reference GLB/PNG/MP4 used by the README and
the website. Regenerate it with:

```sh
gallery/regen.sh            # GLBs + PNG previews
gallery/regen_showcase.sh   # MP4 showcases (needs ffmpeg)
```

Only commit gallery changes that are an intentional, reviewed visual
improvement — and include a before/after in the PR.

## Commit / PR hygiene

- Keep commits focused; describe *what* and *why*.
- If a change touches generation, state in the PR that you verified determinism
  (byte-identical output) and looked at the render.

## License

By contributing you agree that your contributions are licensed under the
project's [MIT License](LICENSE).
