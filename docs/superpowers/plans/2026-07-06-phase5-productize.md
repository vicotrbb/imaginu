# imaginu Phase 5 — Productize: ship it as a real tool people use

> **Mission.** Turn imaginu from a private repo into a polished, installable,
> open-source product with a beautiful showcase site, out-of-the-box binaries,
> and a first-class AI-agent skill. Four workstreams, each landing as its own
> commit(s) on a `phase-5` branch, merged to `main` when the whole thing is
> green. Work autonomously; only stop for the handful of decisions flagged at
> the bottom.

---

## Where the repo is right now (don't rediscover this)

- **Remote:** `https://github.com/vicotrbb/imaginu.git` (owner `vicotrbb`, `gh`
  authed). Branch `main`.
- **Code:** ~14k lines of Rust, clean modular layout (`src/generators/*`,
  `src/world/*`, `sdf.rs`, `mesh.rs`, `gltf.rs`, `render.rs`, `recipe.rs`,
  `texture.rs`, …). Library (`lib.rs`) + CLI (`main.rs`). 56 tests pass.
- **Zero C dependencies.** Deps are `glam, serde, serde_json, clap, png, rand,
  rand_chacha` — all pure Rust. Static cross-compilation (incl. musl) is trivial;
  no `cc`/`openssl`/`*-sys` in the tree. THIS is the distribution superpower.
- **CLI surface:** `generate`, `render`, `showcase`, `schema`, `validate`,
  `world`, `validate-world`. `imaginu schema` already prints the full recipe
  cheat-sheet — the AI-agent contract already exists in-tree.
- **External runtime dep:** `ffmpeg` on PATH, needed ONLY by `showcase` and the
  world `--flyover`. Everything else is self-contained. Must be documented as
  optional.
- **Ready-made content:** `gallery/` holds `.glb` + `.png` + loop-perfect `.mp4`
  for characters, terrains, props, worlds (Ravenspire/Everdale maps + flyovers).
  This is the website's raw material.
- **Missing entirely:** `README.md`, `LICENSE` file (Cargo.toml declares MIT),
  `.github/` (no CI, no release), `CONTRIBUTING`, `CHANGELOG`, crates.io
  metadata, any website, any packaged skill.
- **Quality debt:** 27 `clippy` lints (all trivial: `manual_clamp`,
  `needless_range_loop`, `too_many_arguments`, `approx_constant`, …); 154
  `unwrap()/expect()/panic!` occurrences (fine inside the CLI on infallible
  invariants; NOT fine on the public library API).

## Non-negotiables (carry these from every prior phase)

1. **Determinism is sacred.** Same recipe + seed → byte-identical GLB, across
   processes and platforms. There is a documented macOS-ARM float heisenbug in
   the history (fixed with f64 gradients + `black_box`). CI MUST include a
   determinism check (generate twice, diff bytes). Never regress this.
2. **Visual quality loop still governs asset code.** Any change touching
   generators must be verified by rendering and *looking* against the 6-point
   rubric in `docs/EVALUATION.md`. Don't claim quality you haven't viewed.
3. **No functional regressions.** `cargo test` stays green; the gallery still
   regenerates from `gallery/regen.sh`; `validate` / `validate-world` stay clean.
4. **Dogfood.** The tool targets Babylon.js — the website MUST render real
   gallery `.glb`s in an actual Babylon viewer. Proving the output loads in the
   engine it's built for is the whole pitch.
5. **Each workstream is its own reviewable commit(s)** with a clear message.

---

## Workstream A — OSS-grade hardening (do this FIRST; everything else builds on it)

**Goal:** a newcomer (human or agent) lands on the repo and immediately trusts
it and knows how to use it.

**Deliverables**

- **`README.md`** — the centerpiece. Sections: one-line pitch + hero
  image/GIF (pull from `gallery/`); "what & why" (AI-drivable procedural GLB
  compiler for Babylon.js; vertex-color PBR, deterministic, no textures);
  install (release binary, `cargo install`, build-from-source); 60-second
  quickstart (`imaginu generate '{"kind":"tree","style":"oak"}' -o tree.glb
  --preview`); a recipe gallery with embedded preview images; the AI-agent story
  (link the skill); determinism / seam-law note; ffmpeg-optional note;
  architecture overview (module map); link to the live site; contributing +
  license badges. Keep it visual — embed real gallery renders.
- **`LICENSE`** — MIT, author "Victor Bona" (confirm year 2026). Make Cargo.toml
  and the file agree.
- **`CONTRIBUTING.md`** — dev setup, `cargo test`/`clippy`/`fmt`, the
  render-and-look rubric expectation, determinism rule, how to regen the gallery.
- **`CHANGELOG.md`** — Keep-a-Changelog format; seed with Phases 1–5 summary and
  an `Unreleased`→`v0.1.0` section for the first tagged release.
- **Cargo.toml metadata** — add `authors`, `repository`, `homepage`,
  `documentation`, `readme`, `keywords` (gltf, procedural, gamedev, babylonjs,
  ai), `categories` (game-development, graphics, command-line-utilities),
  `rust-version`. Make it crates-io-publishable (decision C below).
