# imaginu Phase 6 — Monsters & Dungeons: two new recipe kinds

> **Mission.** Add two new first-class recipe kinds to imaginu: **`monster`**
> (procedural creatures of many body plans) and **`dungeon`** (themed,
> navigable underground layouts). Both must feel as crafted and game-ready as
> the existing `character`/`world` output, ship on a `phase-6` branch, and land
> on `main` behind the same green bar as every prior phase. Work autonomously;
> only stop for the decisions flagged at the bottom.

## How to run this

1. **Brainstorm first.** Use `superpowers:brainstorming` to lock the design —
   resolve the open decisions below, sketch the recipe schemas, pick the body
   plans / dungeon themes for v1. Do NOT skip to code.
2. **Write the plan** with `superpowers:writing-plans` (bite-sized, verifiable
   steps), then **execute it** with `superpowers:executing-plans`.
3. Use `superpowers:test-driven-development` for the generators and
   `superpowers:verification-before-completion` before any "done" claim.

---

## Where the repo is right now (don't rediscover this)

- **Shipped:** v0.1.0 is live — on crates.io, a GitHub Release with 5 prebuilt
  targets, and a Pages site (`docs/site/`) with a live Babylon viewer. CI
  (fmt/clippy/test + determinism on Linux & macOS), `release.yml`, `pages.yml`
  all green. Bumping to **v0.2.0** and cutting a tag is how this phase ships.
- **Library shape:** pure Rust lib (`src/lib.rs`) + CLI (`src/main.rs`).
  `imaginu::compile` / `compile_to_glb` return `Result<_, imaginu::Error>`
  (`error.rs`). MSRV 1.87, edition 2024 (**`gen` is a reserved keyword — the
  module is `generators`**).
- **The recipe pipeline** (this is the spine you extend):
  - `src/recipe.rs` — the `Recipe` enum (`Terrain|Tree|Rock|Crystal|Building|
    Prop|Character|Custom|World`), one `*Params` struct per kind, `Recipe::parse`,
    and `Recipe::build()` which validates the palette then dispatches to
    `generators::<kind>::generate(params, &pal) -> Asset` (or `Result`).
  - `src/generators/*` — one module per kind; `generators/mod.rs` re-exports them.
  - `src/main.rs` — the CLI subcommands (`generate/render/showcase/world/schema/
    validate/validate-world`) **and the `schema` cheat-sheet text** (the agent
    contract — must be updated for every new field).
- **The primitives you will reuse (do NOT reinvent):**
  - `src/sdf.rs` — the **organic body system** (character body v7): Quilez round
    cones / ellipsoids fused with **smooth-min**, meshed by **naive surface
    nets**. This is exactly how believable creatures are built — a monster is a
    *generalization* of a character to non-humanoid topology, not a new engine.
  - `src/skinning.rs` — **family-restricted** multi-joint binding (limbs bind
    only to their own segment chain; junctions gated). `src/anim.rs` — the clip
    system (8 clips today: idle/walk/run/attack/sit/wave/death/dance) with eased,
    multi-axis channels.
  - `src/csg.rs` — subtract/union/intersect (carve doorways, windows, arches).
  - `src/mesh.rs`, `subdiv.rs`, `noise.rs`, `uv.rs`, `palette.rs`, `texture.rs`.
  - `src/world/*` — the **chunk + `manifest.json`** streaming pattern
    (`world/manifest.rs`, `world/model.rs`, POI solver, minimap/overview). The
    dungeon's multi-room output should mirror this manifest idea.
  - `src/generators/character.rs` — **the template for `monster`**.
  - `src/generators/prop.rs` (barrel/crate/lantern/campfire) — reuse + extend for
    **dungeon dressing**.
- **Physics contract:** every GLB embeds a collider at
  `nodes[0].extras.imaginu_physics = {collider, mass, friction, restitution}`.
- **Quality tooling:** `gallery/regen.sh` (+ `regen_showcase.sh`) regenerates the
  reference GLB/PNG/MP4 from `gallery/recipes/`; `imaginu validate` /
  `validate-world` do byte-level structural checks; `docs/EVALUATION.md` holds
  the 6-point visual rubric.

## Non-negotiables (carry these from every prior phase)

