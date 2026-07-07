# imaginu Phase 6 — Monsters & Dungeons design

> Status: approved 2026-07-06. Ships as v0.2.0 on the `phase-6` branch.

Adds two first-class recipe kinds to imaginu: **`monster`** (procedural
creatures across many body plans) and **`dungeon`** (themed, navigable
underground layouts). Both reuse the existing SDF/skinning/animation and
world-manifest machinery rather than introducing a new engine, and land on
`main` behind the same green bar (fmt/clippy/test/doc + determinism) as every
prior phase.

## Resolved decisions

- **A. Dungeon output format:** support **both** — a single GLB for ≤1-room
  dungeons (layout in `extras`), a directory + `manifest.json` for multi-room.
  Add `dungeon` and `validate-dungeon` subcommands mirroring
  `world`/`validate-world`.
- **B. Monster species:** **8 body plans + a `class` preset layer**.
- **C. Palettes:** **add 3** — `necrotic`, `infernal`, `fungal` (existing 6
  untouched).
- **D. Release cadence:** ship **both together as v0.2.0**.

---

## Workstream M — the `monster` recipe

### Architecture — rig-template registry + one shared organic pass

A monster is a **generalization of `character.rs`** past the fixed 17-joint
humanoid, not a new engine. The chosen approach (over per-plan copies of
`character.rs`, which was rejected for 8× duplication and 8× fold-order
debugging):

- Each of the 8 body plans is **data**: a builder returning a `MonsterRig` =
  - a list of joints (a `Skeleton` with bind-pose transforms),
  - a list of **SDF primitive descriptors** (round-cone / ellipsoid segments,
    each with radii, a parent joint, and an explicit **fold-order rank**),
  - a **gait descriptor** (which joints are legs / spine / wings / tail, and
    the locomotion style).
- A **single shared pipeline** consumes that description for every plan:
  `organic_field` composes the primitives under `sdf::smin` **in fold-order
  rank** (core first, limbs by rank, hard-min gate below the core — this is the
  non-negotiable "smooth-min fold order *is* skinning correctness"), meshes via
  `sdf::mesh_field`, skins via `skinning::smooth_bind` (family-restricted so
  limbs bind only to their own segment chain), and animates.
- **Clips are generated procedurally from the skeleton** via a generic
  locomotion driver parameterized by the gait descriptor — not hand-authored 8
  times.

### Schema — `MonsterParams` (serde-defaulted per field, like `CharacterParams`)

| field | type | default | notes |
|---|---|---|---|
| `seed` | u64 | 1 | ChaCha8 seed |
| `body` (alias `species`) | enum `BodyPlan` | `quadruped_beast` | the 8 plans |
| `class` | enum `MonsterClass` | `none` | preset bundle over the knobs |
| `size` (alias `bulk`) | f32 | 1.0 | scales geometry, collider, `mass` |
| `horns` | f32 (0..1) | 0 | composable feature knob |
| `spikes` | f32 (0..1) | 0 | |
| `plates` | f32 (0..1) | 0 | |
| `tail` | f32 (0..1) | plan-default | 0 disables |
| `wings` | f32 (0..1) | plan-default | 0 disables |
| `eyes` | u32 | plan-default | count; emissive glow |
| `maw` | f32 (0..1) | plan-default | jaw/teeth prominence |
| `menace` | f32 (0..1) | 0 | proportion slider |
| `age` | f32 (0..1) | 0 | wear/erosion slider |
| `emissive` | f32 (0..1) | class-default | glow markings |
| `palette` | String | plan-default | any of the 9 validates |
| `detail` | f32 (0.5..2) | 1.0 | tessellation (reuse character knob) |
| `animate` | bool | true | |

**8 body plans** (`BodyPlan`): `biped_brute`, `quadruped_beast`,
`serpent`/`wyrm`, `arachnid`, `winged_flyer`, `ooze`/`blob`, `insectoid`,
`aberration` (tentacled). The plan drives limb count/placement, gait, and
collider shape.

**`MonsterClass` presets** (bundle over the knobs, like character `class`):
`none`, `predator`, `brute`, `elemental`, `undead`, `aberration`, `swarm`.
A preset sets defaults for `emissive`, feature knobs, and a default palette,
which explicit fields still override.

### Animation (procedural from skeleton)