- **Clean `cargo clippy --all-targets -- -D warnings`** and
  `cargo fmt --check`. Fix all 27 lints; keep changes mechanical/no-behavior.
- **Library-boundary panic audit.** Public `lib.rs` API must return `Result`
  (define an `imaginu::Error` if none exists), NOT panic, on malformed recipes
  or IO. CLI-internal `unwrap()`s on true invariants may stay but should be
  spot-checked. Do NOT blanket-rewrite 154 sites — triage: anything reachable
  from a public fn with attacker/agent-controlled JSON input gets a real error.
- **Crate-level + module docs** so `cargo doc` reads well (many modules already
  have good `//!` headers — fill the gaps).

**Quality bar:** `cargo test && cargo clippy -- -D warnings && cargo fmt --check`
all clean; `cargo doc --no-deps` warning-free; README renders correctly on GitHub
with working images.

---

## Workstream B — CI + release pipeline (out-of-the-box binaries)

**Goal:** `curl`-and-run on any machine; no Rust/cargo required.

**Deliverables**

- **`.github/workflows/ci.yml`** (push + PR): `cargo fmt --check`, `cargo clippy
  --all-targets -- -D warnings`, `cargo test --release`, and a **determinism job**
  (generate a fixed recipe twice, assert byte-identical; ideally on both
  `ubuntu-latest` and `macos-latest` to guard the ARM heisenbug).
- **`.github/workflows/release.yml`** (trigger on tag `v*`): cross-compile a
  matrix and attach stripped archives + `SHA256SUMS` to a GitHub Release.
  Because there are no C deps, target the full spread:
  - `x86_64-unknown-linux-musl` (fully static), `aarch64-unknown-linux-musl`
  - `x86_64-apple-darwin`, `aarch64-apple-darwin`
  - `x86_64-pc-windows-msvc`
  Use a maintained action (e.g. `taiki-e/upload-rust-binary-action` or
  `houseabsolute/actions-rust-cross`) — don't hand-roll cross toolchains.
  Archives: `tar.gz` (unix), `zip` (windows), name `imaginu-<version>-<target>`.
- **`install.sh`** — `curl -fsSL .../install.sh | sh`: detect os/arch, download
  the matching release asset, verify checksum, drop `imaginu` on PATH. Linked
  from README + site.
- **`cargo-binstall` metadata** in Cargo.toml (`[package.metadata.binstall]`) so
  `cargo binstall imaginu` just works.
- **Optional:** a Homebrew tap formula and/or `cargo dist` adoption (decision D).
- **Publish to crates.io.** Once metadata is complete and CI is green, publish
  `v0.1.0` so `cargo install imaginu` works from the registry. Verify the crate
  name `imaginu` is available first; add a `CARGO_REGISTRY_TOKEN` repo secret and
  a `cargo publish` step in `release.yml`, gated on the `v*` tag and running only
  after the binary matrix succeeds. README/site install sections must list
  `cargo install imaginu` alongside the binary + binstall paths.
- Document that video features need `ffmpeg`; everything else is standalone.

**Quality bar:** tag a `v0.1.0-rc` on a scratch branch (or use `workflow_dispatch`)
and confirm all matrix targets build and upload; download the linux-musl binary
in a clean container and run `imaginu generate` with no Rust installed.

---

## Workstream C — AI-agent skill (Claude Code + Codex)

**Goal:** an agent in Claude Code or Codex can generate/iterate real 3D assets
with zero prior knowledge, tailored to imaginu's workflow.

**Deliverables**

- A **`skill/` package in-repo** (e.g. `skill/imaginu/SKILL.md` + any refs),
  authored to the Claude Code / Superpowers skill format (YAML frontmatter:
  `name`, `description` written as "Use when …"; body with the workflow).
- **Content the skill must teach:**
  1. Ensure the binary exists (`imaginu --version`; else point to `install.sh`
     or `cargo binstall imaginu`).
  2. The recipe contract — instruct the agent to run `imaginu schema` for the
     authoritative cheat-sheet rather than hardcoding fields (schema evolves).
  3. The core loop: write recipe JSON → `imaginu generate <recipe> -o out.glb
     --preview` → **look at the PNG** → iterate. For characters/worlds, the
     render/animation/world subcommands.
  4. Determinism (seed control) and the "make it look right, then trust it" rule.
  5. Where output goes in a Babylon project (load the GLB; `nodes[0].extras.
     imaginu_physics` for colliders; world manifest `position`/spawn points).
  6. Gotchas worth surfacing: ffmpeg needed only for video; worlds emit a
     directory + manifest; validate with `validate` / `validate-world`.
