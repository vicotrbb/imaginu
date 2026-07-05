# imaginu Phase 2 — Quality Ceiling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise imaginu's output ceiling from "polished stylized low-poly" to "high-end stylized / semi-realistic" via 6 upgrades: baked procedural textures+UVs, smooth skinning, character v2 (faces/hands/morphs), animation v2 (clip library + animated rendering), geometry v2 (subdiv/CSG/bevel/curves/LODs), terrain v3 (erosion/rivers/roads/instancing).

**Architecture:** Each upgrade extends the existing pipeline (recipe → generators → Mesh/Asset → GLB writer → software rasterizer) without breaking the JSON surface. New modules: `src/uv.rs` (projections+tangents), `src/texture.rs` (deterministic pattern baking), `src/skinning.rs` (smooth weights), `src/anim.rs` (CPU pose evaluation + skinning for the renderer), `src/csg.rs` (BSP boolean ops), `src/subdiv.rs` (Loop subdivision + decimation).

**Tech Stack:** Rust (edition 2024), glam, serde/serde_json, png crate, clap, rand_chacha. No new dependencies unless unavoidable; no external services.

## Global Constraints

- Determinism: same recipe+seed → byte-identical GLB. No wall-clock, no `HashMap` iteration order in output paths (use `Vec`/`BTreeMap` or sorted iteration where order reaches bytes).
- Rust edition 2024: `gen` is a reserved keyword.
- serde: never derive `Default` on spec structs whose defaults aren't zero (implement manually — TransformSpec regression).
- glTF forbids zero-count accessors — skip empty parts/accessors.
- Raw strings containing `#hex` need `r##"..."##`.
- Visual loop: after every generator change render previews, score against docs/EVALUATION.md rubric (6 axes, target ≥4/5), iterate. Screenshot animations at multiple t.
- Agent surface: every capability reachable from small JSON with defaults; update `imaginu schema` + README per task.
- Each upgrade = its own commit(s) with tests passing (`cargo test`).
- Poly budgets: hero asset ≤200k tris, terrain chunk ≤2M.

---

### Task 1: Procedural texture baking + UVs

**Files:**
- Create: `src/uv.rs` — UV projection + tangent generation
- Create: `src/texture.rs` — deterministic pattern baking to PNG (baseColor, normal, ORM)
- Modify: `src/mesh.rs` — add `uvs: Vec<Vec2>`, `tangents: Vec<Vec4>`; update merge/validate/to_flat_shaded/transform
- Modify: `src/gltf.rs` — `Material.texture: Option<BakedTexture>`; write images/samplers/textures/TEXCOORD_0/TANGENT
- Modify: `src/render.rs` — bilinear baseColor sampling × vertex color; tangent-space normal mapping; roughness from ORM
- Modify: `src/generators/custom.rs` — `MaterialSpec.texture: Option<TextureSpec>`, `NodeSpec.uv: Option<String>` (`box|cylinder|planar`, default box)
- Modify: `src/main.rs` — schema help
- Test: inline `#[cfg(test)]` in texture.rs, uv.rs, gltf.rs

**Interfaces:**
- Produces: `TextureSpec { pattern: String, scale: f32(1.0), seed: u64(0), normal_strength: f32(1.0), resolution: u32(1024, clamp 64..4096) }` (serde, manual defaults)
- Produces: `texture::bake(&TextureSpec) -> Result<BakedTexture, String>` where `BakedTexture { base_color: Rgb8Image, normal: Rgb8Image, orm: Rgb8Image, base_roughness: f32 }`; `Rgb8Image { w: u32, h: u32, data: Vec<u8> }` with `to_png_bytes()` and `sample(u, v) -> Vec3` (bilinear, wrap)
- Produces: `uv::box_project(&mut Mesh, scale: f32)`, `uv::cylindrical_project(&mut Mesh, scale: f32)`, `uv::planar_project(&mut Mesh, scale: f32)` — fill `uvs` + `tangents`
- Patterns: `wood`, `rock`, `fabric`, `metal` (painted metal w/ wear), `plaster`, `noise`. All height-field-derived normal maps (Sobel of a deterministic height function), tileable (sample noise on a torus / periodic fBm).

