# Phase 6 — Monsters & Dungeons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two first-class recipe kinds — `monster` (8 procedural body plans + class presets) and `dungeon` (6 themed, navigable layouts with a `manifest.json`) — and ship them as v0.2.0 with determinism and all prior kinds unregressed.

**Architecture:** `monster` generalizes `character.rs` — a data-driven `MonsterRig` (joints + fold-order-ranked SDF primitives + gait descriptor) fed to one shared organic pipeline (`sdf::smin` compose → `sdf::mesh_field` → `skinning::smooth_bind` → procedural clips). `dungeon` mirrors `world` — a pure-of-seed `DungeonModel` (rooms/corridors/doors/spawns) → geometry pass (floors/walls/ceilings, `csg::subtract` doorways) → single GLB for ≤1 room or directory + `manifest.json` for multi-room, with `dungeon`/`validate-dungeon` subcommands.

**Tech Stack:** Rust (edition 2024, MSRV 1.87), `serde`/`serde_json`, `rand_chacha` (ChaCha8), existing `sdf`/`skinning`/`anim`/`csg`/`prop`/`world`/`palette`/`texture` modules. No new dependencies.

## Global Constraints

- **Determinism is sacred:** same recipe + seed → byte-identical GLB across processes and platforms. Pure functions of `(recipe, seed)` via `generators::rng` (ChaCha8) and fixed SDF grids only. No process/time/address state.
- **Edition 2024:** `gen` is a reserved keyword — the module is `generators`.
- **MSRV 1.87**, edition 2024. Use `.is_multiple_of()` where clippy wants it.
- **DRY against `imaginu schema`:** the `SCHEMA_HELP` const (`src/main.rs:580-740`) is the authoritative agent contract — update it for every new field.
- **Green bar:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo doc --no-deps` all clean before any "done".
- **Render and look:** every generator change is verified by `--preview` render against the 6-point rubric in `docs/EVALUATION.md`; score ≥4/5 on every axis. Never claim quality unseen.
- **No em-dashes** in `docs/` or `docs/site/`.
- **Keep media out of the crate:** `gallery/`, `docs/`, `skill/` stay in `Cargo.toml` `exclude`.
- **Existing kinds unchanged byte-for-byte.**

## File structure

**Monster** (mirror `generators/character.rs`, but split for focus):
- Create `src/generators/monster/mod.rs` — `pub fn generate(&MonsterParams, &Palette) -> Asset`; orchestrates rig → body → skin → clips → asset.
- Create `src/generators/monster/rig.rs` — `MonsterRig`, `PrimitiveDesc`, `GaitDesc`, and the 8 `plan_*` builders.
- Create `src/generators/monster/body.rs` — `organic_field` (fold-order compose), collider fitting.
- Create `src/generators/monster/anim.rs` — procedural clip driver.
- Create `src/generators/monster/preset.rs` — `MonsterClass` → knob defaults.

**Dungeon** (mirror `src/world/`):
- Create `src/generators/dungeon/mod.rs` — `pub fn generate(&DungeonParams, &Palette) -> Result<Asset, String>` (single-GLB path) and re-exports.
- Create `src/generators/dungeon/model.rs` — `DungeonModel`, `Room`, `Corridor`, `Door`, `SpawnPoint`.
- Create `src/generators/dungeon/layout.rs` — BSP/grid + MST corridors + CA caves (pure of seed).
- Create `src/generators/dungeon/geom.rs` — floor/wall/ceiling/arch geometry.
- Create `src/generators/dungeon/dress.rs` — dungeon props (pillar/torch/door/sarcophagus/chest/rubble).
- Create `src/generators/dungeon/manifest.rs` — `Manifest`/`RoomEntry`, `create`, `write_dir`, `validate_dir`.

**Shared / wiring:**
- Modify `src/recipe.rs` — 2 `*Params` structs, `MonsterParams`/`DungeonParams` enums, 2 `Recipe` variants, `palette_name`/`build` arms.
- Modify `src/generators/mod.rs` — `pub mod monster; pub mod dungeon;`.
- Modify `src/palette.rs` — add `necrotic`/`infernal`/`fungal`.
- Modify `src/main.rs` — `SCHEMA_HELP` blocks; `Dungeon`/`ValidateDungeon` subcommands + handlers.
- Modify docs: `README.md`, `docs/site/*`, `skill/imaginu/SKILL.md`, `CHANGELOG.md`, `Cargo.toml` (version).
- Create `gallery/recipes/fire_wyrm.json`, `gallery/recipes/crypt.json`.

---

## Phase 0 — Baseline

### Task 0: Capture the determinism baseline

**Files:** none (produces a scratch hash file outside the repo).

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: compiles clean.

- [ ] **Step 2: Hash every existing kind's output twice**

Run this and save the output to the scratchpad:
```bash
BIN=./target/release/imaginu
OUT=/private/tmp/claude-501/-Users-victorbona-Code-OpenSource-imaginu/50a6ef67-4755-42cc-b93b-18edaeb93b01/scratchpad/baseline.txt
: > "$OUT"
for r in gallery/recipes/*.json; do
  "$BIN" generate "$(cat "$r")" -o /tmp/a.glb 2>/dev/null && \
  "$BIN" generate "$(cat "$r")" -o /tmp/b.glb 2>/dev/null && \
  echo "$r $(shasum -a256 /tmp/a.glb | cut -d' ' -f1) $(shasum -a256 /tmp/b.glb | cut -d' ' -f1)" >> "$OUT"
done
cat "$OUT"
```
Expected: every line's two hashes are identical (in-process determinism holds).

- [ ] **Step 3: Note the baseline** — these hashes must be unchanged after the whole phase. No commit (scratch only).

---

## Phase P — Palettes

### Task P1: Add `necrotic`, `infernal`, `fungal` palettes

**Files:**
- Modify: `src/palette.rs` (the `PALETTES` array ~line 191, the `by_name` match ~line 89).
- Test: `src/palette.rs` (`#[cfg(test)]` module) or `tests/` — inline unit test.

**Interfaces:**
- Consumes: existing `Palette` struct `{ name, terrain:[Vec3;6], foliage:[Vec3;3], trunk:Vec3, rock:[Vec3;2], water:Vec3, accent:Vec3 }`, `srgb`/`hex` helpers.
- Produces: `PALETTES: [&str; 9]`; `by_name("necrotic"|"infernal"|"fungal")` returns tuned palettes. `accent` is the emissive channel.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn new_palettes_registered_and_distinct() {
    for name in ["necrotic", "infernal", "fungal"] {
        assert!(PALETTES.contains(&name), "{name} missing from PALETTES");
        let p = by_name(name);
        assert_eq!(p.name, name);
    }
    // distinct accents so themes read differently
    assert_ne!(by_name("necrotic").accent, by_name("infernal").accent);
    assert_ne!(by_name("infernal").accent, by_name("fungal").accent);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test palette::tests::new_palettes_registered_and_distinct`
Expected: FAIL (names not in PALETTES).

- [ ] **Step 3: Implement — extend `PALETTES` to 9 and add three `by_name` arms**

In `by_name`, add arms modeled on `volcanic`/`mystic`. Use `hex("...")` for colors (follow the existing helper). Concrete values:
```rust
"necrotic" => Palette {
    name: "necrotic",
    terrain: [hex("#2b2f27"), hex("#3a4033"), hex("#4a5140"), hex("#5b6350"), hex("#6d7560"), hex("#7f8871")],
    foliage: [hex("#5a6b3d"), hex("#485a30"), hex("#6b7a4a")],
    trunk: hex("#3b352b"),
    rock: [hex("#4b4f47"), hex("#6a6f63")],
    water: hex("#3d4a3a"),
    accent: hex("#9dff6b"), // sickly emissive green
},
"infernal" => Palette {
    name: "infernal",
    terrain: [hex("#1a1412"), hex("#2a1c16"), hex("#3a251a"), hex("#4d2c1c"), hex("#63331f"), hex("#7a3a22")],
    foliage: [hex("#5a2620"), hex("#43201c"), hex("#6e2c22")],
    trunk: hex("#241a16"),
    rock: [hex("#332a26"), hex("#554842")],
    water: hex("#5a1f16"),
    accent: hex("#ff5a1e"), // ember emissive
},
"fungal" => Palette {
    name: "fungal",
    terrain: [hex("#221a2b"), hex("#2c2238"), hex("#382c47"), hex("#453556"), hex("#524066"), hex("#5f4b77")],
    foliage: [hex("#3a6b6b"), hex("#2e5858"), hex("#4a7a7a")],
    trunk: hex("#2a2233"),
    rock: [hex("#3a3547"), hex("#524b63")],
    water: hex("#2a4a52"),
    accent: hex("#4be0c0"), // bioluminescent teal
},
```
Update the array: `pub const PALETTES: [&str; 9] = ["verdant","autumn","arctic","volcanic","desert","mystic","necrotic","infernal","fungal"];`

If `hex` returns `Vec3` directly this compiles; if it is `to_srgb` based, follow whatever the other arms use verbatim.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test palette`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/palette.rs
git commit -m "feat(palette): add necrotic, infernal, fungal palettes"
```

---

## Phase M — Monster

### Task M1: `MonsterParams`, enums, `Recipe` variant, wiring, stub generator

**Files:**
- Modify: `src/recipe.rs` (enums near ~line 23-66; add `MonsterParams`; `Recipe` variant ~line 262-312; `palette_name` ~319; `build` ~333-356).
- Create: `src/generators/monster/mod.rs` (stub).
- Modify: `src/generators/mod.rs` (add `pub mod monster;`).
- Test: `src/recipe.rs` test module.

**Interfaces:**
- Produces:
  - `enum BodyPlan { BipedBrute, QuadrupedBeast, Serpent, Arachnid, WingedFlyer, Ooze, Insectoid, Aberration }` — `#[serde(rename_all="snake_case")]`, `#[default] QuadrupedBeast`. Accept `wyrm` as alias for `Serpent` and `blob` for `Ooze` via `#[serde(alias=...)]`.
  - `enum MonsterClass { None, Predator, Brute, Elemental, Undead, Aberration, Swarm }` — default `None`.
  - `struct MonsterParams { seed:u64, body:BodyPlan (serde alias "species"), class:MonsterClass, size:f32 (alias "bulk"), horns:f32, spikes:f32, plates:f32, tail:f32, wings:f32, eyes:u32, maw:f32, menace:f32, age:f32, emissive:f32, detail:f32, animate:bool }` — every field `#[serde(default="...")]`. `tail`/`wings`/`eyes`/`maw`/`emissive` default to `-1.0`/sentinel meaning "plan/class decides" (use `f32::NAN`-free sentinel `-1.0` and resolve in the generator).
  - `Recipe::Monster { palette (default d_palette), #[serde(flatten)] params: MonsterParams }`.
  - `generators::monster::generate(&MonsterParams, &Palette) -> Asset`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn monster_parses_and_builds() {
    let r = Recipe::parse(r#"{"kind":"monster","species":"wyrm","palette":"infernal"}"#).unwrap();
    let asset = r.build().expect("monster builds");
    assert!(!asset.parts.is_empty());
    assert!(asset.physics.is_some());
    // aliases resolve
    let r2 = Recipe::parse(r#"{"kind":"monster","body":"serpent"}"#).unwrap();
    r2.build().unwrap();
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test recipe::tests::monster_parses_and_builds`
Expected: FAIL (unknown variant `monster`).

- [ ] **Step 3: Add enums + `MonsterParams` + `Recipe::Monster`**

Add the enums and struct per the Interfaces block. Add default fns (reuse `d_seed`, `d_one`; add `d_zero()->0.0`, `d_neg1()->-1.0`, `d_true()->true`, `d_eyes()->u32::MAX` sentinel or `-1` via i64 — simplest: `eyes: i32` default `-1`). Add the variant to the `Recipe` enum, a `palette_name` arm, and a `build` arm: `Recipe::Monster { params, .. } => generators::monster::generate(params, &pal)`.

- [ ] **Step 4: Add module + stub generator**

`src/generators/mod.rs`: add `pub mod monster;`.
`src/generators/monster/mod.rs` (stub that compiles and returns a placeholder so wiring tests pass):
```rust
use crate::palette::Palette;
use crate::recipe::MonsterParams;
use crate::asset::Asset; // match the actual Asset path used by character.rs

pub fn generate(p: &MonsterParams, pal: &Palette) -> Asset {
    // TEMP stub — replaced in M2-M8. Returns a tiny ellipsoid so wiring passes.
    body_stub(p, pal)
}
```
Implement `body_stub` as a single `sdf::mesh_field` ellipsoid with a capsule collider (copy the smallest working pattern from `prop.rs::barrel`). Keep it real enough that `asset.validate()` passes.

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test recipe::tests::monster_parses_and_builds && cargo build`
Expected: PASS + clean build.

- [ ] **Step 6: Commit**

```bash
git add src/recipe.rs src/generators/mod.rs src/generators/monster/mod.rs
git commit -m "feat(monster): recipe wiring + stub generator"
```

### Task M2: `MonsterRig` data model + `plan_quadruped_beast`

**Files:**
- Create: `src/generators/monster/rig.rs`.
- Modify: `src/generators/monster/mod.rs` (add `mod rig;`).
- Test: `src/generators/monster/rig.rs` test module.

**Interfaces:**
- Produces:
  - `struct PrimitiveDesc { kind: PrimKind, joint_a: usize, joint_b: usize, r1: f32, r2: f32, fold_rank: u8, k: f32 }` where `enum PrimKind { RoundCone, Ellipsoid }` and `fold_rank` orders smooth-min compose (0 = core, higher = later).
  - `struct GaitDesc { legs: Vec<Vec<usize>>, spine: Vec<usize>, wings: Vec<usize>, tail: Vec<usize>, head: Option<usize>, style: Gait }` with `enum Gait { Walk, Slither, Fly, Crawl, Pulse }`.
  - `struct MonsterRig { skeleton: Skeleton, prims: Vec<PrimitiveDesc>, gait: GaitDesc, bounds: (Vec3, Vec3) }`.
  - `fn build_rig(p: &MonsterParams) -> MonsterRig` dispatching on `p.body` to `plan_*`.
  - `fn plan_quadruped_beast(p: &MonsterParams) -> MonsterRig`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn quadruped_rig_is_wellformed() {
    let p = MonsterParams::default(); // body = QuadrupedBeast
    let rig = build_rig(&p);
    assert_eq!(rig.gait.legs.len(), 4, "quadruped has 4 legs");
    assert!(matches!(rig.gait.style, Gait::Walk));
    // fold ranks: at least one core prim (rank 0) exists and ranks are monotone-usable
    assert!(rig.prims.iter().any(|d| d.fold_rank == 0));
    // every prim references valid joints
    let n = rig.skeleton.joints.len();
    assert!(rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
}
```
(Adjust `MonsterParams::default()` — the struct derives `Deserialize` with serde defaults, not `Default`. Add `impl Default for MonsterParams` that calls `serde_json::from_str("{}")` OR derive `Default` by giving each field a matching `#[default]`; simplest is a small `impl Default` in rig.rs test using `Recipe::parse(r#"{"kind":"monster"}"#)` then extracting params. Prefer: add `#[derive(Default)]`-compatible defaults.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test generators::monster::rig`
Expected: FAIL (no `build_rig`).

- [ ] **Step 3: Implement `rig.rs`**

Build the skeleton with a small joint layout for a quadruped: hips, spine×2, neck, head, tail×2, and 4 legs × (upper, lower, foot). Use `Skeleton`/joint construction the same way `character.rs::build_rig` does (bind-pose local transforms; parent indices). Populate `prims`:
- Core torso: `Ellipsoid` on hips→spine, `fold_rank 0`.
- Neck/head: `RoundCone`, `fold_rank 1`.
- Each leg: two `RoundCone` segments, `fold_rank 2`.
- Tail: tapered `RoundCone` chain, `fold_rank 3`.
Set `k` (smin blend) larger near the core, smaller at limb tips (mirror character body v7: shoulder-ramped k, hard-min far from core — encode via small `k` and a rank gate consumed in body.rs). Set `gait.legs` to the four foot-joint chains, `gait.spine`, `gait.head`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test generators::monster::rig`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/generators/monster/rig.rs src/generators/monster/mod.rs
git commit -m "feat(monster): rig data model + quadruped template"
```

### Task M3: Shared organic body pass + collider + first real render

**Files:**
- Create: `src/generators/monster/body.rs`.
- Modify: `src/generators/monster/mod.rs` (wire `generate` to use `build_rig` + `build_body`).
- Test: `src/generators/monster/body.rs` test module + a visual render check.

**Interfaces:**
- Consumes: `MonsterRig`, `sdf::{smin, sd_round_cone, sd_ellipsoid, mesh_field}`, `palette::Palette`.
- Produces:
  - `fn organic_field(rig: &MonsterRig) -> impl Fn(Vec3) -> f32` — composes prims by ascending `fold_rank`, `smin(acc, prim, k)` within a rank band, near-hard-min across bands far from core.
  - `fn build_body(rig: &MonsterRig, p: &MonsterParams, pal: &Palette) -> Mesh` — meshes via `mesh_field` over `rig.bounds` at cell size scaled by `p.detail`, colors by palette (body base + `accent` emissive where `emissive>0`).
  - `fn fit_collider(rig: &MonsterRig, p: &MonsterParams) -> Physics` — capsule/box/elongated-capsule by plan; `mass = base * size.powi(3)`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn quadruped_body_meshes_and_is_watertight_ish() {
    let p = MonsterParams::default();
    let pal = crate::palette::by_name("verdant");
    let rig = build_rig(&p);
    let mesh = build_body(&rig, &p, &pal);
    assert!(mesh.positions.len() > 500, "non-trivial mesh");
    assert!(mesh.indices.len() % 3 == 0, "triangulated");
    // bounds sanity: mesh sits within padded rig bounds
    let (lo, hi) = rig.bounds;
    for v in &mesh.positions {
        assert!(v.x >= lo.x - 0.5 && v.x <= hi.x + 0.5);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test generators::monster::body`
Expected: FAIL.

- [ ] **Step 3: Implement `body.rs`** — `organic_field`, `build_body`, `fit_collider`. Wire `generate` in `mod.rs` to: `let rig = build_rig(p); let mesh = build_body(&rig,p,pal); let phys = fit_collider(&rig,p); Asset::static_mesh(...)` (no skin/anim yet — added M4/M5). Remove the stub body.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test generators::monster::body`
Expected: PASS.

- [ ] **Step 5: RENDER AND LOOK (mandatory gate)**

Run:
```bash
cargo run --release -- generate '{"kind":"monster","body":"quadruped_beast"}' -o /tmp/q.glb --preview
```
Open the produced PNG. Verify against `docs/EVALUATION.md`: reads as a four-legged creature, no fused-limb membranes, silhouette legible from the preview angle. If fold order produces webs, adjust `fold_rank`/`k` in `rig.rs` (core→limbs order) — never guess; add the edge-length probe from M4 first if needed. Iterate until ≥4/5 silhouette/form.

- [ ] **Step 6: Commit**

```bash
git add src/generators/monster/body.rs src/generators/monster/mod.rs
git commit -m "feat(monster): shared organic body pass + collider fitting"
```

### Task M4: Family-restricted skinning + stretched-triangle probe

**Files:**
- Modify: `src/generators/monster/mod.rs` (apply skinning), `src/generators/monster/body.rs` (expose segments).
- Create test helper: `src/generators/monster/mod.rs` test module with the probe.

**Interfaces:**
- Consumes: `skinning::{BoneSeg, smooth_bind, skeleton_segments}`.
- Produces: `fn skin_body(mesh: &mut Mesh, rig: &MonsterRig)` — family-restricted binding: build `BoneSeg`s per limb chain and bind each region against only its own segments (mirror character v7 family restriction); junction-gated blend.
- Produces test helper `fn max_edge_stretch(bind: &Mesh, posed: &Mesh) -> f32`.

- [ ] **Step 1: Write the failing test (the probe)**

```rust
#[test]
fn locomotion_does_not_shatter_mesh() {
    let p = MonsterParams::default();
    let pal = crate::palette::by_name("verdant");
    let asset = generate(&p, &pal); // now rigged + animated after M5; for M4 pose the walk-less rig identity
    // Build bind mesh and a posed mesh at a mid-frame of the first locomotion clip.
    let bind = first_mesh(&asset);
    let posed = pose_first_clip_midframe(&asset); // helper using anim::pose_asset
    let stretch = max_edge_stretch(&bind, &posed);
    assert!(stretch < 2.5, "edge stretch {stretch} indicates skinning web");
}
```
(For M4, before M5 clips exist, test against a hand-built 1-channel test clip that rotates one leg 30°; replace with the real clip check in M5.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test generators::monster::mod::locomotion_does_not_shatter_mesh`
Expected: FAIL (no skinning / stretch too high).

- [ ] **Step 3: Implement `skin_body`** — derive per-limb `BoneSeg` chains from the rig (not global `skeleton_segments`, which would fly shoulders per the memory), bind trunk to spine chain only, each leg to its own segments, pelvis rigid. Apply in `generate`.

- [ ] **Step 4: Run to verify it passes** (iterate on segment radii/falloff using the probe output, per the memory: fix geometric overlap, not blend params). Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/generators/monster/mod.rs src/generators/monster/body.rs
git commit -m "feat(monster): family-restricted skinning + stretch probe"
```

### Task M5: Procedural clip driver

**Files:**
- Create: `src/generators/monster/anim.rs`.
- Modify: `src/generators/monster/mod.rs` (assemble clips when `p.animate`).
- Test: `src/generators/monster/anim.rs` test module.

**Interfaces:**
- Consumes: `gltf::{AnimationClip, Channel, ChannelData}`, `anim::clip_duration`, the `keys/rot_channel/trans_channel/env` pattern from `character.rs:2005-2063` (copy these helpers locally or lift to a shared `anim` util).
- Produces: `fn build_clips(rig: &MonsterRig, p: &MonsterParams) -> Vec<AnimationClip>` producing `idle`, one locomotion clip named per `gait.style` (`walk`/`slither`/`fly`/`crawl`/`pulse`), `attack`, `hurt`, `death`, and `roar` iff `gait.head.is_some()`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn quadruped_has_expected_clips() {
    let p = MonsterParams::default();
    let rig = build_rig(&p);
    let clips = build_clips(&rig, &p);
    let names: Vec<_> = clips.iter().map(|c| c.name.as_str()).collect();
    for want in ["idle", "walk", "attack", "hurt", "death"] {
        assert!(names.contains(&want), "missing clip {want}");
    }
    // every channel targets a real joint, durations > 0
    for c in &clips {
        assert!(crate::anim::clip_duration(c) > 0.0);
        for ch in &c.channels { assert!((ch.joint as usize) < rig.skeleton.joints.len()); }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test generators::monster::anim`
Expected: FAIL.

- [ ] **Step 3: Implement `anim.rs`** — a generic driver: `idle` = subtle spine/head bob + breathing; locomotion = phase-offset leg swing across `gait.legs` (offset by leg index for a natural gait), spine counter-rotation; `slither` = sine wave along `gait.spine`; `fly` = wing flap on `gait.wings` + pitch; `pulse` = uniform scale-ish bob for ooze; `attack` = lunge + head/maw; `hurt` = recoil; `death` = topple + settle; `roar` = head raise + jaw. Reuse eased multi-axis channels.

- [ ] **Step 4: Run to verify it passes**, then re-run the M4 stretch probe against the real `walk` clip midframe. Expected: PASS + stretch < 2.5.

- [ ] **Step 5: RENDER AND LOOK** — `--animation walk` render of the quadruped; verify no deformation artifacts across the cycle.

- [ ] **Step 6: Commit**

```bash
git add src/generators/monster/anim.rs src/generators/monster/mod.rs
git commit -m "feat(monster): procedural clip driver (idle/locomotion/attack/hurt/death/roar)"
```

### Task M6: Remaining 7 body plans

**Files:** Modify `src/generators/monster/rig.rs` (add `plan_*` for the other 7); `body.rs`/`anim.rs` only if a plan needs a new gait branch (already covered by `Gait`).
**Test:** `src/generators/monster/rig.rs`.

Each plan is one sub-task (write template → build test asserting leg/limb counts + gait → RENDER AND LOOK from 4 angles → commit). Implement in this order (simplest topology first):

- [ ] **M6a `ooze`/`blob`** — 1-2 ellipsoids, `Gait::Pulse`, box collider, no legs. Test: `legs.is_empty()`, `style==Pulse`. Render.
- [ ] **M6b `serpent`/`wyrm`** — long tapered `RoundCone` spine chain (8-12 joints), `Gait::Slither`, elongated capsule. Test: `spine.len() >= 8`. Render (hero candidate). Commit.
- [ ] **M6c `biped_brute`** — 2 legs + 2 arms + torso + head, `Gait::Walk`, capsule. Reuse near-humanoid layout from `character.rs`. Render 4 angles. Commit.
- [ ] **M6d `winged_flyer`** — biped core + `gait.wings` populated, `Gait::Fly`, capsule. Wing membranes as thin ellipsoids ranked last. Render (flap). Commit.
- [ ] **M6e `arachnid`** — radial 6-8 legs around a low body, `Gait::Crawl`, trimesh collider. Test: `legs.len() >= 6`. Render top+side. Commit.
- [ ] **M6f `insectoid`** — segmented thorax/abdomen + 6 legs + antennae, `Gait::Crawl`. Render. Commit.
- [ ] **M6g `aberration`** — central mass + N tentacle `RoundCone` chains (radial), `Gait::Pulse`, trimesh collider. Render. Commit.

Each sub-task's render must hit ≥4/5 silhouette/form or its `fold_rank`/`k`/segment geometry is adjusted before committing.

### Task M7: `MonsterClass` presets

**Files:** Create `src/generators/monster/preset.rs`; modify `mod.rs` to apply presets before `build_rig`.
**Test:** `src/generators/monster/preset.rs`.

**Interfaces:**
- Produces: `fn apply_preset(p: &mut MonsterParams)` — for each `MonsterClass`, fill only sentinel/unset fields (respect explicit overrides): e.g. `Undead` → `palette` default `necrotic`, `emissive 0.3`, `age 0.6`; `Elemental` → `infernal`, `emissive 0.8`; `Predator` → `maw 0.8`, `spikes 0.3`; `Brute` → `size 1.4`, `plates 0.6`; `Aberration` → `eyes 5`, `tail 0`; `Swarm` → `size 0.6`.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn undead_preset_sets_defaults_but_respects_overrides() {
    let mut p = params_from(r#"{"kind":"monster","class":"undead"}"#);
    apply_preset(&mut p);
    assert!(p.emissive > 0.0);
    // explicit override wins
    let mut p2 = params_from(r#"{"kind":"monster","class":"undead","emissive":0.0}"#);
    apply_preset(&mut p2);
    assert_eq!(p2.emissive, 0.0);
}
```
(Requires distinguishing "unset" from "explicitly 0". Use the `-1.0` sentinel default for preset-controlled floats; `apply_preset` only fills `< 0.0` values, then a final clamp normalizes remaining sentinels to 0.)

- [ ] **Step 2-4:** Run (fail) → implement `preset.rs` + call `apply_preset(&mut p.clone())` at the top of `generate` → run (pass).
- [ ] **Step 5: RENDER AND LOOK** — render one monster per class; verify the read (undead looks necrotic, elemental glows).
- [ ] **Step 6: Commit** `feat(monster): class presets`.

### Task M8: Feature knobs (horns/spikes/plates/tail/wings/eyes/maw/emissive)

**Files:** Modify `src/generators/monster/rig.rs` (knobs add prims), `body.rs` (emissive coloring), `mod.rs`.
**Test:** `rig.rs`.

**Interfaces:** knobs add/scale `PrimitiveDesc`s: `horns` → 2 ranked-last cones at head; `spikes`/`plates` → ridge of small prims along spine; `tail`/`wings` → enable/scale existing chains; `eyes` → emissive ellipsoids at head (count = resolved `eyes`); `maw` → jaw prim scale; `emissive` → fraction of body colored with `pal.accent` emissive.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn horns_and_eyes_add_geometry() {
    let base = build_rig(&params_from(r#"{"kind":"monster","body":"quadruped_beast"}"#));
    let horned = build_rig(&params_from(r#"{"kind":"monster","body":"quadruped_beast","horns":1.0,"eyes":4}"#));
    assert!(horned.prims.len() > base.prims.len(), "horns/eyes add prims");
}
```

- [ ] **Step 2-4:** fail → implement knob prim additions (all fold-ranked after limbs) → pass.
- [ ] **Step 5: RENDER AND LOOK** — a maxed-knob monster (horns+spikes+plates+glowing eyes); verify additive detail reads and does not create webs.
- [ ] **Step 6: Commit** `feat(monster): composable feature knobs`.

### Task M9: Schema help + determinism + hostile-input tests

**Files:** Modify `src/main.rs` (`SCHEMA_HELP` ~580-740); test in `src/recipe.rs`.

- [ ] **Step 1: Determinism + hostile tests**

```rust
#[test]
fn monster_is_deterministic() {
    let json = r#"{"kind":"monster","body":"wyrm","seed":7,"class":"elemental"}"#;
    let a = Recipe::parse(json).unwrap().build().unwrap();
    let b = Recipe::parse(json).unwrap().build().unwrap();
    assert_eq!(crate::gltf::to_bytes(&a).unwrap(), crate::gltf::to_bytes(&b).unwrap());
}

#[test]
fn monster_survives_hostile_input() {
    // absurd numeric values must clamp, not panic
    let json = r#"{"kind":"monster","size":1e30,"detail":1e30,"horns":-5.0,"eyes":999999}"#;
    Recipe::parse(json).unwrap().build().unwrap();
}
```
(Use whatever the crate's GLB-serialization entry point is — match how existing determinism tests compare bytes; if they render via CLI, follow that pattern. Ensure generators clamp `size`/`detail`/knobs to sane ranges.)

- [ ] **Step 2: Run (fail if clamps missing) → add clamps in `generate`/`build_rig` → pass.**
- [ ] **Step 3: Add `SCHEMA_HELP` monster block** — a JSON example listing every field with its default and the 8 body plans + 7 classes, styled like the existing character block.
- [ ] **Step 4: Run** `cargo run --release -- schema | grep -A20 monster` — verify the block prints.
- [ ] **Step 5: Commit** `feat(monster): schema docs + determinism & hostile-input tests`.

### Task M10: Gallery hero — fire wyrm

**Files:** Create `gallery/recipes/fire_wyrm.json`; run `gallery/regen.sh`/`regen_showcase.sh`.

- [ ] **Step 1:** Write `fire_wyrm.json`: `{"kind":"monster","body":"wyrm","class":"elemental","palette":"infernal","size":1.6,"horns":0.7,"spikes":0.5,"eyes":2,"emissive":0.8}`.
- [ ] **Step 2:** Regenerate the reference GLB/PNG/MP4 for it (follow `gallery/regen.sh` usage).
- [ ] **Step 3: RENDER AND LOOK** — 4 angles + showcase MP4; must hit ≥4/5 on all rubric axes.
- [ ] **Step 4: Commit** `feat(gallery): fire wyrm hero monster`.

---

## Phase D — Dungeon

### Task D1: `DungeonParams`, enums, `Recipe` variant, wiring, stub

**Files:** Modify `src/recipe.rs`; create `src/generators/dungeon/mod.rs` (stub `generate -> Result<Asset,String>`); modify `src/generators/mod.rs`.
**Test:** `src/recipe.rs`.

**Interfaces:**
- Produces: `enum DungeonTheme { Crypt, Cavern, Sewer, Mine, Temple, Fortress }` (default `Crypt`); `enum DungeonSize { Small, Medium, Large }` (default `Medium`); `struct DungeonParams { seed:u64, theme:DungeonTheme (serde rename "type"), size:DungeonSize, rooms:Option<u32>, loops:f32, density:f32, detail:f32 }`; `Recipe::Dungeon { palette (default d_palette), #[serde(flatten)] params }`; `generators::dungeon::generate(&DungeonParams,&Palette) -> Result<Asset,String>` (in `build`, dispatch with `?`).

- [ ] **Step 1: Failing test**

```rust
#[test]
fn dungeon_parses_and_builds_single_glb() {
    let r = Recipe::parse(r#"{"kind":"dungeon","type":"crypt","rooms":1}"#).unwrap();
    let asset = r.build().expect("1-room dungeon builds a single asset");
    assert!(!asset.parts.is_empty());
    assert!(asset.physics.is_some());
}
```

- [ ] **Step 2-6:** fail → add enums/struct/variant/arms + `mod.rs` stub (a single boxed room via `csg`/`prop` pattern, capsule/trimesh collider) → pass → commit `feat(dungeon): recipe wiring + stub generator`.

### Task D2: `DungeonModel` layout (pure of seed)

**Files:** Create `src/generators/dungeon/model.rs`, `src/generators/dungeon/layout.rs`; modify `mod.rs`.
**Test:** `layout.rs`.

**Interfaces:**
- Produces:
  - `struct Room { id: usize, kind: RoomKind, min: Vec3, max: Vec3 }` (`enum RoomKind { Entrance, Normal, Boss, Treasure }`), all dims **integer meters**.
  - `struct Corridor { a: usize, b: usize, path: Vec<Vec3> }`, `struct Door { room: usize, corridor: usize, pos: Vec3 }`, `struct SpawnPoint { kind: SpawnKind, pos: Vec3 }` (`enum SpawnKind { Player, Enemy, Loot, Boss }`).
  - `struct DungeonModel { p: DungeonParams, pal: Palette, rooms: Vec<Room>, corridors: Vec<Corridor>, doors: Vec<Door>, spawns: Vec<SpawnPoint>, bounds:(Vec3,Vec3) }`.
  - `fn DungeonModel::new(&DungeonParams, &Palette) -> Result<Self, String>` — orthogonal themes: BSP/grid rooms → MST of centers → add `loops` fraction of extra edges → carve doors at room/corridor intersections → place spawns (entrance=Player, farthest room=Boss, treasure rooms=Loot, others=Enemy). Cavern theme: seeded cellular automata → connected-component rooms.

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn layout_is_pure_of_seed() {
    let p = params(r#"{"kind":"dungeon","type":"crypt","seed":42}"#);
    let a = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
    let b = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
    assert_eq!(a.rooms.len(), b.rooms.len());
    assert_eq!(a.rooms[0].min, b.rooms[0].min);
}

#[test]
fn layout_is_connected_and_integer_aligned() {
    let p = params(r#"{"kind":"dungeon","type":"crypt","size":"medium"}"#);
    let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
    assert!(m.rooms.len() >= 3);
    // every room reachable from entrance via corridors (union-find)
    assert!(all_rooms_connected(&m));
    // integer-meter alignment (seam-law analog)
    for r in &m.rooms { assert_eq!(r.min.x, r.min.x.round()); assert_eq!(r.max.z, r.max.z.round()); }
}
```

- [ ] **Step 2-4:** fail → implement `layout.rs` (BSP split, MST via Prim/Kruskal over room-center distances, loop edges, CA caves) + `model.rs` → pass.
- [ ] **Step 5: Commit** `feat(dungeon): seed-pure layout model (rooms/corridors/doors/spawns)`.

### Task D3: Geometry pass (floors/walls/ceilings + CSG doorways)

**Files:** Create `src/generators/dungeon/geom.rs`; modify `mod.rs` single-GLB path to build from the model.
**Test:** `geom.rs`.

**Interfaces:**
- Produces: `fn room_mesh(room:&Room, theme:DungeonTheme, pal:&Palette, detail:f32) -> Mesh` (floor slab, extruded walls, ceiling); `fn carve_doorways(walls:Mesh, doors:&[Door]) -> Mesh` via `csg::subtract` with **closed-solid** arch cutters (profile touches base); `fn corridor_mesh(c:&Corridor, ...) -> Mesh`.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn room_geometry_has_floor_walls_ceiling_and_carved_door() {
    let p = params(r#"{"kind":"dungeon","type":"crypt","rooms":2}"#);
    let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
    let mesh = carve_doorways(room_mesh(&m.rooms[0], DungeonTheme::Crypt, &by_name("necrotic"), 1.0), &m.doors);
    assert!(mesh.indices.len() % 3 == 0);
    assert!(mesh.positions.len() > 100);
    // carve actually removed geometry vs. an uncarved wall (door opening exists)
    let solid = room_mesh(&m.rooms[0], DungeonTheme::Crypt, &by_name("necrotic"), 1.0);
    assert!(mesh.positions.len() != solid.positions.len());
}
```

- [ ] **Step 2-4:** fail → implement `geom.rs` (watch the closed-solid cutter trap) → pass.
- [ ] **Step 5: RENDER AND LOOK** — build a small crypt to a single GLB, `--preview` an in-room angle: readable floor/walls/ceiling, doorway actually open.
- [ ] **Step 6: Commit** `feat(dungeon): room/corridor geometry with CSG doorways`.

### Task D4: Manifest + multi-room directory output + `validate-dungeon`

**Files:** Create `src/generators/dungeon/manifest.rs`; modify `mod.rs`.
**Test:** `manifest.rs`.

**Interfaces:**
- Produces (mirror `world/manifest.rs`): `struct Manifest { format:"imaginu-dungeon/1", name, seed, palette, theme, bounds, rooms:Vec<RoomEntry>, corridors:Vec<Polyline>, doors:Vec<DoorEntry>, spawn_points:Vec<SpawnEntry> }`; `RoomEntry{ id, kind, file, min, max }`; `fn create(&DungeonModel) -> Manifest`; `fn write_dir(model:&DungeonModel, dir:&Path) -> Result<(),String>` (per-room GLB + `manifest.json`); `fn validate_dir(&Path) -> Result<String,String>`.

- [ ] **Step 1: Failing test (round-trip)**

```rust
#[test]
fn manifest_round_trips_through_validate() {
    let p = params(r#"{"kind":"dungeon","type":"crypt","size":"medium"}"#);
    let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
    let man = create(&m);
    assert_eq!(man.format, "imaginu-dungeon/1");
    assert_eq!(man.rooms.len(), m.rooms.len());
    // serialize + deserialize
    let s = serde_json::to_string(&man).unwrap();
    let back: Manifest = serde_json::from_str(&s).unwrap();
    assert_eq!(back.rooms.len(), man.rooms.len());
}
```

- [ ] **Step 2-4:** fail → implement `manifest.rs` (+ `write_dir` using the same per-file GLB writing as world chunks) → pass.
- [ ] **Step 5:** Add a `validate_dir` test writing to a temp dir and validating.
- [ ] **Step 6: Commit** `feat(dungeon): manifest.json + multi-room directory output + validator`.

### Task D5: Dressing props + spawn markers

**Files:** Create `src/generators/dungeon/dress.rs`; modify `geom.rs`/`mod.rs` to place dressing.
**Test:** `dress.rs`.

**Interfaces:** `fn dungeon_prop(kind:DProp, pal:&Palette) -> Mesh` for `Pillar, TorchBracket(emissive), Door, Portcullis, Sarcophagus, Chest, Rubble`; `fn dress_room(room:&Room, theme, density:f32, seed:u64, pal) -> Vec<PlacedProp>` (deterministic placement; emissive torches as lighting cues). Reuse `prop.rs` barrel/crate/lantern/campfire.

- [ ] **Step 1: Failing test**

```rust
#[test]
fn dressing_is_deterministic_and_scales_with_density() {
    let room = /* a fixed Room */;
    let sparse = dress_room(&room, DungeonTheme::Crypt, 0.2, 1, &by_name("necrotic"));
    let dense  = dress_room(&room, DungeonTheme::Crypt, 0.9, 1, &by_name("necrotic"));
    assert!(dense.len() >= sparse.len());
    let again  = dress_room(&room, DungeonTheme::Crypt, 0.9, 1, &by_name("necrotic"));
    assert_eq!(dense.len(), again.len()); // deterministic
}
```

- [ ] **Step 2-4:** fail → implement `dress.rs` → pass.
- [ ] **Step 5: RENDER AND LOOK** — a dressed crypt room; torches glow, props sit on the floor, no overlap into walls.
- [ ] **Step 6: Commit** `feat(dungeon): dressing props + emissive torch lighting cues`.

### Task D6: `dungeon` + `validate-dungeon` CLI subcommands

**Files:** Modify `src/main.rs` (`enum Cmd` ~18-121; `run()` dispatch ~139-501). Mirror `World`/`ValidateWorld`.
**Test:** a CLI integration test (or `tests/`), plus manual run.

**Interfaces:** `Cmd::Dungeon { recipe:String, out:PathBuf, overview:bool, ... }` → parse `DungeonParams`, build `DungeonModel`, if ≤1 room write single GLB else `manifest::write_dir`, optional overview render. `Cmd::ValidateDungeon { dir:PathBuf }` → `manifest::validate_dir`.

- [ ] **Step 1: Failing test** — a `#[test]` invoking the handler function on a temp dir and asserting `manifest.json` exists + validates. (Follow how world's handler is testable; if only reachable via CLI, add a small `run_dungeon(params, out) -> Result` fn in main and unit-test that.)
- [ ] **Step 2-4:** fail → implement subcommands → pass.
- [ ] **Step 5: Manual** — `cargo run --release -- dungeon '{"kind":"dungeon","type":"crypt","size":"medium"}' -o /tmp/crypt && cargo run --release -- validate-dungeon /tmp/crypt` → prints OK.
- [ ] **Step 6: Commit** `feat(dungeon): dungeon + validate-dungeon subcommands`.

