# imaginu Phase 7 — the `boss` recipe (design)

> Status: approved for planning (2026-07-06). Ships as **v0.3.0**.
> Companion brief: `docs/superpowers/plans/2026-07-06-phase7-bosses-brief.md`.

A `boss` is a first-class recipe kind: a large, multi-part, multi-phase encounter
creature with signature telegraphed attacks, weak-point/destructible gameplay
metadata, and first-class placement into dungeons (boss rooms) and worlds
(world-boss POIs). It must read *instantly* as the centerpiece of a fight, not a
big monster. It is a **composition and escalation of the phase-6 monster engine,
not a second engine.**

---

## 1. Resolved decisions (brief A–G)

| # | Decision | Resolution |
|---|---|---|
| A | Kind vs. flag | **Distinct `kind:"boss"`** that internally reuses the monster engine. |
| B | Archetype set | **Full 5**: `hydra`, `colossus`, `lich`, `swarm_queen`, `dragon_lord`. `archetype` doubles as the preset (fills sentinel knobs; explicit knobs win). |
| C | Phase model | Bake **metadata for 2 phases** + a `phase_transition` clip + `enrage` clip into one asset (default geometry = phase 1). Optional `phase:2` override regenerates the exposed silhouette. |
| D | Palettes | **Reuse the nine**, driven by `element`; emissive telegraph color = palette accent. Add a dedicated palette only if a read is missing (deferred). |
| E | Weak-point / metadata | **Named per-part colliders + `extras.imaginu_boss`** block (weak points, destructible parts, per-phase ability timings, arena spawn). |
| F | Arena coupling | **Both** — dungeon references a boss by file in the manifest AND an optional inline `boss` field on the dungeon recipe that generates + places it. |
| G | Release | **v0.3.0** on its own. |

## 2. Gallery heroes

- **Infernal hydra** (the marquee example; matches the "infernal hydra for my crypt" definition-of-done).
- **Necrotic lich/overlord** (humanoid + throne + floating implements).

Both get a `--preview` PNG and a showcase MP4. Every one of the 5 archetypes must
render convincingly from 4 angles at ≥4/5 on `docs/EVALUATION.md` (a boss clears a
**higher presence bar** than a monster).

---

## 3. Schema — `BossParams`

Serde-defaulted exactly like `MonsterParams` (each field has a `#[serde(default =
...)]`; `impl Default = from_str("{}")`). Added to the `Recipe` enum as
`Boss { palette: String (default d_palette), #[serde(flatten)] params: BossParams }`.

```jsonc
{ "kind": "boss",
  "seed": 1,
  "archetype": "hydra",              // hydra|colossus|lich|swarm_queen|dragon_lord (default hydra)
  "element": "infernal",             // drives palette + emissive telegraph color; default matches archetype
  "palette": "verdant",              // explicit palette wins over element substitution
  "size": 3.0,                       // (alias "bulk", as in monster) default LARGE; boss clamp raised to 8.0 (monster clamps at 4.0)
  "phases": 2,                       // number of baked phase metadata blocks (clamp 1..=4, default 2)
  "phase": null,                     // optional u32 selector: regenerate a single-phase geometry variant
  "weak_points": true,               // emit named weak-point colliders + metadata
  "armor": 1.0, "plates": -1.0,      // armor/plate escalation (sentinel -> archetype default)
  "crown": -1.0, "regalia": -1.0,    // crown/weapon/throne attachments (sentinel -> archetype default)
  "horns": -1.0, "spikes": -1.0, "eyes": -1, "maw": -1.0, "wings": -1.0, "tail": -1.0,
  "menace": -1.0, "emissive": -1.0,  // reused monster knobs (sentinel -> archetype/preset default)
  "detail": 1.3,                     // hero tessellation default (monster default 1.0)
  "animate": true }
```

`element` -> palette map (only substituted when `palette` was left at the default,
mirroring `Recipe::resolved_palette` / `preferred_palette`): `infernal`->infernal,
`necrotic`->necrotic, `fungal`->fungal, and the remaining elements map to the
closest of the nine existing palettes with a documented fallback. The emissive
telegraph color is the resolved palette's accent.

`archetype` is the **preset**: it fills sentinel knobs and picks defaults
(body plan, size, armor, regalia, signature attack). Explicit knobs override,
same contract as `monster` `class` (`preset::apply_preset`).

### Boss size / mass