Every monster gets `idle`, a plan-appropriate locomotion clip
(`walk`/`slither`/`fly`/`crawl`/`pulse`), `attack`, `hurt`, `death`; a
`roar`/`telegraph` clip is added where the plan has a head joint. Skinning is
family-restricted. A **stretched-triangle edge-length probe** ships as a test
helper: it compares posed-vs-bind edge lengths so membrane/fused-joint webs are
found by measurement, never by guessing blend params.

### Physics

Auto-fit collider by plan: capsule for biped/quadruped, box for ooze,
elongated capsule for serpent, trimesh fallback for arachnid/aberration.
`mass ∝ size³`, embedded at `nodes[0].extras.imaginu_physics`.

### Quality bar

Each plan renders convincingly from 4 angles and animates without deformation
artifacts. Hero monsters (at least a fire wyrm) get gallery showcase MP4s and
score ≥4/5 on every axis of the `docs/EVALUATION.md` rubric.

---

## Workstream D — the `dungeon` recipe

### Architecture — mirror `WorldModel` + `manifest.json`

- **`DungeonModel::new(&DungeonParams) -> Result<Self, String>`** is a **pure
  function of seed**: validates palette, then computes the abstract layout
  (rooms, corridors, doors, spawn points) before any geometry — the same shape
  as `world/model.rs`.
- **A geometry pass** turns the model into meshes. Output collapses per
  decision A: **≤1 room → single GLB** (layout in `extras`); **multi-room →
  directory + `manifest.json`** (format tag `imaginu-dungeon/1`) with per-room
  GLBs, mirroring `world`'s `ChunkEntry`/`Poi` structs.

### Layout (deterministic)

- **Orthogonal themes** (crypt/mine/temple/fortress/sewer): BSP/grid room
  placement + a corridor graph = **MST of room centers plus a few extra edges
  for loops** (the world road-network idea, scoped to a building).
- **Organic theme** (cavern): **seeded cellular-automata caves**, fixed
  iterations (deterministic).
- **Single level for v1.** Multi-level via stairs is a noted stretch so we hit
  the quality bar on one floor first.

### Geometry

- Floor slabs, extruded walls along layout edges, ceilings. **Doorway/arch
  openings via `csg::subtract`** — cutters are **closed solids** (arch profiles
  touch their base) or the carve silently fails (documented trap).
- **Dimensions rounded to integer meters** so room/corridor edges are
  f32-exact (the seam-law analog).
- Themed wall materials via `texture.rs`. Rasterizer near/far auto-fits (no
  hard-coded far plane for large dungeons).

### Themes — 6 for v1 (`DungeonTheme`)

`crypt`, `cavern`, `sewer`, `mine`, `temple`, `fortress`. Each = palette + wall
material + prop set + shape bias (orthogonal rooms vs. organic caves).

### Dressing

Reuse `prop.rs` (barrel/crate/lantern/campfire) and add dungeon props:
`pillar`, `torch_bracket`, `door`/`portcullis`, `sarcophagus`, `chest`,
`rubble`. **Emissive torches double as lighting cues.**

### Manifest payload

`rooms` (bounds, kind), `doors`, `corridors`, `spawn_points` (player start,
enemy, loot, boss — reusing the world `spawn_points` shape), and colliders
(trimesh per room). Round-trips through `validate-dungeon`.

### Schema — `DungeonParams` (serde-defaulted)

| field | type | default | notes |
|---|---|---|---|
| `seed` | u64 | 1 | |
| `type` | enum `DungeonTheme` | `crypt` | the 6 themes |
| `size` | enum `Small/Medium/Large` | `medium` | target extent |
| `rooms` | Option\<u32\> | none | optional explicit room cap |
| `loops` | f32 (0..1) | 0.3 | extra corridor edges beyond MST |
| `density` | f32 (0..1) | 0.5 | dressing amount |
| `palette` | String | theme-default | any of the 9 validates |
| `detail` | f32 (0.5..2) | 1.0 | tessellation |

### CLI

`dungeon` subcommand (parallels `world`: writes `manifest.json` + per-room
GLBs, optional overview render) and `validate-dungeon` (parallels
`validate-world`).

### Quality bar

Each theme renders as a readable, atmospheric space (overview + a couple
in-room angles); the manifest round-trips through `validate-dungeon`; spawn
points and colliders are sane.

