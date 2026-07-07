# imaginu Phase 7 — Bosses: a state-of-the-art `boss` recipe

> **Mission.** Add one new first-class recipe kind to imaginu: **`boss`** — large,
> multi-part, multi-phase encounter creatures with signature telegraphed attacks,
> destructible/weak-point gameplay metadata, and first-class placement into both
> **dungeons** (boss rooms) and **terrains/worlds** (world-boss POIs). A boss must
> read *instantly* as the centerpiece of a fight, not just a big monster: presence,
> silhouette, menace, and readable weak points. Ship it as **v0.3.0**, land it on
> `main` behind the same green bar as every prior phase. Work autonomously; only
> stop for the decisions flagged at the bottom.

## How to run this

1. **Brainstorm first.** Use `superpowers:brainstorming` to lock the design —
   resolve the open decisions below, sketch the recipe schema, pick the boss
   archetypes and the phase/weak-point model for v1. Do NOT skip to code.
2. **Write the plan** with `superpowers:writing-plans` (bite-sized, verifiable
   steps), then **execute it** with `superpowers:subagent-driven-development`
   (fresh implementer per task + per-core adversarial review + a final
   whole-branch review — this is how phase 6 shipped clean).
3. Use `superpowers:test-driven-development` for the generators and
   `superpowers:verification-before-completion` before any "done" claim.
4. **Render and LOOK at every visual change yourself** — delegated code passes
   tests but can look wrong. Score against `docs/EVALUATION.md`.

---

## Where the repo is right now (don't rediscover this)

- **Shipped:** **v0.2.0 is live** — crates.io (0.1.0 published; 0.2.0 packaged and
  tagged), a GitHub Release with 5 prebuilt targets, and a Pages site
  (`docs/site/`) with a live Babylon viewer. CI (fmt/clippy/test + determinism on
  Linux & macOS), `release.yml` (tag `v*`), `pages.yml` all green. Bumping to
  **v0.3.0** and cutting a tag is how this phase ships. **CI runs on PRs / pushes
  to `main`** (open a PR to get it green before merge).
- **Library shape:** pure Rust lib (`src/lib.rs`) + CLI (`src/main.rs`).
  `imaginu::compile` / `compile_to_glb` return `Result<_, imaginu::Error>`.
  MSRV 1.87, edition 2024 (**`gen` is a reserved keyword — the module is
  `generators`**).
- **The recipe pipeline** (the spine you extend):
  - `src/recipe.rs` — the `Recipe` enum (`Terrain|Tree|Rock|Crystal|Building|
    Prop|Character|Monster|Dungeon|Custom|World`), one `*Params` struct per kind,
    `Recipe::parse`, and `Recipe::build()` which validates the palette then
    dispatches to `generators::<kind>::generate(params, &pal)`. A class/theme's
    preferred palette is substituted in `build()` only when the recipe left the
    default `verdant` (see `preferred_palette`/`theme_palette`).
  - `src/generators/*` — one module per kind; `generators/mod.rs` re-exports them
    and holds `rng(seed)` (ChaCha8) + `range`.
  - `src/main.rs` — the CLI subcommands and the **`SCHEMA_HELP` cheat-sheet**
    (the agent contract, `main.rs` ~line 580+; the palette line must list all 9).
- **The system you extend (phase 6 — a boss is a super-monster):**
  - `src/generators/monster/rig.rs` — `MonsterRig` = `Skeleton` + a list of
    fold-order-ranked `PrimitiveDesc` (`RoundCone|Ellipsoid`, with optional
    anisotropic `radii` for flat sheets) + `GaitDesc`. Eight `plan_*` body-plan
    builders + `apply_knobs` (horns/spikes/plates/eyes/maw/…) — **all knobs
    fold-ranked AFTER limbs as their own rigid skin families so they never web.**
  - `src/generators/monster/body.rs` — `organic_field` (smooth-min compose in
    fold-rank order), `build_body` (surface-net mesh via `sdf::mesh_field`,
    palette color + emissive accent), `fit_collider` (per-plan collider), and
    `eval_prim`.
  - `src/generators/monster/mod.rs` — `skin_body` (**family classification by
    primitive SDF, not bone-segment distance** — the belly-vs-buried-leg-bone
    lesson) with a continuous junction blend (branch cutoff half-width == the
    smoothstep width, or you get a weight discontinuity).
  - `src/generators/monster/anim.rs` — `build_clips`: procedural
    `idle`/locomotion(`walk|slither|fly|crawl|pulse`)/`attack`/`hurt`/`death`
    (+`roar` when the plan has a head), driven by the `GaitDesc`.
  - `src/generators/monster/preset.rs` — `apply_preset`: `class` fills only unset
    sentinel fields (explicit wins), then normalizes.