Boss `size` clamp is raised to `8.0` (monster clamps `0.2..=4.0`). Default `size`
is large (per archetype, ~2.5–3.5). Mass scales as `~ size^3` (reusing the monster
collider mass model), so bosses are very heavy. The rasterizer near/far auto-fits
bounds (do NOT hard-code a far plane) — bounds come from the rig, as for monsters.

---

## 4. Architecture — reuse, don't fork

### 4.1 A boss is ONE `MonsterRig`

The monster rig is already a flexible joint + fold-ranked-primitive graph with:
- automatic union-find **skin families** (`skin_body`, classified by primitive SDF),
- glTF **node names derived from joint names**,
- composable **feature knobs** folded LAST as their own rigid skin families.

So a boss archetype is a **new `plan_*`-style builder producing a large,
multi-part `MonsterRig` with named joints** — a hydra is one torso + N neck+head
chains; a lich is a humanoid + throne prims + floating implements; a colossus is
chunky stone segments + an exposed core; a swarm-queen is an insectoid + brood
sacs; a dragon-lord is a winged serpent at scale. The "named part hierarchy the
game can target" **= the named joints** (e.g. `head.1`…`head.N`, `weak_point.core`,
`throne`). Skinning separates them into families automatically. This keeps the
single-mesh / single-skeleton `Asset` model intact and reuses meshing + skinning +
collider verbatim.

### 4.2 Shared-code changes (both guarded byte-identical by the determinism baseline)

1. **Promote monster internals to `pub(crate)`** so `generators::boss` can reuse
   them without copy-paste:
   - `rig.rs`: `MonsterRig`, `RigBuilder` (+ its `joint`/`cone`/`ellip`/`flat`/
     `finish`/`wpos`), `PrimitiveDesc`, `PrimKind`, `PrimTint`, `Gait`, `GaitDesc`,
     `add_joint`, `push_cone`, `push_flat`, `spine_sample`, `compute_bounds`, and
     the knob helpers the boss escalates (`add_horns`/`add_spikes`/`add_plates`/
     `add_eyes`/`add_maw`).
   - `body.rs`: `organic_field`, `build_body`, `fit_collider`, `eval_prim`.
   - `mod.rs`: `skin_body`.
   - `anim.rs`: the clip-building helpers the boss reuses (idle / locomotion /
     death constructors) so `build_boss_clips` can append boss clips.
2. **Decouple `build_body` and `fit_collider` from `&MonsterParams`.** They
   currently read only a few fields. Refactor to take those values explicitly:
   - `build_body(rig, size, detail, seed, emissive, pal) -> Mesh`
   - `fit_collider(rig, size, plan_kind) -> Physics`
   Monster's `generate` passes identical values, so output is **byte-identical**
   (the determinism baseline + `cargo test` guard this). This is the single
   shared change that lets the boss reuse meshing/skinning/collider with no fork.

`monster::generate` stays behaviorally identical; only the internal call sites
change.

### 4.3 New module: `src/generators/boss/`

- `mod.rs` — `pub fn generate(p: &BossParams, pal: &Palette) -> Asset`:
  apply archetype preset → build boss rig → `build_body` → `skin_body` →
  `fit_collider` → `build_boss_clips` → extract weak-point colliders + phase meta
  → assemble `Asset` with the `imaginu_boss` metadata attached.
- `rig.rs` — `build_boss_rig(p) -> BossRig`, dispatching on `archetype` to
  `plan_hydra` / `plan_colossus` / `plan_lich` / `plan_swarm_queen` /
  `plan_dragon_lord`. Each builds a big named-jointed `MonsterRig` via the promoted
  `RigBuilder`, fold-ordered core→limbs→attachments, then applies escalated knobs +
  regalia. `BossRig { rig: MonsterRig, parts: Vec<PartTag>, weak_points:
  Vec<WeakPoint>, phase_geometry: PhasePlan }`.
- `preset.rs` — `apply_archetype_preset(&mut BossParams)`: fills sentinel knobs +
  size/armor/regalia/element defaults per archetype (explicit wins).
- `anim.rs` — `build_boss_clips(rig, p) -> Vec<AnimationClip>`: reuse promoted
  idle/locomotion/death helpers; append `telegraph`, the archetype signature attack
  (`slam` | `breath` | `summon`), `phase_transition`, `enrage`, `stagger`.
