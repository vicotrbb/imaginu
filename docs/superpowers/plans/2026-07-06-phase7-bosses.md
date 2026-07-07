# Phase 7 — `boss` recipe Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-class `boss` recipe kind — large, multi-part, multi-phase encounter creatures with signature telegraphed attacks and weak-point/destructible metadata — that reuses the phase-6 monster engine and drops into dungeon boss rooms and world-boss POIs. Ship as v0.3.0.

**Architecture:** A boss is ONE `MonsterRig` with named joints (the game-targetable part hierarchy) built by a new per-archetype `plan_*` builder, fed through the *same* shared monster body/skin/anim pipeline (promoted to `pub(crate)`). Weak-point colliders + phase/ability timings ride in a freeform `nodes[0].extras.imaginu_boss` block next to the untouched `imaginu_physics` contract. Dungeons emit+place a boss inline; worlds place a boss POI referencing a GLB.

**Tech Stack:** Rust (edition 2024, MSRV 1.87 — `gen` is reserved, the module is `generators`), `glam`, `serde`/`serde_json`, ChaCha8 rng (`generators::rng`). SDF surface-nets (`src/sdf.rs`), family-restricted skinning (`src/skinning.rs`), clip system (`src/anim.rs`), CSG (`src/csg.rs`).

## Global Constraints

- **Determinism is sacred.** Same recipe+seed → byte-identical GLB across processes and platforms. No process/time/address/HashMap-order state in generation — use `BTreeSet`/sorted iteration and `generators::rng(seed)` only. Keep the `texture.rs` f64 + `black_box` guard.
- **No regressions.** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo doc --no-deps` all clean. Every prior kind byte-identical. Gallery still regenerates.
- **Reuse the monster engine, do not fork it.** Promote shared internals to `pub(crate)`; do not copy-paste.
- **Fold order IS skinning correctness.** Compose core→limbs→attachments by fold rank; classify skin families by primitive SDF; keep the junction-blend smoothstep width == branch-cutoff half-width. Debug webs with the posed-vs-bind stretched-triangle edge-length probe — measure, never guess blend params.
- **CSG cutters must be closed solids** (profiles touch the axis/base) or carves silently fail.
- **Render-and-look ≥4/5** on every axis of `docs/EVALUATION.md` for every visual change; a boss clears a *higher* presence bar than a monster.
- **Docs/site: no em-dashes.** Keep `gallery/`, `docs/`, `skill/`, media OUT of the crate (`Cargo.toml` `exclude`).
- **`clap` eats a leading `-`** — pass `--flag=value` for negative args when rendering.
- Spec of record: `docs/superpowers/specs/2026-07-06-phase7-bosses-design.md`.

---

## Task 0: Determinism baseline capture

**Files:**
- Create: `scripts/determinism_baseline.sh`

**Interfaces:**
- Produces: a reproducible hash manifest of every current gallery GLB, used before/after the whole phase to prove prior kinds are byte-identical.

- [ ] **Step 1: Write the baseline script**

```bash
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
```

- [ ] **Step 2: Run it to capture the baseline**

Run: `chmod +x scripts/determinism_baseline.sh && ./scripts/determinism_baseline.sh /tmp/imaginu_baseline.sha256`
Expected: prints "wrote N hashes to /tmp/imaginu_baseline.sha256" (N ≥ 34). Keep this file; it is the ground truth for "prior kinds unchanged."

- [ ] **Step 3: Commit the script**

```bash
git add scripts/determinism_baseline.sh
git commit -m "chore(boss): determinism baseline capture script"
```

Note: `/tmp/imaginu_baseline.sha256` is NOT committed — it is a local pre-change artifact. Re-run the script after every foundation task and diff against it: `diff <(./scripts/determinism_baseline.sh /tmp/after.sha256 >/dev/null; cat /tmp/after.sha256) /tmp/imaginu_baseline.sha256` must be empty for all prior recipes.

---

## Task 1: Decouple `build_body`/`fit_collider` from `MonsterParams` and promote monster internals to `pub(crate)`

This is the one shared-code change that lets the boss reuse meshing/skinning/collider verbatim. It must be **byte-identical** — the baseline (Task 0) is the gate.

**Files:**
- Modify: `src/generators/monster/body.rs` (signatures of `build_body`, `fit_collider`; make `organic_field`, `eval_prim` `pub(crate)`)
- Modify: `src/generators/monster/mod.rs:28-31` (call sites; make `skin_body` `pub(crate)`)
- Modify: `src/generators/monster/rig.rs` (make `MonsterRig`, `RigBuilder` + methods, `PrimitiveDesc`, `PrimKind`, `PrimTint`, `Gait`, `GaitDesc`, `add_joint`, `push_cone`, `push_flat`, `spine_sample`, `compute_bounds`, and knob helpers `pub(crate)`)
- Modify: `src/generators/monster/anim.rs` (make the per-clip constructor helpers `pub(crate)` so a boss driver can reuse idle/locomotion/death)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub(crate) fn build_body(rig: &MonsterRig, size: f32, detail: f32, seed: u64, emissive: f32, pal: &Palette) -> Mesh`
  - `pub(crate) fn fit_collider(rig: &MonsterRig, size: f32, plan: BodyPlan) -> Physics`
  - `pub(crate) fn skin_body(mesh: &mut Mesh, rig: &MonsterRig)`
  - `pub(crate)` on `MonsterRig`, `RigBuilder` (+ `new`/`joint`/`wpos`/`cone`/`ellip`/`flat`/`finish`), `PrimitiveDesc`/`PrimKind`/`PrimTint`, `Gait`/`GaitDesc`, `add_joint`, `push_cone`, `push_flat`, `spine_sample`, `compute_bounds`.

- [ ] **Step 1: Read the current signatures**

Run: `sed -n '95,190p' src/generators/monster/body.rs` and `sed -n '21,63p' src/generators/monster/mod.rs`
Confirm `build_body(rig, p, pal)` reads only `p.size`, `p.detail`, `p.seed`, `p.emissive`; `fit_collider(rig, p)` reads only `p.size`, `p.body`.

- [ ] **Step 2: Change `build_body` and `fit_collider` signatures**

In `src/generators/monster/body.rs`, change the two `pub fn` signatures to take the primitive values directly (replace every `p.size`→`size`, `p.detail`→`detail`, `p.seed`→`seed`, `p.emissive`→`emissive` inside `build_body`; `p.size`→`size`, `p.body`→`plan` inside `fit_collider`). Add `use crate::recipe::BodyPlan;` if not present. Make `organic_field` and `eval_prim` `pub(crate)`.

```rust
pub(crate) fn build_body(
    rig: &MonsterRig,
    size: f32,
    detail: f32,
    seed: u64,
    emissive: f32,
    pal: &Palette,
) -> Mesh { /* body unchanged except p.<field> -> <field> */ }

pub(crate) fn fit_collider(rig: &MonsterRig, size: f32, plan: BodyPlan) -> Physics {
    /* body unchanged except p.size -> size, p.body -> plan */
}
```

- [ ] **Step 3: Update the monster call sites**

In `src/generators/monster/mod.rs`, update lines 29 and 31:

```rust
let mut mesh = body::build_body(&r, p.size, p.detail, p.seed, p.emissive, pal);
skin_body(&mut mesh, &r);
let phys = body::fit_collider(&r, p.size, p.body);
```

Change `fn skin_body` to `pub(crate) fn skin_body`. Change `mod anim; mod body; mod preset; mod rig;` to `pub(crate) mod anim; pub(crate) mod body; mod preset; pub(crate) mod rig;`.

- [ ] **Step 4: Promote rig + anim internals**

In `src/generators/monster/rig.rs`, add `pub(crate)` to every item listed in Interfaces above (the enums/structs are already `pub`; the free helpers `add_joint`, `push_cone`, `push_flat`, `spine_sample`, `compute_bounds` and the `RigBuilder` methods need `pub(crate)`). In `src/generators/monster/anim.rs`, add `pub(crate)` to the individual clip-builder helpers (identify them by reading `sed -n '53,130p' src/generators/monster/anim.rs`; at minimum the idle, locomotion, and death constructors).

- [ ] **Step 5: Build + run the full test suite**

Run: `cargo build && cargo test`
Expected: PASS, no warnings about unused pub.