---

## Cross-cutting

### New palettes (additive; existing 6 untouched)

- `necrotic` — bruised greens/greys, sickly emissive accent (undead, crypt).
- `infernal` — charcoal/ember reds, hot emissive accent (fire wyrm, forge).
- `fungal` — damp violets/teals, bioluminescent accent (ooze, cavern).

Slot into `palette.rs`: extend `PALETTES` to 9 and add match arms in
`by_name`. The `Palette` struct already carries `accent` (used as emissive) —
no struct change needed.

### Integration checklist (both kinds complete ALL)

- `recipe.rs`: 2 `*Params` structs + 2 `Recipe` variants + `palette_name`
  arms + `build` dispatch; sensible defaults on every field.
- `generators/mod.rs`: `pub mod monster; pub mod dungeon;`.
- `generators/{monster,dungeon}.rs`: `generate()` — monster → `Asset`,
  dungeon → `Result` (multi-file/manifest writing can fail).
- `main.rs`: extend `SCHEMA_HELP` (main.rs:580-740) with monster + dungeon
  blocks; add `dungeon` + `validate-dungeon` subcommands.
- `validate.rs` + a dungeon manifest validator mirroring
  `world::manifest::validate_dir`.
- Tests: parse+build per kind and per preset/theme; **determinism
  (byte-identical twice)**; `validate`/`validate-dungeon` clean; hostile-input
  → `Err`; monster stretched-triangle skinning probe; dungeon edge-alignment
  check.
- `gallery/recipes/` + `regen.sh`/`regen_showcase.sh` hero assets (fire wyrm,
  a crypt).
- `skill/imaginu/SKILL.md`: note the two new kinds (schema stays the
  reference).
- `README.md` gallery + `docs/site/` (viewer models, recipe rows, gallery
  grid): add a monster and a dungeon. **No em-dashes** in docs/site.
- `CHANGELOG.md`: `Unreleased` → features; bump to **v0.2.0**.

### Determinism (sacred)

- Capture a **baseline GLB hash of all existing kinds before starting**;
  re-verify byte-identical after. Existing kinds must not move a byte.
- New generators are pure functions of `(recipe, seed)` — ChaCha8 via
  `generators::rng`, fixed SDF grid, no process/time/address state. The
  macOS-ARM float heisenbug guard (f64 + `black_box`) only bites the
  `texture.rs` normal-bake path — inherited automatically if we bake normals;
  the geometry path needs no new guard.
- CI determinism job (Linux + macOS) guards it.

### No regressions

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `cargo doc --no-deps` all clean; the gallery still regenerates;
existing kinds unchanged byte-for-byte.

### Ship

Green CI → tag **v0.2.0** (GitHub Release + crates.io publish; publish job
skips gracefully without the token, per phase-5) → Pages site redeploys. Keep
gallery/site/skill media out of the crate via existing `Cargo.toml exclude`.

## Definition of done

- `monster` and `dungeon` are first-class kinds: documented in `imaginu
  schema`, covered by tests, present in the gallery, shown on the site and in
  the skill.
- An agent can go from "make me a fire wyrm" or "make me a crypt dungeon" to a
  loadable, correctly-collided asset using only the skill + the binary.
- v0.2.0 is tagged and green; determinism and all prior kinds are unregressed.

## Known traps carried forward

- Smooth-min **fold order is skinning correctness** — order core→limbs
  deliberately; debug webs with the posed-vs-bind edge-length probe, never by
  guessing blend params. Surface-nets quad winding must match rasterizer
  culling.
- **CSG cutters must be closed solids** (profiles touch the axis/base) or
  carves fail.
- Determinism heisenbug on macOS ARM (float state) — keep the f64 +
  `black_box` guard in `texture.rs`; never introduce process/time/address
  state into generation.
- Flat shading averages face colors, so bit-exact seam tests compare the
  pre-flat vertex grid, not the shaded mesh.
- `clap` eats a leading `-` — pass `--flag=value` for negative args.
- Rasterizer near/far auto-fits; don't hard-code a far plane for large
  dungeons.
- Keep `gallery/`, `docs/`, `skill/`, media OUT of the crate (`Cargo.toml`
  `exclude`); the published crate must stay small.
