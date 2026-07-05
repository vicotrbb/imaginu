# imaginu Phase 3 — Painted Garments & Hero Characters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reach hand-painted-MMO fidelity (reference: layered-robe sage): lofted garment shells with structured UVs + a UV-space procedural painter (hem borders, motifs, fold shading), hair cards, painted faces, accessories — drivable both from one-line high-level params and the low-level DSL.

**Architecture:** New `loft` primitive generates garment shells *with intrinsic structured UVs* (u = around the body, v = hem→collar), so a new `paint` layer system in texture.rs can place borders/motifs/folds by garment position. Character generator gains an outfit grammar built on lofts. Everything stays deterministic and lands in the same GLB pipeline.

**Tech Stack:** Rust, existing modules (mesh/texture/uv/subdiv/skinning). No new deps.

## Global Constraints

- Same as Phase 2: determinism, edition-2024 `gen`, no derived Defaults on non-zero spec structs, no zero-count accessors, `r##""##` for hex, visual rubric ≥4/5 with rendered proof, schema/README per task, commit per task, `cargo test` green.
- Paint system must work on ANY mesh with UVs (loft, lathe, projected) — not characters only.

---

### Task 1: Loft primitive with intrinsic structured UVs

**Files:** `src/mesh.rs` (loft fn), `src/generators/custom.rs` (ShapeSpec::Loft), tests, schema.

**Interfaces:** `mesh::loft(stations: &[LoftStation], segments, arc_deg, arc_offset_deg, color) -> Mesh` where `LoftStation { center: Vec3, rx: f32, rz: f32 }`. UVs: u = angle fraction (0..1 over the arc), v = station fraction (0 = first station). Open arcs (< 360°) make front-open robes. Caps optional. DSL: `{"shape":"loft","path":[[x,y,z]..],"rx":[..],"rz":[..],"arc":360,"arc_offset":0,"segments":24}`.

- [ ] loft() + tests (vertex/uv counts, arc openness, valid mesh)
- [ ] DSL wiring + schema; render flared open robe shell; commit `feat: loft primitive with structured garment UVs`

### Task 2: UV-space paint layers

**Files:** `src/texture.rs` (PaintLayer enum + compositor), `src/generators/custom.rs` (uv "keep" mode), tests.

**Interfaces:** `TextureSpec` gains `base: Option<String>` (solid color base alternative to pattern) and `paint: Vec<PaintLayer>`. Layers (serde tag "op"): `band {v, height, color, motif?, motif_color?, motif_scale?}`, `stripes {axis, count, width, color}`, `gradient {from, to, axis}`, `motif_grid {motif, color, scale, v_min, v_max}`, `folds {strength, count}` (painted vertical cloth folds → albedo shading + normal map), `weathering {strength}`. Motifs: `meander`, `zigzag`, `dots`, `diamonds`, `scroll`, `runes` — deterministic 2D SDF stamps. Bake order: base → layers in sequence; height contributions feed the normal map; per-layer roughness/metallic override optional.

- [ ] Motif SDF functions + band/stripe/gradient/folds compositor + determinism/tile tests
- [ ] Node `uv: "keep"` (use intrinsic UVs, skip projection); textured loft renders: hem band with meander motif on an open robe — LOOK, iterate ≥4/5
- [ ] Schema/README; commit `feat: UV-space paint layers — borders, motifs, fold shading`

### Task 3: Character v3 — garment system

**Files:** `src/generators/character.rs` (+ maybe `src/generators/outfit.rs`), `src/recipe.rs`.

**Interfaces:** CharacterParams gains `outfit: Option<String>` (`robe|coat|tunic|plain` — plain = v2 look) and `ornamentation: f32 (0..1)`, `trim_motif: Option<String>`. Outfits are loft stacks rigged to the skeleton: under-robe (closed loft, hem at ankles), open outer coat (arc ≈ 300°, collar), flared sleeves over arms, sash ribbon + hanging tails, mantle/collar. Garments painted via Task 2 (hem/cuff bands, motif panels, folds, palette-driven colors). Smooth-bound: skirt blends hips↔thighs by height; sleeves to arm segments.

- [ ] Under-robe + skirt binding; render idle/walk phases — no leg poke-through at walk amplitude, iterate
- [ ] Outer coat (open arc) + sleeves + sash + mantle; painted trims; render all 4 classes × robe/coat, iterate ≥4/5
- [ ] Commit `feat: character v3 — lofted, painted garment system`

### Task 4: Hair & beard v3 (ribbon cards)

**Files:** `src/generators/character.rs` (hair module section).

**Interfaces:** hair styles add `long`, `topknot`; ribbon cards = tapered curved strips (loft arc segments) along deterministic guide curves from the scalp, clumped, slight sway-ready (bound to HEAD). Beard param: `beard: Option<String>` (`none|mustache|short|long`) — chin/jaw ribbon cards + mustache; brows stay geometry (morphs). Painted strand gradient via paint stripes.

- [ ] Long hair + beard cards; render elder head closeups; iterate ≥4/5
- [ ] Commit `feat: hair & beard v3 — ribbon card styles`

### Task 5: Painted faces + age

**Files:** `src/generators/character.rs`, `src/texture.rs` (face painter reuses paint infra).

**Interfaces:** head gets intrinsic spherical UVs + a baked face texture: painted lids/lip color/cheek blush/nose shading and `age: f32 (0..1)` wrinkles (forehead lines, crow's feet, nasolabial). Geometry eyes/brows/mouth REMAIN (morphs must keep working); paint adds detail around them.

- [ ] Face texture painter + age param; expression renders still correct; closeup renders ≥4/5
- [ ] Commit `feat: painted faces with age detail`

### Task 6: Accessories, AO bake, hero showcase

**Files:** `src/generators/character.rs`, `src/mesh.rs` (AO), examples, gallery.

**Interfaces:** `accessories: Vec<String>` (`necklace|pendant|belt_knot|staff`): chain = instanced torus links along a curve, pendant gem emissive; AO bake = `mesh::bake_ao(&mut Mesh, samples)` cavity approximation multiplied into vertex colors (crevices read in any light), applied to characters + buildings. Hero piece: `examples/elder_sage.json` (high-level recipe) reproducing the reference vibe: long white hair + beard, layered painted robes, pendant. Gallery + turntable & walk MP4s; EVALUATION.md Phase 3 table.

- [ ] Accessories + AO + tests
- [ ] Elder sage recipe — iterate against the reference until ≥4.3/5
- [ ] Gallery/docs refresh; commit `feat: accessories + AO bake`, `docs: phase 3 evaluation + elder sage showcase`

## Self-Review Notes

- Task order is dependency-driven: paint needs loft UVs; garments need both.
- Morphs vs painted faces conflict resolved: geometry face features stay, paint augments.
- Poke-through risk on skirts is called out with an explicit walk-phase render gate in Task 3.