- `meta.rs` — weak-point / part collider extraction and the `extras.imaginu_boss`
  writer (see §5). All output is ordered `Vec`s — no `HashMap` iteration — for
  determinism.

### 4.4 Regalia & attachments

Crown / weapon / pauldrons / throne / pedestal / brood sacs / chains reuse
`prop.rs` + `csg.rs` (CSG cutters MUST be closed solids) and the monster knob
system. Attachments are fold-ranked LAST as their own skin families so they never
web.

---

## 5. Phases, clips, and `extras.imaginu_boss`

### 5.1 Phase model

Default asset carries **phase-1 geometry** (armored) + baked `phase_transition`
and `enrage` clips + metadata for both phases. The `phase_transition` clip scales
the plate/armor bones toward zero and ramps the core emissive up (the armor
"sheds", exposing a glowing core — the plates are their own joints, so a clip can
shrink them). An optional `phase:2` param regenerates a single-phase **exposed**
silhouette (plates physically removed, core emissive raised) for games that want a
distinct geometry per phase. `phases` (default 2, clamp 1..=4) controls how many
phase metadata blocks are emitted.

### 5.2 Clip set (escalated over the monster set)

`idle`, locomotion (gait-driven), `telegraph` (readable wind-up), a signature
attack per archetype, `phase_transition`, `enrage`, `stagger` (= the monster
`hurt`), `death`. Signature attacks: hydra → multi-head lunge/breath, colossus →
ground `slam`, lich → `summon`/cast, swarm_queen → `summon` brood, dragon_lord →
`breath`. Telegraph clips + emissive cues are gameplay, not decoration.

### 5.3 `extras.imaginu_boss` metadata (freeform node-0 extras)

`nodes[0].extras` is a plain `serde_json::Map`; the existing `imaginu_physics`
block (single whole-body collider) stays untouched and backward-compatible. We add
a sibling `imaginu_boss` key. Weak-point colliders reference **joints by name** (no
new glTF nodes — the game maps them onto the skeleton), mirroring how the
dungeon/world manifests carry gameplay payloads.

```jsonc
"imaginu_boss": {
  "format": "imaginu-boss/1",
  "archetype": "hydra",
  "element": "infernal",
  "phases": [
    { "id": 1, "name": "armored", "hp_fraction": 1.0,
      "active_weak_points": ["weak_point.core"],
      "abilities": [ { "name":"slam", "telegraph_s":1.2, "active_s":0.4,
                       "recover_s":0.8, "clip":"telegraph" } ] },
    { "id": 2, "name": "exposed", "hp_fraction": 0.5, "enrage": true,
      "active_weak_points": ["weak_point.core","head.1","head.2"],
      "abilities": [ /* ... */ ] }
  ],
  "weak_points": [
    { "name":"weak_point.core", "joint":"core",
      "collider": { "type":"sphere", "radius":0.4 },
      "offset":[0,0,0], "destructible":true, "phase":2 }
  ],
  "parts": [ { "name":"head.1", "joint":"neck1_head", "destructible":true } ],
  "arena": { "recommended_radius":8.0, "spawn_offset":[0,0,0] }
}
```

All arrays are deterministically ordered. `Asset` gains an optional
`boss: Option<BossMeta>` field; `to_glb` serializes it into the node-0 `extras`
map alongside `imaginu_physics`.

---

## 6. Arena integration

### 6.1 Dungeon (inline emit + manifest reference)

- `DungeonParams` gains an optional `boss: Option<BossParams>` field.
- On multi-room `write_dir`: if `boss` is set, generate `boss.glb`, scale it to the
  boss room, place it at the boss `SpawnPoint` (the room farthest from the entrance).
- `SpawnEntry` gains an optional `file: Option<String>` (set to `boss.glb` for the
  boss spawn) — mirroring how `RoomEntry` references per-room GLBs.
- `validate-dungeon` (`dungeon::manifest::validate_dir`) checks: if a spawn has a
  `file`, the file exists and is a valid GLB carrying an `imaginu_boss` block.
- Seam law: placement geometry snaps to the same integer-meter rules the dungeon uses.

### 6.2 World (world-boss POI)

- New `PoiKind::Boss` (snake_case `boss`) with a `radius`/`separation` sized for an
  arena. Solver `place` scores it favoring **open, flat** ground (unlike `Dungeon`,
  which favors steep terrain).