### Task D7: Schema help + determinism + hostile + edge-alignment tests

**Files:** Modify `src/main.rs` (`SCHEMA_HELP`); test in `recipe.rs`/`dungeon` modules.

- [ ] **Step 1: Tests**

```rust
#[test]
fn dungeon_is_deterministic() {
    let json = r#"{"kind":"dungeon","type":"cavern","seed":9}"#;
    let a = Recipe::parse(json).unwrap().build().unwrap();
    let b = Recipe::parse(json).unwrap().build().unwrap();
    assert_eq!(crate::gltf::to_bytes(&a).unwrap(), crate::gltf::to_bytes(&b).unwrap());
}
#[test]
fn dungeon_survives_hostile_input() {
    Recipe::parse(r#"{"kind":"dungeon","loops":1e9,"density":-5.0,"rooms":100000000}"#).unwrap().build().unwrap();
}
#[test]
fn dungeon_edges_are_integer_aligned() {
    let m = DungeonModel::new(&params(r#"{"kind":"dungeon","type":"crypt"}"#), &by_name("necrotic")).unwrap();
    for r in &m.rooms { for c in [r.min.x,r.min.z,r.max.x,r.max.z] { assert_eq!(c, c.round()); } }
}
```

- [ ] **Step 2-4:** fail → add clamps (rooms cap, loops/density clamp) → pass.
- [ ] **Step 3:** Add `SCHEMA_HELP` dungeon block (all fields, 6 themes, sizes, output-format note).
- [ ] **Step 5: Commit** `feat(dungeon): schema docs + determinism/hostile/alignment tests`.