- **Distribution:** document install for both hosts — Claude Code
  (`~/.claude/skills/` or plugin), Codex (its skills/prompt mechanism). Provide a
  one-command copy step. Cross-link from README and the website (a "Use it with
  your AI agent" section).
- Keep the skill DRY against `imaginu schema` — the skill is the *workflow*, the
  schema command is the *reference*.

**Quality bar:** dry-run the skill mentally end-to-end (install → schema →
generate → preview → iterate) and confirm every command in it actually works
against the current CLI. Verify the frontmatter parses in the target format.

---

## Workstream D — Showcase website (GitHub Pages)

**Goal:** a genuinely beautiful, fast, single-page site that sells the tool and
proves the output is real.

**Deliverables** (`docs/site/` or a `site/` dir → GitHub Pages via Actions)

- **Design:** load the `artifact-design`/`frontend-design` skill first and invest
  real craft. Self-contained, responsive, light+dark aware, fast. A confident
  visual identity (this is a graphics tool — the site should look like one).
- **Sections:**
  1. **Hero** — tagline, one-liner, primary CTAs (Get started / GitHub /
     Install), a striking looping gallery MP4 or rotating GLB.
  2. **Live Babylon viewer** — the hero feature. Embed Babylon.js (CDN is fine
     on Pages, unlike sandboxed artifacts) and load REAL `gallery/*.glb`
     (character, tree, terrain, a world chunk) with a model switcher + orbit
     controls. This dogfoods the exact engine imaginu targets.
  3. **Recipe → asset** — show the JSON recipe beside its rendered result
     (pull matched recipe/preview pairs from `gallery/recipes/` + `gallery/`).
  4. **Gallery grid** — the MP4 showcases (characters, worlds, props) in a
     masonry/grid with the recipe behind each.
  5. **AI-agent** — "give it to Claude Code / Codex," the skill install, a short
     transcript-style demo of an agent making an asset.
  6. **Install / quickstart** — copyable commands (release binary, install.sh,
     binstall, cargo install), ffmpeg note.
  7. Footer — license, repo, docs, changelog.
- **`.github/workflows/pages.yml`** — build/deploy to GitHub Pages on push to
  `main`. Enable Pages in repo settings (decision E — may need the user to flip
  the toggle).
- Copy the needed `gallery/` assets into the site's served dir (keep them small;
  transcode/downsize MP4s if heavy).

**Quality bar:** site loads under a few seconds, the Babylon viewer actually
renders a real GLB and orbits, no horizontal scroll on mobile, all CTAs/links
resolve, works in light and dark.

---

## Ordering & dependencies

1. **A (hardening)** first — README/LICENSE/metadata/clippy/panic-audit are the
   foundation the release and site reference.
2. **B (CI/release)** next — needs clean clippy/tests from A; produces the
   binaries the site + skill link to. Cut the first real `v0.1.0` tag here.
3. **C (skill)** — needs the install story from B and the schema (already exists).
4. **D (website)** last — links to the release (B), the skill (C), and the docs
   (A); pulls gallery content that already exists.

Land each as its own commit(s); open the merge to `main` once CI is green and
the site deploys.

## Definition of done

- `main` has: README with images, LICENSE, CONTRIBUTING, CHANGELOG, crate
  metadata; green CI (fmt/clippy/test/determinism); a working release workflow
  that produced downloadable binaries for all targets under a `v0.1.0` tag; a
  published, live GitHub Pages site with a working Babylon viewer; an installable
  AI-agent skill documented for Claude Code + Codex.
- A clean machine with no Rust can install and run imaginu from the release.
- An agent can go from "make me a 3D oak tree" to a loadable GLB using only the
  skill + the released binary.

## Known traps (from prior phases + this analysis)

- Determinism heisenbug on macOS ARM (float state) — keep the `black_box`/f64
  guard; the CI determinism job exists to catch regressions early.
- Flat shading averages face colors — irrelevant here but don't "fix" it.
- `clap` eats a leading `-` in args (`--flyover=…` form) — matters if the site
  shows CLI examples.
- CSG cutters must be closed solids; rasterizer near/far auto-fits — already
  handled, don't touch.
- ffmpeg is the ONLY non-Rust runtime dep and only for video — never let it creep
  into the core path or the install story.
- Sandbox CSP that blocks CDNs applies to *artifacts*, NOT GitHub Pages — the
  site may use the Babylon CDN freely.

## Open decisions (reasonable defaults chosen; flag before diverging)

- **A. Author identity for LICENSE/Cargo:** default "Victor Bona
  <victor.bona.vb@gmail.com>", year 2026.
- **B. Site location:** default `docs/site/` served by a Pages Action (keeps
  source and site in one repo, no `gh-pages` branch churn).
- **C. crates.io publish:** CONFIRMED — publish `v0.1.0` to crates.io as part of
  the release workstream (`cargo install imaginu` must work). Verify the crate
  name is free first and wire `CARGO_REGISTRY_TOKEN` into `release.yml`.
- **D. Homebrew tap / `cargo dist`:** default skip for v0.1.0 (install.sh +
  binstall cover it); revisit if adoption grows.
- **E. Enabling GitHub Pages** in repo settings and any branch protection may
  require the user to click once — surface it, don't block on it.