1. **Determinism is sacred.** Same recipe + seed → byte-identical GLB across
   processes and platforms. Capture a baseline hash before you start; re-verify
   after. Beware the documented macOS-ARM float heisenbug (fixed with f64
   gradients + `std::hint::black_box`); the CI determinism job guards it.
2. **Render and look.** Every generator change is verified by rendering
   (`--preview`) and *viewing* against the 6-point rubric in `docs/EVALUATION.md`.
   Never claim quality you haven't seen. Score ≥4/5 on every axis before "done."
3. **DRY against `imaginu schema`.** The schema command is the authoritative
   contract; update it for every new field. The skill teaches the *workflow*.
4. **No regressions.** `cargo fmt --check`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test`, `cargo doc --no-deps` all clean; the gallery still
   regenerates; existing kinds unchanged byte-for-byte.
5. **Complete the integration**, not just the generator (see the checklist).

---

## Workstream M — the `monster` recipe

**Goal:** `imaginu generate '{"kind":"monster","species":"wyrm"}' -o m.glb
--preview` yields a rigged, animated, collider-bearing creature that reads
instantly as a game monster.

**Design direction (grounded in `sdf.rs` + `skinning.rs` + `anim.rs`):**
- A monster is a **parameterized skeleton of SDF primitives** (round-cone limbs,
  ellipsoid torso/head, tapered tail/neck) fused with smooth-min and surface-net
  meshed — the same pipeline as `character.rs`, generalized past bipedal humanoid.
- **Body plans (`species`/`body`) — pick ~5–7 for v1**, each a skeleton template:
  `biped_brute`, `quadruped_beast`, `serpent`/`wyrm`, `arachnid` (radial legs),
  `winged_flyer`, `ooze`/`blob`, `insectoid`, `aberration` (tentacled). The plan
  chosen drives limb count/placement, gait, and collider shape.
- **Feature knobs that compose across plans:** `size`/`bulk`, `horns`, `spikes`/
  `plates`, `tail`, `wings`, `eyes` (count + emissive glow), `maw`/teeth,
  `detail` (tessellation, reuse the character knob), `palette` + emissive
  markings for elemental/undead reads. Consider `menace`/`age` style sliders.
- **Animation:** reuse `anim.rs` — at minimum `idle`, a locomotion clip
  appropriate to the plan (`walk`/`slither`/`fly`/`crawl`), `attack`, `hurt`,
  `death`; optionally a `telegraph`/`roar`. Skinning uses family-restricted
  binding — **respect smooth-min fold order** (it *is* skinning correctness:
  fuse core→limbs in the right order or you get membranes/fused joints; debug
  with a stretched-triangle probe, never by guessing blend params).
- **Physics:** capsule/box/trimesh collider auto-fit to the body; `mass` scales
  with size.
- **Variety class (optional):** `predator|brute|swarm|elemental|undead|aberration`
  as a preset bundle over the knobs, like character `class`.

**Quality bar:** at least the v1 species each render convincingly from 4 angles
and animate without deformation artifacts; add a few hero monsters to the
gallery with showcase MP4s.

---

## Workstream D — the `dungeon` recipe

**Goal:** one recipe → a themed, **navigable** dungeon the game can drop players
into, with rooms, corridors, doors, dressing, lighting cues, spawn points, and
colliders.

**Design direction (grounded in `csg.rs` + `world/manifest.rs` + `prop.rs`):**
- **Output format — DECIDE in brainstorming (decision A).** Options: (a) a single
  GLB for small dungeons with layout metadata in `extras`; (b) a **directory +
  `manifest.json`**, mirroring `world` (rooms as the streamable unit, doors,
  spawn points, prop transforms, per-room colliders). Recommended: support both,
  defaulting to the manifest form for anything multi-room, and add a `dungeon`
  subcommand + `validate-dungeon` paralleling `world`/`validate-world`.
- **Layout (deterministic, pure function of seed):** grid/BSP room placement +
  a corridor graph (MST of room centers plus a few extra edges for loops); or
  **cellular-automata caves** for organic types. Multi-level via stairs.
- **Geometry:** floor slabs, extruded walls along layout edges, ceilings,
  **doorway/arch openings via CSG subtract** (remember: **CSG cutters must be
  closed solids** — a lathe/tube profile must touch its axis or the carve
  silently fails). Themed wall materials via `texture.rs`.
- **Themes (`type`) — pick ~4–6 for v1:** `crypt`, `cavern`, `sewer`, `mine`,
  `temple`, `fortress` — each a palette + wall material + prop set + shape bias
  (orthogonal rooms vs. organic caves).
- **Dressing:** reuse `prop.rs` (barrels/crates/lanterns/campfires) and add
  dungeon props (pillar, torch bracket, door/portcullis, sarcophagus, chest,
  rubble). **Emissive torches** double as lighting cues.
- **Metadata in the manifest:** rooms (bounds, kind), doors, corridors,
  `spawn_points` (player start, enemy spawns, loot, boss), and colliders
  (trimesh/heightfield). This is the payload that makes it *usable*, like the
  world POI/spawn data.
- **Seam law analog:** if chunked, room/corridor edges must align exactly (round
  dimensions to integer meters so edges are f32-exact, as `world` does).

**Quality bar:** a generated dungeon of each theme renders as a readable,
atmospheric space (overview + a couple of in-room angles), the manifest round-
trips through `validate-dungeon`, and spawn points/colliders are sane.

---

## Integration checklist (both kinds must complete ALL of this)

- [ ] `recipe.rs`: `*Params` struct + `Recipe` variant + `build()` dispatch +
      palette validation. Provide sensible defaults for every field.
- [ ] `generators/mod.rs`: `pub mod monster; pub mod dungeon;`
- [ ] `generators/{monster,dungeon}.rs`: `generate()` returning `Asset`/`Result`.
- [ ] `main.rs`: extend the **`schema` cheat-sheet** text; add a `dungeon`
      subcommand + `validate-dungeon` if the manifest form is chosen.
- [ ] `validate.rs` / world-style validators: structural checks for new output.
- [ ] Tests: parse+build for each new kind and preset; **determinism**
      (byte-identical, twice); `validate` clean; hostile-input returns `Err`.
- [ ] `gallery/recipes/` + `gallery/regen.sh` (+ showcases) for the hero assets.
- [ ] `skill/imaginu/SKILL.md`: note the new kinds (schema stays the reference).
- [ ] `README.md` recipe gallery + `docs/site/` (viewer models, recipe rows,
      gallery grid) get a monster and a dungeon. **No em-dashes** in docs/site.
- [ ] `CHANGELOG.md`: `Unreleased` → the new features; bump to **v0.2.0**.
- [ ] Ship: green CI, tag `v0.2.0` (release + crates.io publish), site redeploys.

## Definition of done

- `monster` and `dungeon` are first-class kinds: documented in `imaginu schema`,
  covered by tests, present in the gallery, shown on the site and in the skill.
- An agent can go from "make me a fire wyrm" or "make me a crypt dungeon" to a
  loadable, correctly-collided asset using only the skill + the binary.
- v0.2.0 is tagged and green; determinism and all prior kinds are unregressed.

## Known traps (from prior phases)

- Smooth-min **fold order is skinning correctness** — order core→limbs
  deliberately; debug webs with a posed-vs-bind edge-length probe, never by
  guessing blend params. Surface-nets quad winding must match rasterizer culling.
- **CSG cutters must be closed solids** (profiles touch the axis) or carves fail.
- Determinism heisenbug on macOS ARM (float state) — keep the f64 + `black_box`
  guard; never introduce process/time/address-dependent state into generation.
- Flat shading averages face colors, so bit-exact seam tests compare the
  pre-flat vertex grid, not the shaded mesh.
- `clap` eats a leading `-` — pass `--flag=value` for negative args.
- Rasterizer near/far auto-fits; don't hard-code a far plane for large dungeons.
- Keep `gallery/`, `docs/`, `skill/`, media OUT of the crate (Cargo.toml
  `exclude`); the published crate must stay small.

## Open decisions (resolve in brainstorming; reasonable defaults noted)

- **A. Dungeon output format:** single GLB vs. directory + `manifest.json`.
  Default: support both, manifest for multi-room, add `dungeon` +
  `validate-dungeon` subcommands.
- **B. Monster species set for v1:** which ~5–7 body plans, and whether to add a
  `class`/preset layer over the raw knobs.
- **C. New palettes:** do monsters/dungeons need dedicated palettes (e.g.
  `necrotic`, `infernal`, `fungal`) or do the existing six suffice?
- **D. Release cadence:** ship monsters and dungeons together as v0.2.0, or land
  monsters first (v0.2.0) and dungeons as v0.3.0.