### Task D8: Gallery hero — crypt

**Files:** Create `gallery/recipes/crypt.json`; run regen.
- [ ] **Step 1:** `{"kind":"dungeon","type":"crypt","palette":"necrotic","size":"medium","density":0.6}`.
- [ ] **Step 2:** Regenerate overview + a couple in-room renders.
- [ ] **Step 3: RENDER AND LOOK** — atmospheric, readable; manifest validates. ≥4/5 rubric.
- [ ] **Step 4: Commit** `feat(gallery): crypt hero dungeon`.

---

## Phase R — Docs, verification, release

### Task R1: Docs + CHANGELOG + version bump

**Files:** `README.md` (recipe gallery + a monster & dungeon row), `docs/site/*` (viewer model list, recipe rows, gallery grid — **no em-dashes**), `skill/imaginu/SKILL.md` (note the two new kinds; schema stays the reference), `CHANGELOG.md` (Unreleased → features), `Cargo.toml` (`version = "0.2.0"`).

- [ ] **Step 1:** Update `README.md` gallery with fire wyrm + crypt entries.
- [ ] **Step 2:** Update `docs/site/` viewer to load the new gallery GLBs; add recipe rows + grid tiles. Grep the changed files for em-dashes (`grep -n "—" docs/site/* docs/*.md`) — none allowed.
- [ ] **Step 3:** Update `skill/imaginu/SKILL.md` — add `monster` and `dungeon` to the kinds list, pointing at `imaginu schema`.
- [ ] **Step 4:** `CHANGELOG.md`: move to a `## [0.2.0]` section (monster: 8 plans + presets; dungeon: 6 themes + manifest; 3 palettes; `dungeon`/`validate-dungeon`).
- [ ] **Step 5:** `Cargo.toml` `version = "0.2.0"`.
- [ ] **Step 6: Commit** `docs: monster + dungeon in README, site, skill; bump v0.2.0`.