- `build_asset` dispatch builds the boss GLB; `poi_file` → `poi_boss_{i}.glb`.
- World `manifest.json` `Poi { file: Some("poi_boss_i.glb"), spawn_points, ... }` —
  the exact existing "POI references an external GLB with a world transform + spawn
  points" precedent.
- `validate-world` (`world::manifest::validate_dir`) already checks referenced
  files exist; extend to sanity-check the boss's `imaginu_boss` block.
- Seam law: snap to world-coord rules like every other POI (edges f32-exact).

---

## 7. CLI, validation, docs

- `main.rs`: insert a `boss` block into `SCHEMA_HELP` (after the monster block);
  the palette line already lists all nine. A single-GLB boss flows through the
  generic `Generate` → `r.build()` path (no new generate subcommand needed).
- New `validate-boss` subcommand: reads `nodes[0].extras.imaginu_boss`, checks the
  block is well-formed (format tag, phases present and ordered, each weak point
  references an existing joint, ability timings non-negative, arena radius > 0).
  Single-GLB structural checks continue via `validate`.
- `skill/imaginu/SKILL.md`: note the new kind (schema stays the reference).
- `README.md` recipe gallery + `docs/site/` (viewer model, recipe rows, gallery
  grid): add a hero boss. **No em-dashes in docs/site.**
- `CHANGELOG.md`: `Unreleased` → boss features; bump to **v0.3.0**.
- Keep `gallery/`, `docs/`, `skill/`, media OUT of the crate (`Cargo.toml` exclude).

---

## 8. Testing & determinism

- **Determinism baseline**: before any change, capture a hash of every existing
  gallery kind's GLB; after, assert byte-identical (all prior kinds unchanged).
  Keep the `texture.rs` f64 + `black_box` guard; no process/time/address/HashMap
  order in generation (use `BTreeSet` / sorted iteration / `rng(seed)` only).
- **Per archetype**: parse+build; build twice → byte-identical; the
  **stretched-triangle edge-length skinning probe** (posed vs bind) below the web
  threshold across every clip; hostile-input (absurd knob/size/phase values) →
  clamps, no panic, bounded joint count.
- **Preset**: each archetype preset fills the expected sentinels; explicit knobs win.
- **Metadata round-trip**: generate → parse the GLB → `imaginu_boss` block matches
  (weak points reference real joints; phases/abilities/arena present and ordered).
- **Arena**: dungeon-with-inline-boss writes `boss.glb` + a spawn `file`, and
  `validate-dungeon` passes; a world with a boss POI writes `poi_boss_*.glb` +
  manifest entry, and `validate-world` passes.
- **No regressions**: `cargo fmt --check`, `cargo clippy --all-targets -- -D
  warnings`, `cargo test`, `cargo doc --no-deps` all clean; gallery regenerates.

## 9. Render-and-look gate

Every archetype rendered from 4 angles + its full clip set (`render --animation`),
scored against `docs/EVALUATION.md` at ≥4/5 on each axis, with a boss clearing a
higher presence bar than a monster. The two heroes (infernal hydra, necrotic lich)
get showcase MP4s. Delegated code passes tests but can look wrong — LOOK at every
visual change.

---

## 10. Definition of done

`boss` is a first-class kind: documented in `imaginu schema`, covered by tests,
present in the gallery, shown on the site and in the skill. An agent can go from
"make me an infernal hydra boss for my crypt" to a loadable, correctly-collided,
weak-point-tagged asset placed in a dungeon boss room or a world POI, using only
the skill + the binary. v0.3.0 is tagged and green; determinism and all prior
kinds are unregressed.

## 11. Known traps (internalized from prior phases)

- Smooth-min **fold order IS skinning correctness**; classify families by primitive
  SDF; junction-blend smoothstep width == branch-cutoff half-width. Composite bosses
  multiply this risk — fuse each sub-body in a deliberate order, keep parts as their
  own skin families, debug webs with the edge-length probe (measure, never guess).
- CSG cutters must be **closed solids**.
- Determinism heisenbug on macOS ARM — keep the `texture.rs` f64 + `black_box` guard.
- Surface-nets quad winding must match rasterizer culling; flat shading averages
  face colors (compare the pre-flat vertex grid for seam tests).
- Rasterizer near/far auto-fits — don't hard-code a far plane for a huge boss.
- `clap` eats a leading `-` — pass `--flag=value` for negative args.
