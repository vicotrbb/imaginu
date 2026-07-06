# Changelog

All notable changes to **imaginu** are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-06

First public release: a deterministic, AI-drivable procedural 3D asset compiler
that turns JSON recipes into game-ready GLB for Babylon.js. Everything below
landed across the project's five development phases and is available in `0.1.0`.

### Added

- **Recipe → GLB compiler.** Compile a JSON recipe to a `.glb` with PBR vertex
  colors and embedded physics metadata: `imaginu generate <recipe> -o out.glb`.
- **Asset generators.** `terrain` (hills, mountains, island, archipelago,
  canyon, mesa, crater, valley, dunes; erosion, rivers, paths, GPU-instanced
  scatter), `tree` (oak/pine/palm/dead), `rock`, `crystal`, `building`, `prop`
  (barrel/crate/lantern/campfire), and `character`.
- **Characters.** 17-joint skeleton, smooth multi-joint skinning, lofted painted
  outfits with trim/brocade motifs, faces with facial morph targets, and eight
  animation clips (idle, walk, run, attack, sit, wave, death, dance).
- **`custom` DSL.** A freeform node graph - box/sphere/cylinder/cone/lathe/
  prism/curve/tube/loft primitives, CSG (subtract/union/intersect), bevel,
  subdivision, displacement, bones, animations with eased/multi-axis channels,
  and baked procedural PBR textures (baseColor + normal + ORM).
- **Worlds.** `imaginu world <recipe> -o mapdir/` compiles a whole streaming map
  into `manifest.json` + one seamless GLB per chunk, with Voronoi biome zones,
  a POI solver (city/village/castle/watchtower/dungeon), global erosion, traced
  rivers, A\*-routed roads with stone bridges, and optional map render.
- **Determinism.** The same recipe and seed produce byte-identical GLB across
  processes and platforms; adjacent world chunks share bit-identical edges.
- **Built-in software renderer.** `imaginu render` (turntable / animation
  frames) and `imaginu showcase` (loop-perfect turntable MP4, needs `ffmpeg`)
  for visual verification - no GPU or external engine required to look at output.
- **Validation.** `imaginu validate` (byte-level GLB structure) and
  `imaginu validate-world` (manifest + all chunks).
- **Agent contract.** `imaginu schema` prints the authoritative recipe
  cheat-sheet; GLBs embed colliders at `nodes[0].extras.imaginu_physics`.
- **glTF extensions.** `EXT_mesh_gpu_instancing` for scatter, `MSFT_lod` for
  LODs (`--lods N`).
- **Library API.** `imaginu::compile` / `imaginu::compile_to_glb` and a typed
  `imaginu::Error`; the public boundary returns `Result` instead of panicking
  on malformed or hostile recipe JSON.

[Unreleased]: https://github.com/vicotrbb/imaginu/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/vicotrbb/imaginu/releases/tag/v0.1.0