### Task R2: Full green bar + determinism re-verify

**Files:** none (verification).

- [ ] **Step 1:** `cargo fmt --check` → clean (else `cargo fmt` + commit).
- [ ] **Step 2:** `cargo clippy --all-targets -- -D warnings` → clean.
- [ ] **Step 3:** `cargo test` → all pass.
- [ ] **Step 4:** `cargo doc --no-deps` → clean.
- [ ] **Step 5: Determinism re-verify** — re-run the Task 0 hash script; every **existing** recipe's hash must equal the Task 0 baseline (existing kinds unchanged byte-for-byte). New kinds must be internally identical across two runs.
- [ ] **Step 6:** `gallery/regen.sh` runs clean (gallery still regenerates).
- [ ] **Step 7: Verification-before-completion** — invoke that skill; do not claim done until every command above shows the expected output. Commit any fmt-only changes.

### Task R3: Merge, tag, release

**Files:** none (release).

- [ ] **Step 1:** Ensure `phase-6` is green in CI (push branch, watch fmt/clippy/test + determinism on Linux & macOS).
- [ ] **Step 2:** Merge `phase-6` → `main` (per `finishing-a-development-branch` — confirm with user before merging/tagging).
- [ ] **Step 3:** Tag `v0.2.0`; `release.yml` builds the 5 targets + creates the GitHub Release; crates.io publish job runs (skips gracefully if token absent — publish locally via `cargo publish` if needed).
- [ ] **Step 4:** Confirm `pages.yml` redeploys the site with the new gallery.
- [ ] **Step 5:** Update memory (`imaginu-project.md`) with the phase-6 summary + any new gotchas.

---

## Self-review notes

- **Spec coverage:** monster (M1-M10) covers architecture/schema/8 plans/presets/knobs/anim/physics/gallery; dungeon (D1-D8) covers model/layout/geometry/manifest/dressing/CLI/themes/gallery; palettes (P1); integration + determinism + docs + release (M9/D7/R1-R3). Every spec section maps to a task.
- **Placeholders:** generator internals that require visual iteration (fold-order tuning, exact clip curves) are specified by algorithm + a mandatory RENDER-AND-LOOK gate rather than fabricated final float constants — this is correct for a rubric-driven visual pipeline, not a placeholder. Every test step has real assertion code.
- **Type consistency:** `MonsterRig`/`PrimitiveDesc`/`GaitDesc`/`build_rig`/`build_body`/`skin_body`/`build_clips`/`apply_preset` and `DungeonModel`/`Room`/`Corridor`/`Manifest`/`create`/`write_dir`/`validate_dir` are used consistently across tasks. `Asset`/`Mesh`/`Physics`/`gltf::to_bytes` names must be reconciled against the real module paths in Task M1 Step 4 (the first place they're touched) — adjust imports to match `character.rs`/`prop.rs`.