**Steps:**
- [ ] Add `uvs`/`tangents` to Mesh with merge/validate handling + unit test (`merge` pads UVs when mixed)
- [ ] Implement `uv.rs` projections + test: box projection on a cuboid yields UVs in expected ranges; tangents orthogonal to normals
- [ ] Implement pattern height functions + albedo colorizers in `texture.rs`; test: `bake` deterministic (two calls byte-equal), tileable (row 0 ≈ row N-1 continuation)
- [ ] Exporter: images (bufferView PNGs), samplers (REPEAT), textures, material texture refs; test: GLB with texture parses, image chunk magic `\x89PNG`, TEXCOORD_0 accessor count == positions
- [ ] Renderer: sample baseColor+normal+roughness when part has texture; render textured cube per pattern → look at them
- [ ] DSL wiring + schema/README; new example `examples/tavern.json` (textured timber building via custom DSL) → render, score ≥4/5, iterate
- [ ] `cargo test`, commit `feat: procedural texture baking + UV projections`

### Task 2: Smooth skinning

**Files:**
- Create: `src/skinning.rs`
- Modify: `src/generators/character.rs` — smooth-bind body core
- Modify: `src/generators/custom.rs` — `NodeSpec.skin: Option<String>` (`"smooth"`)
- Test: inline tests + character render check

**Interfaces:**
- Produces: `skinning::BoneSeg { joint: u16, a: Vec3, b: Vec3 }`; `skinning::smooth_bind(mesh: &mut Mesh, segs: &[BoneSeg], falloff: f32)` — per vertex: distance to each segment, take 4 nearest, weight `w_i = 1/(d_i + eps)^falloff`, normalize, quantize-stable ordering (sort by joint index on ties).
- Character rig exposes `Rig::bone_segments(&self) -> Vec<BoneSeg>` (joint → child-joint world positions).

**Steps:**
- [ ] Failing test: smooth_bind on a 2-bone tube gives mid vertices split weights that sum to 1
- [ ] Implement; validate weights sum≈1, ≤4 joints
- [ ] Character body core smooth-bound (gear stays rigid); render idle/walk poses via Task 4's evaluator once it exists — for now static render + GLB inspect
- [ ] DSL `skin:"smooth"` binds node to all bone segments; test via totem recipe
- [ ] Commit `feat: smooth multi-joint skinning with distance falloff`

### Task 3: Character v2

**Files:**
- Create: `src/subdiv.rs` — Loop subdivision (`subdivide_smooth(&Mesh, n) -> Mesh`, preserves colors/uvs/weights by midpoint interpolation)
- Modify: `src/generators/character.rs` — bodies via smoothed geometry, mitten hands, face (eyes w/ pupils+whites, brows, nose, mouth), hair styles, morph targets
- Modify: `src/mesh.rs` or gltf.rs — `MorphTarget { name: String, deltas: Vec<Vec3> }`, `Part.morphs: Vec<MorphTarget>`
- Modify: `src/gltf.rs` — primitive `targets` (POSITION deltas, sparse not required), mesh `weights: [0,...]`, `extras.targetNames`
- Modify: `src/recipe.rs` — CharacterParams: `hair: Option<String>` (`short|ponytail|bald|bun`), `skin_tone: Option<u32>`, `face: Option<String>` (`neutral|round|angular`), `expressions: bool (default true)`
- Test: morph export byte-structure test; determinism; visual renders of all 4 classes × hair styles

**Interfaces:**
- Produces: glTF morph targets named `smile`, `blink`, `angry`, `surprised` on the character mesh; Babylon reads via morphTargetManager.

**Steps:**
- [ ] Loop subdiv + test (icosahedron subdivides to valid mesh, vertex count grows, bounds shrink slightly)
- [ ] Morph target export + structural test (targets count, targetNames, accessor counts match POSITION)
- [ ] Rebuild character bodies: smooth lathe/subdiv torso+limbs, mitten hands, feet; render all classes, iterate to ≥4/5
- [ ] Face features + morphs (deltas only on head vertices); render expression previews by applying deltas at bake time in a debug render path; iterate
- [ ] Hair styles; renders; schema/README; gallery `char_*_v2` pieces
- [ ] Commit `feat: character v2 — smoothed bodies, hands, faces, hair, morph expressions`

### Task 4: Animation v2

**Files:**
- Create: `src/anim.rs` — `pose_at(&Skeleton, &AnimationClip, t) -> Vec<Mat4>` (sampled channels, LERP/quat SLERP between keys) and `skin_mesh(&Mesh, &[Mat4], &[Mat4] /*ibms*/) -> Mesh`
- Modify: `src/generators/character.rs` — clips: `run`, `attack`, `sit`, `wave`, `death`, `dance`; additive idle sway baked into each
- Modify: `src/generators/custom.rs` — `ChannelSpec.ease: Option<String>` (`cubic_in|cubic_out|cubic_in_out`), `keys_euler: Vec<[f32;3]>` multi-axis rotation
- Modify: `src/render.rs` / `src/main.rs` — `render --animation <name> --at <t>`; `showcase --animation <name>` (fixed 3/4 camera, clip time drives frames, loops)
- Test: pose_at at t=0 equals bind pose for identity clips; skinned vertex moves when joint rotates; determinism