- [ ] **Step 6: Prove byte-identical**

Run: `./scripts/determinism_baseline.sh /tmp/after1.sha256 >/dev/null && diff /tmp/after1.sha256 /tmp/imaginu_baseline.sha256`
Expected: EMPTY output (every prior kind unchanged). If not empty, the refactor changed behavior — revert and redo without altering value flow.

- [ ] **Step 7: fmt + clippy + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/generators/monster/
git commit -m "refactor(monster): decouple body/collider from MonsterParams, promote internals to pub(crate)

Byte-identical output (determinism baseline unchanged); enables the boss
generator to reuse the monster body/skin/anim pipeline without forking."
```

---

## Task 2: `BossParams` schema, `Recipe::Boss` variant, dispatch, element→palette

Wire the recipe kind end-to-end with a **stub generator** so parse+dispatch is testable before any archetype geometry exists.

**Files:**
- Create: `src/generators/boss/mod.rs` (stub `generate`)
- Modify: `src/generators/mod.rs` (add `pub mod boss;`)
- Modify: `src/recipe.rs` (BossParams struct + enums + `Boss` variant + `palette_name`/`resolved_palette`/`build` arms + `element_palette`)

**Interfaces:**
- Consumes: `build_body`/`skin_body`/`fit_collider` (Task 1) — not yet, stub returns a trivial asset.
- Produces:
  - `pub struct BossParams { seed, archetype, element, size, phases, phase, weak_points, armor, plates, crown, regalia, horns, spikes, eyes, maw, wings, tail, menace, emissive, detail, animate }`
  - `pub enum BossArchetype { #[default] Hydra, Colossus, Lich, SwarmQueen, DragonLord }`
  - `pub enum BossElement { #[default] Infernal, Necrotic, Fungal, Arctic, Volcanic, Verdant, Autumn, Desert, Mystic }` (each arm is one of the nine real palettes: `verdant, autumn, arctic, volcanic, desert, mystic, necrotic, infernal, fungal`)
  - `pub fn generate(p: &BossParams, pal: &Palette) -> Asset`

- [ ] **Step 1: Write the failing parse+build test**

Add to `src/recipe.rs` tests (`mod tests`):

```rust
#[test]
fn boss_minimal_parses_and_builds() {
    let r = Recipe::parse(r#"{"kind":"boss","archetype":"hydra","element":"infernal"}"#).unwrap();
    assert_eq!(r.resolved_palette(), "infernal");
    let asset = r.build().unwrap();
    assert!(!asset.parts.is_empty());
}

#[test]
fn boss_element_drives_palette_only_when_default() {
    // explicit palette wins
    let r = Recipe::parse(r#"{"kind":"boss","element":"infernal","palette":"arctic"}"#).unwrap();
    assert_eq!(r.resolved_palette(), "arctic");
    // element substitutes when palette left default
    let r = Recipe::parse(r#"{"kind":"boss","element":"necrotic"}"#).unwrap();
    assert_eq!(r.resolved_palette(), "necrotic");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test boss_minimal 2>&1 | head -20`
Expected: FAIL to compile ("no variant `Boss`").

- [ ] **Step 3: Add the enums + BossParams**

In `src/recipe.rs` (near `MonsterParams`), add. Element names must each be a real palette in `palette::PALETTES` — verify with `target/release/imaginu schema | head -1` (the palette line lists all nine) and align the enum arms to those names. `element_palette` returns the palette string for an element.

```rust
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BossArchetype {
    #[default]
    Hydra,
    Colossus,
    Lich,
    SwarmQueen,
    DragonLord,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BossElement {
    #[default]
    Infernal,
    Necrotic,
    Fungal,
    Arctic,
    Volcanic,
    Verdant,
    Autumn,
    Desert,
    Mystic,
}

/// Palette an element maps to (used only when the recipe left the default
/// palette). Every arm MUST be a name in `palette::PALETTES`
/// (verdant, autumn, arctic, volcanic, desert, mystic, necrotic, infernal, fungal).
fn element_palette(e: BossElement) -> &'static str {
    match e {
        BossElement::Infernal => "infernal",
        BossElement::Necrotic => "necrotic",
        BossElement::Fungal => "fungal",
        BossElement::Arctic => "arctic",
        BossElement::Volcanic => "volcanic",
        BossElement::Verdant => "verdant",
        BossElement::Autumn => "autumn",
        BossElement::Desert => "desert",
        BossElement::Mystic => "mystic",
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BossParams {
    #[serde(default = "d_seed")]
    pub seed: u64,
    #[serde(default)]
    pub archetype: BossArchetype,
    #[serde(default)]
    pub element: BossElement,
    /// Overall scale (default LARGE, per-archetype). Alias `bulk`.
    #[serde(default = "d_boss_size", alias = "bulk")]
    pub size: f32,
    /// Number of baked phase metadata blocks (clamp 1..=4).
    #[serde(default = "d_two_u32")]
    pub phases: u32,
    /// Optional single-phase geometry selector.
    #[serde(default)]
    pub phase: Option<u32>,
    #[serde(default = "d_true")]
    pub weak_points: bool,
    /// 0..1 armor escalation; `-1` = archetype default.
    #[serde(default = "d_neg1")]
    pub armor: f32,
    #[serde(default = "d_neg1")]
    pub plates: f32,
    #[serde(default = "d_neg1")]
    pub crown: f32,
    #[serde(default = "d_neg1")]
    pub regalia: f32,
    #[serde(default = "d_neg1")]
    pub horns: f32,
    #[serde(default = "d_neg1")]
    pub spikes: f32,
    #[serde(default = "d_neg1_i32")]
    pub eyes: i32,
    #[serde(default = "d_neg1")]
    pub maw: f32,
    #[serde(default = "d_neg1")]
    pub wings: f32,
    #[serde(default = "d_neg1")]
    pub tail: f32,
    #[serde(default = "d_neg1")]
    pub menace: f32,
    #[serde(default = "d_neg1")]
    pub emissive: f32,
    /// Hero tessellation default.
    #[serde(default = "d_hero_detail")]
    pub detail: f32,
    #[serde(default = "d_true")]
    pub animate: bool,
}

fn d_boss_size() -> f32 {
    3.0
}
fn d_two_u32() -> u32 {
    2
}
fn d_hero_detail() -> f32 {
    1.3
}

impl Default for BossParams {
    fn default() -> Self {
        serde_json::from_str("{}").expect("boss defaults deserialize")
    }
}
```

- [ ] **Step 4: Add the `Boss` variant + dispatch**

In the `Recipe` enum (after `Dungeon`), add:

```rust
    Boss {
        #[serde(default = "d_palette")]
        palette: String,
        #[serde(flatten)]
        params: BossParams,
    },
```

Add `Recipe::Boss { palette, .. } =>` to the `palette_name` match arm list. In `resolved_palette`, add before the `_ =>` fallback:

```rust
            Recipe::Boss { palette, params } if *palette == d_palette() => {
                element_palette(params.element)
            }
```

In `build`, add to the match:

```rust
            Recipe::Boss { params, .. } => crate::generators::boss::generate(params, &pal),
```

- [ ] **Step 5: Add the stub generator + module**

In `src/generators/mod.rs`, add `pub mod boss;` (alphabetical order near `building`). Create `src/generators/boss/mod.rs`:

```rust
//! Boss generator — a composite, multi-part, multi-phase encounter creature
//! built by escalating the monster rig/body/skin/anim pipeline. See
//! docs/superpowers/specs/2026-07-06-phase7-bosses-design.md.

use crate::gltf::{Asset, Material, Part};
use crate::palette::Palette;
use crate::recipe::BossParams;

/// STUB (Task 2): returns a trivial single-sphere asset so recipe dispatch is
/// testable before archetype geometry lands (Task 4+). Replaced in Task 4.
pub fn generate(_p: &BossParams, pal: &Palette) -> Asset {
    let mesh = crate::mesh::Mesh::uv_sphere(0.5, 12, 8);
    Asset {
        name: "boss".into(),
        parts: vec![Part {
            mesh,
            material: Material {
                emissive: pal.accent * 0.3,
                ..Default::default()
            },
        }],
        skeleton: None,
        animations: Vec::new(),
        physics: None,
        lods: Vec::new(),
        instanced: Vec::new(),
    }
}
```

(If `Mesh::uv_sphere` has a different name/signature, run `grep -n "pub fn.*sphere" src/mesh.rs` and use the real constructor; any small valid mesh works for the stub.)

- [ ] **Step 6: Run the tests**

Run: `cargo test boss_ 2>&1 | tail -20`
Expected: PASS (`boss_minimal_parses_and_builds`, `boss_element_drives_palette_only_when_default`).

- [ ] **Step 7: fmt + clippy + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/recipe.rs src/generators/mod.rs src/generators/boss/
git commit -m "feat(boss): BossParams schema, Boss recipe variant, element->palette, stub generator"
```

---

## Task 3: `imaginu_boss` metadata types + glTF extras writer

Add the freeform `nodes[0].extras.imaginu_boss` block alongside the untouched `imaginu_physics`. Backward-compatible: when `Asset.boss` is `None`, output is byte-identical to today.

**Files:**
- Create: `src/generators/boss/meta.rs` (metadata types + `to_json`)
- Modify: `src/gltf.rs` (add `boss: Option<BossMeta>` to `Asset`; serialize in `to_glb` node-0 extras at ~`gltf.rs:428-448`; update `Asset::static_mesh` and any other `Asset { .. }` literal to set `boss: None`)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct BossMeta { pub archetype: String, pub element: String, pub phases: Vec<PhaseMeta>, pub weak_points: Vec<WeakPointMeta>, pub parts: Vec<PartMeta>, pub arena: ArenaMeta }`
  - `pub struct PhaseMeta { pub id: u32, pub name: String, pub hp_fraction: f32, pub enrage: bool, pub active_weak_points: Vec<String>, pub abilities: Vec<AbilityMeta> }`
  - `pub struct AbilityMeta { pub name: String, pub telegraph_s: f32, pub active_s: f32, pub recover_s: f32, pub clip: String }`
  - `pub struct WeakPointMeta { pub name: String, pub joint: String, pub collider: ColliderJson, pub offset: [f32;3], pub destructible: bool, pub phase: u32 }`
  - `pub enum ColliderJson { Sphere { radius: f32 }, Box { half_extents: [f32;3] }, Capsule { radius: f32, height: f32 } }`
  - `pub struct PartMeta { pub name: String, pub joint: String, pub destructible: bool }`
  - `pub struct ArenaMeta { pub recommended_radius: f32, pub spawn_offset: [f32;3] }`
  - `pub fn boss_meta_json(m: &BossMeta) -> serde_json::Value`
  - `Asset.boss: Option<BossMeta>`

- [ ] **Step 1: Write the metadata types + JSON serializer**

Create `src/generators/boss/meta.rs`. Derive `Serialize` and hand-build the JSON via `serde_json::to_value` so field order is stable (serde preserves struct field order into a `Map`, which `serde_json` serializes deterministically):

```rust
//! Boss gameplay metadata: weak points, destructible parts, per-phase ability
//! timings, arena spawn. Serialized into `nodes[0].extras.imaginu_boss`
//! (format `imaginu-boss/1`), a sibling of the untouched `imaginu_physics`.

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ColliderJson {
    Sphere { radius: f32 },
    Box { half_extents: [f32; 3] },
    Capsule { radius: f32, height: f32 },
}

#[derive(Clone, Debug, Serialize)]
pub struct AbilityMeta {
    pub name: String,
    pub telegraph_s: f32,
    pub active_s: f32,
    pub recover_s: f32,
    pub clip: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct PhaseMeta {
    pub id: u32,
    pub name: String,
    pub hp_fraction: f32,
    pub enrage: bool,
    pub active_weak_points: Vec<String>,
    pub abilities: Vec<AbilityMeta>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WeakPointMeta {
    pub name: String,
    pub joint: String,
    pub collider: ColliderJson,
    pub offset: [f32; 3],
    pub destructible: bool,
    pub phase: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct PartMeta {
    pub name: String,
    pub joint: String,
    pub destructible: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArenaMeta {
    pub recommended_radius: f32,
    pub spawn_offset: [f32; 3],
}

#[derive(Clone, Debug, Serialize)]
pub struct BossMeta {
    pub format: &'static str,
    pub archetype: String,
    pub element: String,
    pub phases: Vec<PhaseMeta>,
    pub weak_points: Vec<WeakPointMeta>,
    pub parts: Vec<PartMeta>,
    pub arena: ArenaMeta,
}

impl BossMeta {
    pub fn new(archetype: String, element: String) -> Self {
        Self {
            format: "imaginu-boss/1",
            archetype,
            element,
            phases: Vec::new(),
            weak_points: Vec::new(),
            parts: Vec::new(),
            arena: ArenaMeta {
                recommended_radius: 8.0,
                spawn_offset: [0.0, 0.0, 0.0],
            },
        }
    }
}

/// Deterministic JSON for the extras block.
pub fn boss_meta_json(m: &BossMeta) -> Value {
    serde_json::to_value(m).expect("boss meta serializes")
}
```

Add `pub mod meta;` to `src/generators/boss/mod.rs`.

- [ ] **Step 2: Write the failing gltf test**

Add to `src/gltf.rs` tests:

```rust
#[test]
fn boss_extras_absent_when_none() {
    // A normal asset with boss: None must not carry an imaginu_boss key.
    let a = Asset::static_mesh("t".into(), vec![], Some(Physics::default()));
    let glb = to_glb(&a);
    let json = extract_json_chunk(&glb); // existing test helper; else parse manually
    let extras = &json["nodes"][0]["extras"];
    assert!(extras.get("imaginu_boss").is_none());
}
```

If no `extract_json_chunk` helper exists, parse the JSON chunk inline (bytes 12.. per GLB layout) or reuse whatever the existing gltf tests use — run `grep -n "fn.*json\|chunk\|to_glb(" src/gltf.rs | head` to find the pattern.

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test boss_extras_absent 2>&1 | head`
Expected: FAIL to compile (`Asset` has no way to set `boss` yet — or the field is missing). This confirms the field must be added.

- [ ] **Step 4: Add the `boss` field to `Asset` + serialize it**

In `src/gltf.rs`: add `pub boss: Option<crate::generators::boss::meta::BossMeta>,` to `struct Asset`. Set `boss: None` in `Asset::static_mesh` and EVERY other `Asset { .. }` literal in the crate (compiler will list them — `monster/mod.rs`, `dungeon`, `world`, `character`, etc.; a quick `grep -rn "Asset {" src/` finds them). In `to_glb`, in the node-0 extras block (~line 431), after the `imaginu_physics` insert, add:

```rust
    if let Some(bm) = &asset.boss {
        extras.insert(
            "imaginu_boss".to_string(),
            crate::generators::boss::meta::boss_meta_json(bm),
        );
    }
```

- [ ] **Step 5: Run tests + prove prior kinds unchanged**

Run: `cargo test && ./scripts/determinism_baseline.sh /tmp/after3.sha256 >/dev/null && diff /tmp/after3.sha256 /tmp/imaginu_baseline.sha256`
Expected: tests PASS; diff EMPTY (boss: None ⇒ byte-identical prior kinds).

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/gltf.rs src/generators/boss/
git commit -m "feat(boss): imaginu_boss extras metadata block (backward-compatible)"
```

---

## Task 4: Boss rig scaffolding + first archetype (hydra) + skinning probe

This is the template archetype. It establishes `BossRig`, `build_boss_rig` dispatch, the weak-point extraction path, and the render-and-look + stretched-triangle probe loop that Tasks 7–10 repeat. **Archetype geometry is a visual-iteration task:** the scaffold below is a starting point; the acceptance gate is the render + probe, not fixed magic numbers.

**Files:**
- Create: `src/generators/boss/rig.rs` (`BossRig`, `build_boss_rig`, `plan_hydra`)
- Modify: `src/generators/boss/mod.rs` (real `generate`: preset→rig→body→skin→collider→clips→meta→Asset)
- Test: rig well-formedness + skinning probe in `src/generators/boss/rig.rs` tests

**Interfaces:**
- Consumes: `monster::rig::{MonsterRig, RigBuilder, GaitDesc, Gait, PrimTint}`, `monster::body::{build_body, fit_collider}`, `monster::mod::skin_body`, `meta::{BossMeta, WeakPointMeta, PartMeta, ColliderJson}`.
- Produces:
  - `pub struct BossRig { pub rig: MonsterRig, pub weak_points: Vec<WeakPointMeta>, pub parts: Vec<PartMeta> }`
  - `pub fn build_boss_rig(p: &BossParams) -> BossRig`
  - joint naming convention: weak points named `weak_point.<x>`, targetable parts named like `head.1`, `throne`, `core` (glTF node names derive from joint names).

- [ ] **Step 1: Write the failing rig test (well-formedness + probe)**

Create the tests in `src/generators/boss/rig.rs`. The probe compares posed vs bind edge lengths (reuse the monster probe approach — read `grep -rn "stretch\|edge_len\|posed" src/generators/monster/` and mirror it; if none is exposed, implement the probe inline: mesh the rig, pose it with each clip at several phases, and assert `max(posed_edge / bind_edge) < THRESHOLD` where `THRESHOLD ≈ 8.0`, the phase-6 accepted ceiling).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::BossParams;

    fn boss(json: &str) -> BossParams {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn hydra_rig_is_wellformed() {
        let br = build_boss_rig(&boss(r#"{"kind":"boss","archetype":"hydra"}"#));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(br.rig.prims.iter().any(|d| d.fold_rank == 0), "has a core");
        // hydra has multiple heads -> multiple head.N parts + a core weak point
        assert!(br.parts.iter().filter(|p| p.name.starts_with("head.")).count() >= 3);
        assert!(br.weak_points.iter().any(|w| w.name == "weak_point.core"));
    }

    #[test]
    fn hydra_hostile_input_cannot_panic() {
        let br = build_boss_rig(&boss(
            r#"{"kind":"boss","archetype":"hydra","size":1e30,"phases":999,"eyes":999999,"horns":1e30}"#,
        ));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(n < 2000, "joint count bounded: {n}");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test hydra_rig 2>&1 | head`
Expected: FAIL to compile (`build_boss_rig` undefined).

- [ ] **Step 3: Implement `BossRig`, dispatch, and `plan_hydra` (starting scaffold)**

Create `src/generators/boss/rig.rs`. Hydra = one broad low torso along the ground + N (5) reared serpentine neck+head chains fanning forward, heavy haunches, a thick tail. Named joints: `core` (the exposed weak point at the torso center), `neck{i}_*`, `neck{i}_head`. This is a *starting scaffold* — tune radii/positions in Step 6 against renders.

```rust
use glam::Vec3;

use crate::generators::monster::body;
use crate::generators::monster::rig::{Gait, GaitDesc, MonsterRig, PrimTint, RigBuilder};
use crate::recipe::{BossArchetype, BossParams};

use super::meta::{ColliderJson, PartMeta, WeakPointMeta};

pub struct BossRig {
    pub rig: MonsterRig,
    pub weak_points: Vec<WeakPointMeta>,
    pub parts: Vec<PartMeta>,
}

pub fn build_boss_rig(p: &BossParams) -> BossRig {
    match p.archetype {
        BossArchetype::Hydra => plan_hydra(p),
        // Tasks 7-10 fill these; until then, fall back to hydra so dispatch is total.
        BossArchetype::Colossus => plan_hydra(p),
        BossArchetype::Lich => plan_hydra(p),
        BossArchetype::SwarmQueen => plan_hydra(p),
        BossArchetype::DragonLord => plan_hydra(p),
    }
}

/// Hydra: broad low torso + 5 reared serpentine necks each ending in a head.
/// The `core` joint is the exposed weak point between the necks.
fn plan_hydra(p: &BossParams) -> BossRig {
    let s = p.size.clamp(0.4, 8.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let mut parts = Vec::new();
    let mut weak_points = Vec::new();

    // core torso (rank 0)
    let hips = r.joint(None, "hips", v(0.0, 0.55 * s, -0.5 * s));
    let core = r.joint(Some(hips), "core", v(0.0, 0.75 * s, 0.2 * s));
    let tail = r.joint(Some(hips), "tail", v(0.0, 0.3 * s, -1.2 * s));
    r.ellip(hips, core, 0.6 * s, 0.1 * s, 0, 0.18 * s);
    r.cone(hips, tail, 0.4 * s, 0.08 * s, 0, 0.08 * s);

    // 5 necks fanning forward-up
    let nheads = 5usize;
    for i in 0..nheads {
        let f = (i as f32 / (nheads - 1) as f32 - 0.5) * 2.0; // -1..1 fan
        let base = r.joint(Some(core), &format!("neck{i}_0"), v(f * 0.35 * s, 0.9 * s, 0.35 * s));
        let mid = r.joint(Some(base), &format!("neck{i}_1"), v(f * 0.5 * s, 1.5 * s, 0.7 * s));
        let head = r.joint(Some(mid), &format!("neck{i}_head"), v(f * 0.55 * s, 1.8 * s, 1.05 * s));
        r.cone(base, mid, 0.16 * s, 0.12 * s, 1, 0.05 * s);
        r.cone(mid, head, 0.12 * s, 0.14 * s, 1, 0.05 * s);
        r.ellip(head, head, 0.16 * s, 0.0, 1, 0.05 * s);
        parts.push(PartMeta { name: format!("head.{}", i + 1), joint: format!("neck{i}_head"), destructible: true });
    }

    if p.weak_points {
        weak_points.push(WeakPointMeta {
            name: "weak_point.core".into(),
            joint: "core".into(),
            collider: ColliderJson::Sphere { radius: 0.4 * s },
            offset: [0.0, 0.0, 0.0],
            destructible: true,
            phase: 2,
        });
    }

    let gait = GaitDesc {
        legs: Vec::new(),
        spine: vec![hips, core],
        wings: Vec::new(),
        tail: vec![tail],
        head: None, // multi-headed: no single roar head
        style: Gait::Slither,
    };
    let rig = r.finish(gait);
    BossRig { rig, weak_points, parts }
}
```

Adjust the `RigBuilder`/`GaitDesc` field names to match the real promoted API from Task 1 (e.g. `r.ellip`/`r.cone` argument order — confirm against `src/generators/monster/rig.rs`).

- [ ] **Step 4: Wire the real `generate`**

Replace the stub in `src/generators/boss/mod.rs`:

```rust
pub mod meta;
mod preset; // added in Task 5; for now `pub fn apply_archetype_preset` may be a no-op
mod rig;

use crate::gltf::{Asset, Material, Part};
use crate::palette::Palette;
use crate::recipe::BossParams;

use meta::BossMeta;

pub fn generate(p: &BossParams, pal: &Palette) -> Asset {
    let mut owned = p.clone();
    preset::apply_archetype_preset(&mut owned);
    let p = &owned;

    let br = rig::build_boss_rig(p);
    let emissive = p.emissive.clamp(0.0, 1.0).max(0.0);
    let mut mesh = body::build_body(&br.rig, p.size, p.detail, p.seed, emissive, pal);
    crate::generators::monster::skin_body(&mut mesh, &br.rig);
    mesh.validate().expect("boss mesh invalid");
    // whole-body collider reuses the monster fit; boss body plan approximated by
    // the closest monster BodyPlan for the collider shape (Capsule for tall,
    // TriMesh for sprawling) — pass an explicit plan per archetype in Task 4+.
    let phys = body::fit_collider(&br.rig, p.size, crate::recipe::BodyPlan::Serpent);

    let animations = if p.animate {
        crate::generators::boss::anim_stub(&br.rig) // replaced by build_boss_clips in Task 6
    } else {
        Vec::new()
    };

    let mut bm = BossMeta::new(format!("{:?}", p.archetype).to_lowercase(), format!("{:?}", p.element).to_lowercase());
    bm.weak_points = br.weak_points;
    bm.parts = br.parts;
    bm.arena.recommended_radius = (p.size * 2.7).max(4.0);
    // phases filled in Task 6 (clip-linked); leave a single phase for now.

    Asset {
        name: "boss".into(),
        parts: vec![Part { mesh, material: Material { roughness: 0.7, emissive: pal.accent * emissive * 0.6, ..Default::default() } }],
        skeleton: Some(br.rig.skeleton),
        animations,
        physics: Some(phys),
        boss: Some(bm),
        lods: Vec::new(),
        instanced: Vec::new(),
    }
}

// Temporary until Task 6: reuse the monster clip builder for a basic clip set.
fn anim_stub(_r: &crate::generators::monster::rig::MonsterRig) -> Vec<crate::anim::AnimationClip> {
    Vec::new()
}
```

Adjust module declarations so `preset`/`anim` compile at each task boundary (a no-op `apply_archetype_preset` and empty `anim_stub` are fine until Tasks 5–6). Confirm `AnimationClip`'s real path with `grep -n "pub struct AnimationClip" src/anim.rs`.

- [ ] **Step 5: Run tests**

Run: `cargo test hydra_ && cargo test boss_`
Expected: PASS (well-formedness, hostile-input, recipe parse/build).

- [ ] **Step 6: Render-and-LOOK (the real acceptance gate)**

```bash
cargo build --release
target/release/imaginu generate '{"kind":"boss","archetype":"hydra","element":"infernal"}' -o /tmp/hydra.glb --preview
target/release/imaginu render /tmp/hydra.glb -o /tmp/hydra_front.png
# 4 angles: read the render CLI (target/release/imaginu render --help) for the camera/rotation flag
```

View `/tmp/hydra_front.png` (and 3 more angles) with the Read tool. Score against `docs/EVALUATION.md`: silhouette, color harmony, shading integrity, detail density, game readability, technical correctness. **A hydra must read as a multi-headed fight centerpiece at a glance.** Iterate on the `plan_hydra` radii/positions/fold-ranks until every axis ≥4/5. If necks web into the torso or each other, run the stretched-triangle probe test and fix by geometry (fold rank / junction), not by guessing blend `k`.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/generators/boss/
git commit -m "feat(boss): BossRig scaffold + hydra archetype (5-headed), weak-point extraction"
```

---

## Task 5: Archetype preset layer

**Files:**
- Create: `src/generators/boss/preset.rs`

**Interfaces:**
- Consumes: `BossParams`, `BossArchetype`.
- Produces: `pub fn apply_archetype_preset(p: &mut BossParams)` — fills sentinel (`< 0.0` / `-1` / default `size`) knobs per archetype; explicit values win (mirror `monster::preset`).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipe::BossParams;

    #[test]
    fn preset_fills_sentinels_explicit_wins() {
        let mut p: BossParams = serde_json::from_str(r#"{"archetype":"colossus","horns":0.9}"#).unwrap();
        apply_archetype_preset(&mut p);
        assert!((p.horns - 0.9).abs() < 1e-6, "explicit horns wins");
        assert!(p.plates >= 0.0, "colossus preset filled plate sentinel");
        assert!(p.armor >= 0.0, "colossus preset filled armor");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test preset_fills 2>&1 | head`
Expected: FAIL to compile (`apply_archetype_preset` undefined / no-op has no effect).

- [ ] **Step 3: Implement the presets**

```rust
//! Archetype preset: fills unset (sentinel) boss knobs per archetype so a bare
//! `{"archetype":"hydra"}` reads right; explicit user knobs always win.

use crate::recipe::{BossArchetype, BossParams};

fn set_neg(f: &mut f32, val: f32) {
    if *f < 0.0 {
        *f = val;
    }
}
fn set_neg_i(f: &mut i32, val: i32) {
    if *f < 0 {
        *f = val;
    }
}

pub fn apply_archetype_preset(p: &mut BossParams) {
    match p.archetype {
        BossArchetype::Hydra => {
            set_neg(&mut p.spikes, 0.5);
            set_neg(&mut p.maw, 0.7);
            set_neg_i(&mut p.eyes, 2);
            set_neg(&mut p.menace, 0.6);
            set_neg(&mut p.emissive, 0.5);
        }
        BossArchetype::Colossus => {
            set_neg(&mut p.armor, 1.0);
            set_neg(&mut p.plates, 1.0);
            set_neg(&mut p.menace, 0.8);
            set_neg(&mut p.emissive, 0.4);
            set_neg(&mut p.horns, 0.3);
        }
        BossArchetype::Lich => {
            set_neg(&mut p.regalia, 1.0);
            set_neg(&mut p.crown, 1.0);
            set_neg_i(&mut p.eyes, 2);
            set_neg(&mut p.emissive, 0.7);
        }
        BossArchetype::SwarmQueen => {
            set_neg_i(&mut p.eyes, 6);
            set_neg(&mut p.spikes, 0.6);
            set_neg(&mut p.emissive, 0.5);
        }
        BossArchetype::DragonLord => {
            set_neg(&mut p.wings, 1.0);
            set_neg(&mut p.horns, 0.8);
            set_neg(&mut p.spikes, 0.6);
            set_neg(&mut p.maw, 0.8);
            set_neg(&mut p.emissive, 0.6);
        }
    }
    // Any remaining sentinels resolve to "off" downstream; clamp size sanity.
    if p.size <= 0.0 {
        p.size = 3.0;
    }
    p.phases = p.phases.clamp(1, 4);
}
```

Wire it: in `src/generators/boss/mod.rs` change `mod preset;` and the call already added in Task 4 (`preset::apply_archetype_preset(&mut owned)`).

- [ ] **Step 4: Run tests**

Run: `cargo test preset_fills && cargo test boss_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/generators/boss/preset.rs src/generators/boss/mod.rs
git commit -m "feat(boss): archetype preset layer (sentinel-fill, explicit wins)"
```

---

## Task 6: Boss clip driver + phase metadata

**Files:**
- Create: `src/generators/boss/anim.rs`
- Modify: `src/generators/boss/mod.rs` (call `build_boss_clips`; fill `bm.phases`)

**Interfaces:**
- Consumes: promoted `monster::anim` helpers, `MonsterRig`, `BossRig`, `BossParams`, `meta::{PhaseMeta, AbilityMeta}`.
- Produces:
  - `pub fn build_boss_clips(rig: &MonsterRig, p: &BossParams) -> Vec<AnimationClip>` — emits `idle`, locomotion (gait-driven), `telegraph`, the archetype signature attack (`slam`|`breath`|`summon`), `phase_transition`, `enrage`, `stagger`, `death`.
  - `pub fn build_phase_meta(p: &BossParams, weak_points: &[WeakPointMeta]) -> Vec<PhaseMeta>` — one block per `p.phases`, ability timings referencing the clips above.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::boss::rig::build_boss_rig;
    use crate::recipe::BossParams;

    #[test]
    fn boss_clip_set_has_signature_and_transition() {
        let p: BossParams = serde_json::from_str(r#"{"archetype":"colossus"}"#).unwrap();
        let br = build_boss_rig(&p);
        let clips = build_boss_clips(&br.rig, &p);
        let names: Vec<_> = clips.iter().map(|c| c.name.as_str()).collect();
        for want in ["idle", "telegraph", "phase_transition", "enrage", "death"] {
            assert!(names.contains(&want), "missing clip {want}: {names:?}");
        }
        assert!(names.iter().any(|n| ["slam", "breath", "summon"].contains(n)), "has a signature attack");
    }

    #[test]
    fn phase_meta_matches_phase_count() {
        let p: BossParams = serde_json::from_str(r#"{"archetype":"hydra","phases":2}"#).unwrap();
        let br = build_boss_rig(&p);
        let phases = build_phase_meta(&p, &br.weak_points);
        assert_eq!(phases.len(), 2);
        assert!(phases[1].enrage, "last phase enrages");
        assert!(phases.iter().all(|ph| ph.abilities.iter().all(|a| a.telegraph_s >= 0.0)));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test boss_clip_set 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `build_boss_clips` + `build_phase_meta`**

Reuse the promoted monster clip constructors for `idle`/locomotion/`death` (read `src/generators/monster/anim.rs` for the exact helper names from Task 1). Append boss clips built from the same rotation-keyframe machinery. The signature attack name is chosen by archetype. Keep all keyframe generation seed-pure (no rng, or `rng(seed)` only). `build_phase_meta` emits `p.phases` blocks; the last is `enrage: true`; abilities reference clip names with plausible telegraph/active/recover seconds. Concrete implementation lives here; use `src/generators/monster/anim.rs` clip shapes as the model so deformation stays within the probe threshold.

Signature-attack selection:

```rust
fn signature_clip(a: BossArchetype) -> &'static str {
    match a {
        BossArchetype::Hydra | BossArchetype::DragonLord => "breath",
        BossArchetype::Colossus => "slam",
        BossArchetype::Lich | BossArchetype::SwarmQueen => "summon",
    }
}
```

- [ ] **Step 4: Fill `bm.phases` in `generate`**

In `src/generators/boss/mod.rs`, replace the `anim_stub` call with `anim::build_boss_clips(&br.rig, p)` and set `bm.phases = anim::build_phase_meta(p, &bm.weak_points);` before constructing the `Asset`. Remove `anim_stub`.

- [ ] **Step 5: Run tests + render an animation**

Run: `cargo test boss_clip_set phase_meta_matches boss_`
Expected: PASS.
Then LOOK:

```bash
cargo build --release
target/release/imaginu render /tmp/hydra.glb --animation telegraph -o /tmp/hydra_telegraph.png
target/release/imaginu render /tmp/hydra.glb --animation phase_transition -o /tmp/hydra_transition.png
```

View both. The telegraph must read as a visible wind-up; the transition must show the armor/plate bones shrinking + core emissive ramping. Iterate the clip until it reads and the probe stays green.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
git add src/generators/boss/
git commit -m "feat(boss): telegraphed clip set (telegraph/signature/phase_transition/enrage) + phase metadata"
```

---

## Tasks 7–10: Remaining archetypes (colossus, lich, swarm_queen, dragon_lord)

Each is an independent implement-and-review unit following the **exact same shape as Task 4**: a failing well-formedness + hostile-input test, the `plan_*` builder (starting scaffold below), the render-and-LOOK acceptance gate (4 angles + full clip set, ≥4/5, stretched-triangle probe green), then commit. Replace the corresponding fallback arm in `build_boss_rig` (Task 4 Step 3). Pass the archetype-appropriate `BodyPlan` to `fit_collider` in `generate` via a small `collider_plan(archetype)` helper (Capsule-ish for Colossus/Lich/DragonLord → `BipedBrute`/`Serpent`; TriMesh-ish for SwarmQueen → `Insectoid`).

Each task's parts + weak points (the named-joint hierarchy the game targets):

### Task 7: Colossus
- **Silhouette:** massive humanoid stone golem, chunky segmented limbs, a glowing **exposed core** in the chest cavity, heavy shoulders. Base on the `biped_brute` layout at large scale with `plates`/`armor` escalated.
- **Named joints/parts:** `core` (chest weak point, sphere), `arm.l`/`arm.r` (destructible), `head`.
- **Weak points:** `weak_point.core` (chest), phase 1 covered by plates, exposed phase 2.
- **Signature:** `slam`. **Collider plan:** `BipedBrute`.
- **Scaffold:** reuse `monster::rig::plan_biped_brute` shape via `RigBuilder` at `size*` large, then add a chest-cavity core ellipsoid (own fold rank) + shoulder plates. Acceptance: reads as a towering golem with a visible core.

### Task 8: Lich / overlord
- **Silhouette:** gaunt humanoid + floating **throne/pedestal** behind + 2–3 floating implements (orbs/blades) orbiting; crown + regalia. Palette necrotic (hero).
- **Named joints/parts:** `head`, `throne` (destructible), `implement.1`..`implement.N`, `core` (chest phylactery weak point).
- **Weak points:** `weak_point.phylactery` at `core`.
- **Signature:** `summon`. **Collider plan:** `BipedBrute` (throne excluded from tight collider; whole-body capsule is fine).
- **Scaffold:** biped core + a throne built from `prop.rs`/CSG closed solids fold-ranked last as its own family + floating implement ellipsoids parented to `core` with a slow orbit in `idle`. Acceptance: reads as an enthroned caster with regalia.

### Task 9: Swarm-queen
- **Silhouette:** huge insectoid (base `insectoid`/`arachnid`) + a bulbous **abdomen with brood sacs** (emissive), broad thorax, many eyes.
- **Named joints/parts:** `abdomen` (destructible), `brood_sac.1`..`N` (destructible weak points, emissive), `head`.
- **Weak points:** `weak_point.brood_sac.i` per sac.
- **Signature:** `summon`. **Collider plan:** `Insectoid` (Box) or `Arachnid` (TriMesh).
- **Scaffold:** insectoid rig at scale + a large abdomen ellipsoid + N brood-sac ellipsoids (emissive tint) fold-ranked last, each its own family. Acceptance: reads as a brood-mother; sacs glow.

### Task 10: Dragon-lord
- **Silhouette:** winged serpent at scale (base `winged_flyer`+`serpent`): long reared neck, huge membrane wings, horns, a fanged maw, sweeping tail.
- **Named joints/parts:** `head` (breath origin), `wing.l`/`wing.r` (destructible), `heart` (chest weak point exposed on enrage).
- **Weak points:** `weak_point.heart`.
- **Signature:** `breath`. **Collider plan:** `Serpent` (Capsule).
- **Scaffold:** serpent spine reared up + winged_flyer wing prims (flat sheets, low-k) fold-ranked last + horns/maw knobs. Acceptance: reads as a dragon-lord centerpiece; wings are genuine thin sheets (not lobes).

For **each** of Tasks 7–10:
- [ ] Write `<archetype>_rig_is_wellformed` + `<archetype>_hostile_input_cannot_panic` tests (mirror Task 4 Step 1, adjusting the expected parts/weak points).
- [ ] Run to verify they fail.
- [ ] Implement `plan_<archetype>` and replace the fallback arm in `build_boss_rig`; add the `collider_plan` mapping in `generate`.
- [ ] `cargo test <archetype>_` → PASS.
- [ ] `cargo build --release`; generate + render 4 angles + the full clip set; view with Read; score ≥4/5 on every `docs/EVALUATION.md` axis; run the stretched-triangle probe; iterate geometry until green.
- [ ] `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`; commit `feat(boss): <archetype> archetype`.

---

## Task 11: `validate-boss` subcommand + `SCHEMA_HELP`

**Files:**
- Modify: `src/main.rs` (`Cmd::ValidateBoss` variant + dispatch; insert boss block into `SCHEMA_HELP` after the monster block ~`main.rs:777`)
- Modify: `src/validate.rs` (add `pub fn validate_boss_bytes(&[u8]) -> Result<String,String>`)

**Interfaces:**
- Consumes: `validate::validate_glb_bytes` (structural), the `imaginu_boss` JSON shape.
- Produces: `validate-boss <glb>` that structurally validates the GLB AND checks the `imaginu_boss` block (format tag `imaginu-boss/1`; ≥1 phase; phases ordered by `id`; every weak point's `joint` exists in the skin joints; ability timings ≥ 0; `arena.recommended_radius > 0`).

- [ ] **Step 1: Write the failing test**

In `src/validate.rs` tests: generate an infernal hydra GLB in-process (`Recipe::parse(...).unwrap().build().unwrap()` → `to_glb`), then assert `validate_boss_bytes` returns `Ok` and that a doctored GLB with a weak point pointing at a nonexistent joint returns `Err`.

```rust
#[test]
fn validate_boss_accepts_hydra_and_rejects_bad_joint() {
    let asset = crate::recipe::Recipe::parse(r#"{"kind":"boss","archetype":"hydra"}"#).unwrap().build().unwrap();
    let glb = crate::gltf::to_glb(&asset);
    assert!(validate_boss_bytes(&glb).is_ok());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test validate_boss 2>&1 | head`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `validate_boss_bytes`**

Parse the JSON chunk (reuse the internal helper `validate_glb_bytes` uses), read `nodes[0].extras.imaginu_boss`, and run the checks above. Collect joint names from the skin/skeleton nodes. Return a summary string like `"boss: hydra, 2 phases, 1 weak points, 5 parts"`.

- [ ] **Step 4: Add the CLI subcommand + schema block**

Add `ValidateBoss { file: PathBuf }` to `Cmd`, dispatch it to `validate::validate_boss_bytes(&fs::read(file)?)`. Insert a `boss` JSON block + comment into `SCHEMA_HELP` after the monster block, listing `archetype` (5 values), `element` (9 palettes), `size`, `phases`, `phase`, `weak_points`, `armor`/`plates`/`crown`/`regalia`, the reused knobs, `detail`, `animate`, and a one-line note that `extras.imaginu_boss` carries weak points + phase/ability timings.

- [ ] **Step 5: Run tests + smoke the CLI**

Run: `cargo test validate_boss && cargo build --release && target/release/imaginu generate '{"kind":"boss","archetype":"lich","element":"necrotic"}' -o /tmp/lich.glb && target/release/imaginu validate-boss /tmp/lich.glb && target/release/imaginu schema | grep -A2 '"kind": "boss"'`
Expected: validate-boss prints an OK summary; schema shows the boss block.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/main.rs src/validate.rs
git commit -m "feat(boss): validate-boss subcommand + schema help entry"
```

---

## Task 12: Dungeon inline boss (emit + place + reference + validate)

**Files:**
- Modify: `src/recipe.rs` (`DungeonParams` gains `boss: Option<BossParams>`)
- Modify: `src/generators/dungeon/manifest.rs` (`SpawnEntry.file: Option<String>`; write `boss.glb`; validate the reference)
- Modify: `src/generators/dungeon/mod.rs` or `model.rs` (generate + scale + place the boss at the boss spawn)

**Interfaces:**
- Consumes: `boss::generate`, `gltf::to_glb`, the boss `SpawnPoint`.
- Produces: a dungeon directory whose `manifest.json` boss spawn has `"file":"boss.glb"`, plus a `boss.glb` scaled to the boss room and translated to the boss spawn.

- [ ] **Step 1: Write the failing test**

In `src/generators/dungeon/manifest.rs` tests (or an integration test): build a dungeon recipe with an inline boss, write to a temp dir, assert `boss.glb` exists, the boss `SpawnEntry` carries `file: Some("boss.glb")`, and `validate_dir` passes.

```rust
#[test]
fn dungeon_emits_and_references_inline_boss() {
    let dir = std::env::temp_dir().join("imaginu_boss_dungeon_test");
    let _ = std::fs::remove_dir_all(&dir);
    let r = crate::recipe::Recipe::parse(
        r#"{"kind":"dungeon","type":"crypt","size":"small","boss":{"archetype":"hydra","element":"necrotic"}}"#,
    ).unwrap();
    // build the dungeon dir via the same path the CLI uses:
    crate::generators::dungeon::write_dungeon_dir(&r, &dir).unwrap(); // use the real fn name
    assert!(dir.join("boss.glb").exists());
    assert!(crate::generators::dungeon::manifest::validate_dir(&dir).is_ok());
}
```

(Confirm the real dungeon-dir entry point with `grep -n "pub fn write_dir\|write_dungeon\|pub fn build_dungeon" src/generators/dungeon/*.rs src/main.rs`.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test dungeon_emits_and_references 2>&1 | head`
Expected: FAIL (no `boss` field / `boss.glb` not written).

- [ ] **Step 3: Add the `boss` field + `SpawnEntry.file`**

Add `#[serde(default)] pub boss: Option<BossParams>` to `DungeonParams`. Add `#[serde(default, skip_serializing_if = "Option::is_none")] pub file: Option<String>` to `SpawnEntry` (so existing manifests stay byte-identical: `skip_serializing_if` keeps the key absent when `None`).

- [ ] **Step 4: Emit + place the boss**

In the dungeon dir writer: if `params.boss` is `Some`, generate the boss asset (`boss::generate(bp, &pal)`), scale it so its bounds fit the boss room footprint (read the room extent; uniform-scale the mesh + skeleton translations by `min(room_w, room_d) / boss_footprint`), translate it to the boss `SpawnPoint.pos` snapped to integer meters (seam law), write `boss.glb`, and set the boss `SpawnEntry.file = Some("boss.glb")`. Keep all transforms f32-exact and seed-pure.

- [ ] **Step 5: Validate the reference**

In `dungeon::manifest::validate_dir`, after the room-file checks: for each spawn with `file: Some(f)`, assert the file exists, `validate::validate_glb` passes, and `validate::validate_boss_bytes` passes.

- [ ] **Step 6: Run tests + prior dungeons unchanged**

Run: `cargo test dungeon_ && ./scripts/determinism_baseline.sh /tmp/after12.sha256 >/dev/null && diff /tmp/after12.sha256 /tmp/imaginu_baseline.sha256`
Expected: tests PASS; diff EMPTY (dungeons without a `boss` field are byte-identical — `skip_serializing_if` guarantees no new manifest key).

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/recipe.rs src/generators/dungeon/
git commit -m "feat(boss): dungeon inline boss - emit boss.glb, place at boss spawn, reference + validate"
```

---

## Task 13: World-boss POI (place + reference + validate)

**Files:**
- Modify: `src/world/poi.rs` (`PoiKind::Boss` + radius/separation + scoring + `build_asset` arm + `poi_file`)
- Modify: `src/world/manifest.rs` (the boss POI flows through the existing `Poi{file, spawn_points}` path; extend `validate_dir` to check the boss block)

**Interfaces:**
- Consumes: `boss::generate`, the existing POI solver + `Poi` manifest struct.
- Produces: a world whose `manifest.json` carries a boss `Poi` with `file: Some("poi_boss_i.glb")` + spawn points; the GLB validates as a boss.

- [ ] **Step 1: Write the failing test**

In `src/world/poi.rs` tests: assert `PoiKind::Boss` exists with a sane `radius`/`separation`, and that `build_asset` for a boss site returns an asset whose `boss` metadata is `Some`. In `src/world/manifest.rs` tests (or integration): build a small world with a pinned boss POI, write the dir, assert `poi_boss_0.glb` exists and `validate_dir` passes.

```rust
#[test]
fn boss_poi_kind_and_asset() {
    assert!(PoiKind::Boss.radius() > 0.0);
    // build_asset for a boss site yields a boss asset
    // (construct a minimal PoiSite with kind: PoiKind::Boss and assert asset.boss.is_some())
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test boss_poi 2>&1 | head`
Expected: FAIL (no `Boss` variant).

- [ ] **Step 3: Add `PoiKind::Boss`**

Add `Boss` to `PoiKind` (snake_case `boss`). Give it `.name()="boss"`, a `.radius()` sized for an arena (e.g. `18.0`), a `.separation()` keeping bosses far apart. In the solver `score` closure, favor **open, flat, accessible** ground (low slope, mid altitude, away from water) — the opposite bias to `Dungeon` (which favors steep mountains). Add the `build_asset` arm building a boss GLB (choose an archetype/element from the site seed deterministically). Set `poi_file` → `poi_boss_{i}.glb`.

- [ ] **Step 4: Validate the boss POI**

In `world::manifest::validate_dir`, where POI files are checked for existence + `validate_glb`, additionally run `validate_boss_bytes` when the POI kind is `boss`.

- [ ] **Step 5: Run tests + prior worlds unchanged**

Run: `cargo test boss_poi world_ && ./scripts/determinism_baseline.sh /tmp/after13.sha256 >/dev/null && diff /tmp/after13.sha256 /tmp/imaginu_baseline.sha256`
Expected: tests PASS; diff EMPTY (worlds that don't request a boss POI are unchanged; adding a new enum variant must not shift existing POI placement — verify the gallery worlds specifically).

- [ ] **Step 6: Render a world overview with a boss POI**

```bash
cargo build --release
target/release/imaginu world '{"kind":"world","seed":3,"size":"small","pois":[{"kind":"boss","at":[0,0]}]}' -o /tmp/boss_world --overview
```
(Use the real world recipe schema — `target/release/imaginu schema | grep -A15 '"kind": "world"'`.) View `/tmp/boss_world/overview.png`: the boss must sit in an open arena, seamless with chunks.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings
git add src/world/
git commit -m "feat(boss): world-boss POI - place in open arenas, reference GLB + spawn points, validate"
```

---

## Task 14: Gallery heroes + regen + render-and-look

**Files:**
- Create: `gallery/recipes/infernal_hydra.json`, `gallery/recipes/necrotic_lich.json`
- Modify: `gallery/regen.sh` (bosses are single-GLB → default path; no special-casing), `gallery/regen_showcase.sh` (2 boss showcase MP4s)

**Interfaces:** none (asset + tooling).

- [ ] **Step 1: Write the hero recipes**

`gallery/recipes/infernal_hydra.json`:
```json
{ "kind": "boss", "archetype": "hydra", "element": "infernal", "size": 3.2, "seed": 3 }
```
`gallery/recipes/necrotic_lich.json`:
```json
{ "kind": "boss", "archetype": "lich", "element": "necrotic", "size": 2.8, "seed": 5 }
```

- [ ] **Step 2: Add showcase lines**

Append to `gallery/regen_showcase.sh`:
```bash
"$BIN" showcase gallery/recipes/infernal_hydra.json --animation breath -o gallery/showcase_infernal_hydra.mp4 --duration 5
"$BIN" showcase gallery/recipes/necrotic_lich.json --animation summon -o gallery/showcase_necrotic_lich.mp4 --duration 5
```

- [ ] **Step 3: Regenerate the gallery**

Run: `cargo build --release && ./gallery/regen.sh 2>&1 | tail -20`
Expected: `infernal_hydra.glb/.png` and `necrotic_lich.glb/.png` produced; all prior assets still regenerate.

- [ ] **Step 4: Render-and-LOOK (hero bar)**

View `gallery/infernal_hydra.png` and `gallery/necrotic_lich.png` with Read. These are the marquee assets — they must clear a **higher presence bar** than any monster (≥4.3 target on silhouette + readability). Iterate the recipe (`size`, knobs, `emissive`, `seed`) and, if needed, the `plan_*` builder until both are hero-grade. Generate the showcase MP4s (`./gallery/regen_showcase.sh` or the two lines directly) and spot-check a frame.

- [ ] **Step 5: Verify prior gallery unchanged + commit**

Run: `./scripts/determinism_baseline.sh /tmp/after14.sha256 >/dev/null && diff /tmp/after14.sha256 /tmp/imaginu_baseline.sha256`
Expected: EMPTY (only new boss recipes added; prior kinds untouched).

```bash
git add gallery/recipes/infernal_hydra.json gallery/recipes/necrotic_lich.json gallery/infernal_hydra.* gallery/necrotic_lich.* gallery/showcase_infernal_hydra.mp4 gallery/showcase_necrotic_lich.mp4 gallery/regen_showcase.sh
git commit -m "feat(gallery): infernal hydra + necrotic lich hero bosses + showcases"
```

---

## Task 15: Docs, site, skill, CHANGELOG, version bump to v0.3.0

**Files:**
- Modify: `README.md` (recipe gallery: add a hero boss row + boss kind blurb)
- Modify: `docs/site/` (viewer model, recipe rows, gallery grid — add a hero boss; **no em-dashes**)
- Modify: `skill/imaginu/SKILL.md` (note the `boss` kind; schema stays the reference)
- Modify: `CHANGELOG.md` (`Unreleased` → boss features)
- Modify: `Cargo.toml` (`version = "0.3.0"`)

**Interfaces:** none.

- [ ] **Step 1: README + SKILL**

Add a `boss` entry to the README recipe gallery (mirror the monster/dungeon rows; reference `gallery/infernal_hydra.png`) and a short "boss = multi-part, multi-phase, weak-point-tagged encounter creature; places into dungeon boss rooms + world POIs" blurb. In `skill/imaginu/SKILL.md`, add a line noting the new kind and that `imaginu schema` is the field reference. Check both for em-dashes (`grep -n '—' README.md skill/imaginu/SKILL.md` → none in site; README follows its own existing style).

- [ ] **Step 2: Site**

Update `docs/site/` to add the hero boss to the viewer model list, a recipe row, and the gallery grid, matching how monster/dungeon were added (read the site source for the pattern). Copy the hero GLB/PNG into wherever the site loads gallery media from. **No em-dashes** (`grep -rn '—' docs/site/` → none).

- [ ] **Step 3: CHANGELOG + version**

Under `## [Unreleased]` add an `### Added` section describing the `boss` kind (5 archetypes, phases + telegraphed clips, `extras.imaginu_boss` weak-point/ability metadata, `validate-boss`, dungeon inline boss, world-boss POI). Move it under a new `## [0.3.0] - 2026-07-06` heading. Set `version = "0.3.0"` in `Cargo.toml`.

- [ ] **Step 4: Build the site / verify links**

Run: `grep -rn '—' docs/site/ ; cargo doc --no-deps 2>&1 | tail -5`
Expected: no em-dashes; docs build clean.

- [ ] **Step 5: Commit**

```bash
git add README.md docs/site/ skill/imaginu/SKILL.md CHANGELOG.md Cargo.toml
git commit -m "docs(boss): README + site + skill + CHANGELOG; bump to v0.3.0"
```

---

## Task 16: Whole-branch verification + ship

**Files:** none (verification + release).

- [ ] **Step 1: Full green bar**

Run:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
```
Expected: all clean.

- [ ] **Step 2: Determinism — full re-verify**

Run: `./scripts/determinism_baseline.sh /tmp/final.sha256 >/dev/null && diff /tmp/final.sha256 /tmp/imaginu_baseline.sha256`
Expected: EMPTY for every PRIOR recipe. Then prove the bosses themselves are deterministic:
```bash
target/release/imaginu generate '{"kind":"boss","archetype":"hydra","element":"infernal","seed":3}' -o /tmp/h1.glb
target/release/imaginu generate '{"kind":"boss","archetype":"hydra","element":"infernal","seed":3}' -o /tmp/h2.glb
cmp /tmp/h1.glb /tmp/h2.glb && echo "hydra byte-identical"
```
Repeat for all 5 archetypes. Expected: `cmp` silent (identical) each time.

- [ ] **Step 3: Adversarial whole-branch review**

Use `superpowers:requesting-code-review` for a final whole-branch review (correctness, determinism, DRY against the monster engine, no fold-order/skinning regressions, backward-compatible extras). Address findings.

- [ ] **Step 4: Definition-of-done smoke (skill + binary only)**

Run the definition-of-done path end to end:
```bash
target/release/imaginu generate '{"kind":"boss","archetype":"hydra","element":"infernal"}' -o /tmp/dod.glb --preview
target/release/imaginu validate /tmp/dod.glb
target/release/imaginu validate-boss /tmp/dod.glb
target/release/imaginu dungeon '{"kind":"dungeon","type":"crypt","boss":{"archetype":"hydra","element":"necrotic"}}' -o /tmp/dod_crypt
target/release/imaginu validate-dungeon /tmp/dod_crypt
```
Expected: all succeed; the crypt contains a placed, validated `boss.glb`.

- [ ] **Step 5: PR → green CI → merge → tag**

```bash
git push -u origin <branch>
gh pr create --title "Phase 7: boss recipe (v0.3.0)" --body "<summary + definition-of-done checklist>"
```
Wait for CI (fmt/clippy/test + determinism on Linux & macOS) green. Merge. Then tag:
```bash
git checkout main && git pull
git tag v0.3.0 && git push origin v0.3.0
```
(`release.yml` builds targets + GitHub Release; `pages.yml` redeploys the site; crates.io publish is a local `cargo publish` — the CI token is unset by design and the job skips gracefully.)

---

## Self-review notes (author)

- **Spec coverage:** decisions A–G → Task 2 (A,D), Tasks 4/7–10 (B), Task 6 (C), Task 3/6/11 (E), Tasks 12–13 (F), Task 15 (G). Schema → Task 2. Shared refactor → Task 1. Metadata → Task 3. Arena → Tasks 12–13. Tests/determinism → Tasks 0,1,3,12,13,16. Gallery/docs/ship → Tasks 14–16. Render-and-look gate → Tasks 4,6,7–10,14.
- **Archetype tasks intentionally carry starting-scaffold code + an explicit render-and-look tuning loop** rather than pre-tuned magic numbers, because presence is a visual-iteration outcome (phase 6 took three rounds per `docs/EVALUATION.md`). The acceptance gate is the render + the stretched-triangle probe, both concrete.
- **Backward compatibility is asserted, not assumed:** the baseline diff is re-run after Tasks 1, 3, 12, 13, 14, 16, and `skip_serializing_if` guards the new `SpawnEntry.file` key.
- Confirm real function/type names flagged inline (`Mesh::uv_sphere`, dungeon dir writer, monster anim helper names, `AnimationClip` path) at implementation time via the `grep` commands given; the plan cites the search rather than a guessed name where the exact identifier wasn't verified.