- **The homes a boss must fit (phase 4 + 6):**
  - `src/generators/dungeon/*` — `DungeonModel` already emits `SpawnPoint{kind:
    Boss, pos}` and a `Boss` room kind, and a `manifest.json` (`imaginu-dungeon/1`)
    with `rooms/corridors/doors/spawn_points`. A boss should drop into the boss
    room; the manifest should be able to reference a boss asset by file (like the
    world `Poi{file}` does).
  - `src/world/*` — the chunk + `manifest.json` streaming map; `world/poi.rs`
    already has a `Dungeon` POI kind and a POI solver placing cities/dungeons with
    `spawn_points`. A world-boss is a natural new POI kind referencing a boss GLB.
- **Primitives to reuse (do NOT reinvent):** `src/sdf.rs` (round cones /
  ellipsoids / smooth-min / surface nets), `src/skinning.rs` (family-restricted
  multi-joint binding + `smooth_bind`), `src/anim.rs` (clip system: `pose_at`,
  `pose_asset`, `clip_duration`), `src/csg.rs` (subtract/union/intersect —
  **cutters must be closed solids**), `src/mesh.rs`/`subdiv.rs`/`noise.rs`/
  `uv.rs`/`palette.rs`/`texture.rs`.
- **Physics contract:** every GLB embeds a collider at
  `nodes[0].extras.imaginu_physics = {collider, mass, friction, restitution}`.
  Colliders: `Box|Sphere|Capsule|TriMesh|Heightfield` (`src/gltf.rs`).
- **Quality tooling:** `gallery/regen.sh` (+ `regen_showcase.sh`) regenerates
  reference GLB/PNG/MP4 from `gallery/recipes/` (dungeons are special-cased to use
  the ceiling-less overview as the preview). `imaginu validate` / `validate-world`
  / `validate-dungeon` do byte-level structural checks. `docs/EVALUATION.md` holds
  the 6-point visual rubric.

## Non-negotiables (carry these from every prior phase)

1. **Determinism is sacred.** Same recipe + seed → byte-identical GLB across
   processes and platforms. Capture a baseline hash of all existing kinds before
   you start; re-verify byte-identical after (all prior kinds unchanged). Beware
   the macOS-ARM float heisenbug (the `texture.rs` f64 + `black_box` guard); the
   CI determinism job guards it. No process/time/address state in generation.
2. **Render and look.** Every generator change is verified by rendering
   (`--preview`, `render --animation <clip>`, and for arena placement the dungeon
   `--overview`) and *viewing* against the 6-point rubric. Score ≥4/5 on every
   axis before "done." A boss must clear a **higher** presence bar than a monster.