**Steps:**
- [ ] anim.rs + tests
- [ ] CLI flags; render walk at t=0/0.25/0.5/0.75 → LOOK at frames, fix pops
- [ ] New clips one by one, each screenshotted at ≥4 phases and scored
- [ ] DSL easing + euler keys + tests
- [ ] showcase --animation MP4 for a dancing character; schema/README; commit `feat: animation v2 — clip library, easing, animated rendering`

### Task 5: Geometry v2

**Files:**
- Create: `src/csg.rs` — BSP-based union/subtract/intersect on Mesh (csg.js algorithm: build BSP per mesh, clip, invert; rebuild polygons as triangles)
- Modify: `src/subdiv.rs` — `decimate(&Mesh, ratio) -> Mesh` (edge-collapse by shortest-edge with quadric-ish error, deterministic ordering)
- Modify: `src/generators/custom.rs` — NodeSpec: `subdivide: u32(0)`, `smooth: bool(false)` (Loop), `bevel: f32(0)` (box/prism chamfer), `csg: Vec<CsgSpec { op, node }>`; ShapeSpec: `Curve { points, radius, segments, samples }` (Catmull-Rom tube)
- Modify: `src/main.rs`, `src/gltf.rs` — `generate --lods N`: extra decimated primitives as nodes `<name>_LOD1..` with `MSFT_lod` extension + extras
- Test: CSG cube-minus-cylinder is watertight-ish (validates, tri count > 0, bounds correct); bevel box has > 12 tris; decimate halves tris; LOD GLB structure

**Steps:**
- [ ] Bevel + curve + subdivide DSL knobs (reuse subdiv.rs) + tests + renders
- [ ] csg.rs (largest step; port csg.js structure: Plane/Polygon/BspNode with EPSILON 1e-5) + tests
- [ ] DSL csg wiring; build `examples/archway_bridge.json` (CSG arches) → render, iterate ≥4/5
- [ ] decimate + `--lods`; structural test; commit `feat: geometry v2 — subdiv, CSG, bevel, curves, LODs`

### Task 6: Terrain v3

**Files:**
- Modify: `src/generators/terrain.rs` — hydraulic erosion (droplet sim, seeded ChaCha8, `erosion: f32(0)`), rivers (`rivers: u32(0)` springs traced downhill, carve + water ribbon), `paths: Vec<PathSpec { points: Vec<[f32;2]>, width }>` (flatten + dirt), strata texture on cliffs (Task 1 triplanar rock texture on terrain part), denser scatter with `Part.instances: Option<Vec<(Vec3, Quat, Vec3)>>` exported via `EXT_mesh_gpu_instancing`
- Modify: `src/gltf.rs` — instanced part export (separate node, extension attributes TRANSLATION/ROTATION/SCALE)
- Modify: `src/render.rs` — expand instances before rasterizing
- Test: erosion determinism + seamless-tiling still passes (erosion must sample world-space; if erosion breaks tiling, gate it: `erosion` requires skirt/diorama mode and errors on tiled chunks — document); instancing GLB structure test
- Gallery: eroded river valley + showcase MP4

**Steps:**
- [ ] Erosion + tests + renders (before/after compare) ≥4/5
- [ ] Rivers + paths + renders
- [ ] Instanced scatter (exporter + renderer + test)
- [ ] Strata texture; gallery + MP4; schema/README; commit `feat: terrain v3 — erosion, rivers, paths, instanced scatter`

### Task 7: Refresh gallery + docs

- [ ] Regenerate all gallery assets incl. new pieces (textured tavern, dancing avatar v2, CSG archway bridge, eroded river valley) + showcase MP4s (`--animation dance` for avatar)
- [ ] Byte-level validator run over every gallery GLB (extend existing test to cover images, morph targets, instancing extension)
- [ ] Update docs/EVALUATION.md with Phase 2 scoring table; README feature list; commit `docs: phase 2 evaluation + refreshed gallery`

## Self-Review Notes

- Ordering: subdiv.rs is created in Task 3 (character needs it) and extended in Task 5 — intentional.
- Task 2's animated visual verification is deferred to Task 4's evaluator; static bind-pose renders + GLB inspection cover Task 2's commit gate.
- Erosion vs seamless tiling conflict is resolved explicitly (world-space sampling or documented gating) — decided during Task 6 with the tiling test as the arbiter.
