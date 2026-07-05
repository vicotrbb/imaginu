# imaginu — AI-drivable 3D asset generator (design)

Date: 2026-07-05

## Problem

An AI-built browser game (Babylon.js + Rust + Postgres) needs beautiful, complex,
performant 3D assets — terrain, maps, characters, props — but LLMs cannot emit
good raw mesh data directly.

## Insight

AI agents are excellent at *structured parameters* and terrible at *vertex soup*.
imaginu is a **recipe compiler**: an agent emits a small declarative JSON recipe
and imaginu deterministically compiles it into a production-quality glTF 2.0 (GLB)
asset, ready to drop into Babylon.js.

## Goals

- Beautiful, stylized-realistic low/mid-poly assets with strong silhouettes and
  harmonious palettes (vertex-color PBR — no texture pipeline needed, tiny files,
  fast to load in the browser).
- Deterministic: same recipe + seed → identical bytes. Cacheable, diffable.
- Fast: pure Rust, no GPU, no network, no external AI 3D services.
- Animated characters: skeleton + skinning + idle/walk clips baked into the GLB.
- Physics-ready: collider shape, mass, friction, restitution in glTF `extras`,
  consumable by Babylon + Havok/Cannon.
- Self-evaluating: built-in headless software renderer produces PNG turntables so
  quality is *visually verifiable* in CI or by an agent.

## Non-goals

- Photorealism, texture baking/UV unwrapping, ML inference inside the tool.
- A general-purpose modeling app. This generates game-ready assets from recipes.

## Architecture (single crate: lib + `imaginu` CLI)

- `mesh` — `MeshBuilder`: positions/normals/vertex colors/indices; transforms,
  merging, flat/smooth shading, primitive lathing/extrusion helpers.
- `gen::terrain` — heightfield: fBm + domain warping + slope/altitude biome
  coloring, water plane, optional scatter (trees/rocks) → a full map chunk.
- `gen::tree`, `gen::rock`, `gen::crystal`, `gen::building`, `gen::prop` — props
  with parameterized style/palette/complexity.
- `gen::character` — parameterized humanoid (proportions, palette, gear),
  skeleton, skin weights, baked idle + walk animation clips.
- `palette` — curated color ramps + HSL harmonization utilities.
- `gltf` — hand-written glTF 2.0 GLB writer: meshes, PBR materials, nodes,
  skins, animations, physics `extras`. Round-trip validated in tests.
- `render` — software rasterizer (perspective camera, z-buffer, Lambert +
  hemisphere ambient + rim), PNG output, turntable/gallery shots.
- `recipe` — serde JSON schema (`kind`, `seed`, `style`, per-kind params).
- CLI — `imaginu generate <recipe.json|inline> -o out.glb`,
  `imaginu render <glb|recipe> -o shot.png`, `imaginu gallery -o dir/`.

## Data flow

recipe JSON → generator → `Mesh` (+ skeleton/anim/physics) → GLB writer → file
→ (optional) software renderer → PNG → quality review.

## Error handling

Recipes validated with descriptive errors (unknown kind, out-of-range params).
Generators clamp rather than panic. GLB writer asserts internal invariants
(index bounds, accessor alignment) in tests.

## Testing

- Unit: mesh invariants (finite values, unit normals, index bounds), GLB header
  and JSON-chunk structural validity, determinism (same seed → same bytes).
- Visual: `imaginu gallery` renders a diverse artifact set; images reviewed
  against the quality rubric below; iterate until all pass.

## Quality rubric (each rendered artifact scored 1–5, target ≥4 on all)

1. Silhouette — distinct, readable at a glance.
2. Color harmony — cohesive palette, controlled contrast, no raw primaries.
3. Shading integrity — correct normals, no faceting artifacts where smooth.
4. Detail density — enough complexity to look crafted, no noise vomit.
5. Game readability — reads correctly at gameplay camera distance.
6. Technical correctness — valid GLB, sane scale/origin, physics extras present.

## Decisions taken (autonomous mode)

- Vertex-color PBR over textures: better perf, tiny GLBs, stylized look fits
  an AI-generated game; textures can be layered later.
- Hand-written GLB writer over `gltf` crate: full control of skins/extras,
  no heavyweight deps.
- Software rasterizer over GPU/headless-browser screenshots: deterministic,
  zero setup, runs anywhere; Babylon rendering will only look better (PBR + IBL).