3. **DRY against `imaginu schema`.** Update the cheat-sheet for every new field.
4. **No regressions.** `cargo fmt --check`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test`, `cargo doc --no-deps` all clean; the gallery still
   regenerates; existing kinds unchanged byte-for-byte.
5. **Reuse the monster engine, don't fork it.** A boss is a *composition and
   escalation* of the monster rig/body/skin/anim pipeline, not a second engine.
   Refactor shared monster internals to `pub(crate)` where the boss needs them
   rather than copy-pasting (watch fold-order = skinning correctness).

---

## Workstream B — the `boss` recipe

**Goal:** `imaginu generate '{"kind":"boss","archetype":"hydra","element":"infernal"}'
-o boss.glb --preview` yields a rigged, animated, multi-part, collider-bearing
encounter boss that reads instantly as a fight centerpiece, with per-part weak-
point colliders and phase/ability metadata in `extras`.

**What makes a boss ≠ a monster (design the escalation):**
- **Scale & presence.** Much larger than a monster; deliberate silhouette,
  heavier proportions, hero-tier tessellation by default. `size` defaults high;
  mass scales accordingly.
- **Multi-part / composite bodies.** A boss is often more than one organic body:
  a **hydra** (one torso, N necks+heads), a **golem/colossus** (chunky stone
  segments, exposed core), a **lich/overlord** (humanoid + throne/pedestal +
  floating implements), a **swarm-queen** (huge insectoid + brood sacs), a
  **dragon/wyrm-lord** (winged serpent at scale). Compose several `MonsterRig`-
  style sub-rigs into one asset with a **named part hierarchy** (glTF node names)
  so the game can target parts.
- **Phases.** A boss changes over a fight. Decide (open decision C) whether to
  bake **phase variants** into one asset — phase-transition clips + per-phase
  emissive/armor state (e.g. `phase2` sheds plates, exposes a glowing core) —
  and/or expose a `phase` param that generates each state.
- **Signature telegraphed attacks.** Richer than the monster clip set: at minimum
  `idle`, `telegraph` (wind-up the player reads), a signature `slam`/`breath`/
  `summon`, `phase_transition`, `stagger`/`hurt`, `enrage`, `death`. Telegraph
  clips + emissive cues are gameplay, not decoration.
- **Weak points & destructible parts.** Named sub-node colliders (e.g.
  `weak_point.core`, `head.3`) plus `extras.imaginu_boss` metadata listing weak
  points, destructible parts, and per-phase ability timings — the payload that
  makes it a *fight*, mirroring how the dungeon manifest carries spawn/nav data.
- **Attachments & regalia.** Crown, weapon, pauldrons, throne/pedestal, brood
  sacs, chains — reuse `prop.rs`/`csg.rs` and the monster knob system.
- **Element / theme.** `element` (e.g. `infernal|necrotic|fungal|arctic|
  volcanic|…`) drives palette + emissive telegraph color; consider dedicated boss
  palettes if the existing nine don't sell it (open decision D).

**Schema sketch (`BossParams`, serde-defaulted like `MonsterParams`):**
`seed`, `archetype` (the ~5 composite templates), `element`/`palette`,
`size`/`bulk` (default large), `phases` (u32, default 2) or `phase` selector,
`weak_points` (bool/count), `armor`/`plates`, `crown`/`regalia`, feature knobs
reused from monster (`horns/spikes/eyes/maw/wings/tail/emissive`), `detail`
(default hero), `animate`. Provide a `class`-style **preset** per archetype.

**Quality bar:** each v1 archetype renders convincingly from 4 angles, animates
its full clip set without deformation artifacts (verify with the stretched-
triangle edge-length probe — measure, never guess blend params), reads as a boss
at a glance, and exposes sane weak-point colliders. Add ≥2 hero bosses to the
gallery with showcase MP4s.

## Workstream A — arena integration (this is what makes it usable)

A boss that can't be *placed* is a statue. Wire it into the two homes:
- **Dungeons.** The `DungeonModel` already yields a `Boss` spawn point and a boss
  room. Let a dungeon reference/emit a boss for its boss room (a `boss` field on
  the dungeon recipe, or a manifest `boss` entry pointing at a boss GLB like the
  world `Poi{file}`). Scale the boss to the room; place it at the boss spawn.
  Extend `validate-dungeon` to check the boss reference.
- **Terrains / worlds.** Add a **world-boss POI** (new `world/poi.rs` kind) that
  the POI solver can place, referencing a boss GLB and carrying its spawn/arena
  metadata in the world `manifest.json`, seamless with the chunk streaming.
- Keep the **seam law**: any placement geometry snaps to the same integer-meter /
  world-coord rules the dungeon/world use so edges stay f32-exact.

---

## Integration checklist (complete ALL of this)

- [ ] `recipe.rs`: `BossParams` struct + `Boss` variant + `build()` dispatch +
      palette/element validation + preferred-palette substitution. Sensible
      defaults for every field.
- [ ] `generators/mod.rs`: `pub mod boss;`
- [ ] `generators/boss/*`: composite rig + body + skin + boss clip driver + weak-
      point collider extraction + `extras.imaginu_boss` metadata writer. Reuse
      `generators::monster::*` internals (promote to `pub(crate)` as needed).
- [ ] `gltf.rs`: if bosses need per-part named colliders / multiple colliders,
      extend the asset/extras writer (mind determinism + the existing single
      `imaginu_physics` contract; keep it backward compatible).
- [ ] `main.rs`: extend the `SCHEMA_HELP` cheat-sheet; add any `boss` /
      `validate-boss` subcommand only if a directory form is chosen.
- [ ] Dungeon + world integration: boss reference in the dungeon manifest and a
      world-boss POI; extend `validate-dungeon` / `validate-world` accordingly.
- [ ] Tests: parse+build per archetype and preset; **determinism (byte-identical
      twice)**; `validate`(+arena validators) clean; hostile-input → clamps, no
      panic; the stretched-triangle skinning probe per archetype; weak-point /
      metadata round-trip.
- [ ] `gallery/recipes/` + `regen.sh` (+ showcases) for ≥2 hero bosses.
- [ ] `skill/imaginu/SKILL.md`: note the new kind (schema stays the reference).
- [ ] `README.md` recipe gallery + `docs/site/` (viewer model, recipe rows,
      gallery grid) get a hero boss. **No em-dashes in docs/site.**
- [ ] `CHANGELOG.md`: `Unreleased` → the new features; bump to **v0.3.0**.
- [ ] Ship: PR green on CI, merge, tag `v0.3.0` (release + pages redeploy;
      crates.io publish is a local `cargo publish` — the CI token is unset by
      design and the job skips gracefully).

## Definition of done

- `boss` is a first-class kind: documented in `imaginu schema`, covered by tests,
  present in the gallery, shown on the site and in the skill.
- An agent can go from "make me an infernal hydra boss for my crypt" to a
  loadable, correctly-collided, weak-point-tagged asset placed in a dungeon boss
  room or a world POI, using only the skill + the binary.
- v0.3.0 is tagged and green; determinism and all prior kinds are unregressed.

## Known traps (from prior phases — internalize these)

- **Smooth-min fold order IS skinning correctness** — compose core→limbs→
  attachments by rank; classify skin families by primitive SDF (not bone-segment
  distance); keep the junction-blend smoothstep width equal to the branch-cutoff
  half-width. Debug webs with the posed-vs-bind edge-length probe, never by
  guessing blend params. Composite multi-part bosses multiply this risk — fuse
  each sub-body in a deliberate order and keep parts as their own skin families.
- **CSG cutters must be closed solids** (profiles touch the axis/base) or carves
  silently fail.
- **Determinism heisenbug on macOS ARM** — keep the `texture.rs` f64 + `black_box`
  guard; never introduce process/time/address/HashMap-order state into generation
  (use `BTreeSet`/sorted iteration and `rng(seed)` only).
- **Surface-nets quad winding must match rasterizer culling**; flat shading
  averages face colors, so bit-exact seam tests compare the pre-flat vertex grid.
- **Rasterizer near/far auto-fits** — don't hard-code a far plane for a huge boss.
- **`clap` eats a leading `-`** — pass `--flag=value` for negative args.
- Roofed/enclosed geometry hides interiors from a turntable — bosses are open, so
  fine, but if a boss sits in an arena shell, render it ceiling-less (see the
  dungeon `--overview`).
- Keep `gallery/`, `docs/`, `skill/`, media OUT of the crate (`Cargo.toml`
  `exclude`); the published crate must stay small.

## Open decisions (resolve in brainstorming; reasonable defaults noted)

- **A. Kind vs. flag.** A distinct `kind:"boss"` (richer schema + gameplay
  metadata, its own generator) vs. a `tier:"boss"` flag on `monster`. Default:
  **distinct `boss` kind** that internally reuses the monster engine.
- **B. Archetype set for v1.** Which ~5 composite templates (hydra, golem/
  colossus, lich/overlord, swarm-queen, dragon/wyrm-lord?), and whether to add an
  archetype **preset** layer over the raw knobs (like monster `class`).
- **C. Phase model.** Bake phase variants + phase-transition clips into ONE asset
  vs. a `phase` param generating each state vs. both. Default: **bake 2 phases +
  a `phase_transition` clip**, expose an optional `phase` override.
- **D. Boss palettes.** Do bosses need dedicated palettes/telegraph colors, or do
  the existing nine (incl. necrotic/infernal/fungal) suffice? Default: **reuse the
  nine, driven by `element`**; add only if a read is missing.
- **E. Weak-point / metadata format.** Named per-part node colliders + an
  `extras.imaginu_boss` block (weak_points, destructible parts, phase/ability
  timings, arena spawn) vs. a leaner scheme. Default: **named colliders +
  `extras.imaginu_boss`**, mirroring the world/dungeon manifest payload idea.
- **F. Arena coupling.** Should the dungeon recipe optionally *emit* its boss
  (a `boss` field), or only reference one by file in the manifest? Default:
  **both — reference by file in the manifest, with an optional inline `boss`
  field that generates + places it.**
- **G. Release cadence.** Ship as **v0.3.0** on its own (recommended), or bundle
  with other work.
