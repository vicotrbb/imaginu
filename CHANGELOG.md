# Changelog

All notable changes to **imaginu** are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-07-07

A new first-class recipe kind - **boss** - for multi-phase encounter creatures,
plus the dungeon and world plumbing to place them. Determinism holds
byte-for-byte and every prior kind is unchanged.

### Added

- **`boss` recipe.** A multi-part, multi-phase encounter creature. Five
  archetypes: `hydra`, `colossus`, `lich`, `swarm_queen`, `dragon_lord`. Each
  boss bakes two distinct phases and a telegraphed clip set - `telegraph`,
  `signature`, `phase_transition`, `enrage`, `stagger` - on top of the usual
  idle/locomotion/attack/hurt/death clips.
- **`extras.imaginu_boss` metadata.** Weak-point tags and phase/ability
  metadata ride along in the GLB extras, so a game can drive hit-reactions,
  phase-gated mechanics, and telegraph timing without re-deriving them from
  the mesh.
- **`validate-boss` subcommand.** `imaginu validate-boss <glb>` structurally
  round-trips a boss GLB, including its phase and weak-point extras.
- **Dungeon inline boss placement.** Dungeon recipes can place a `boss` inline
  into a boss room, wired through the existing spawn-point system.
- **World-boss POI.** The world POI solver can place a boss encounter as a
  world point of interest alongside cities/villages/castles/dungeons.
- **Five gallery bosses.** `infernal_hydra`, `necrotic_lich`,
  `volcanic_colossus`, `fungal_broodmother`, `frost_dragon_lord` - five
  palettes across the gallery, each a dark body with glowing accents.

## [0.2.0] - 2026-07-06

Two new first-class recipe kinds - **monster** and **dungeon** - plus three new
palettes. Determinism holds byte-for-byte and every prior kind is unchanged.

### Added

- **`monster` recipe.** A rigged, animated, collider-bearing creature built by
  generalizing the character body pipeline (SDF round-cones/ellipsoids fused
  with smooth-min, surface-net meshed, family-restricted skinning). Eight body
  plans: `biped_brute`, `quadruped_beast`, `serpent` (alias `wyrm`), `arachnid`,
  `winged_flyer`, `ooze` (alias `blob`), `insectoid`, `aberration`. Composable
  feature knobs (`horns`, `spikes`, `plates`, `tail`, `wings`, `eyes`, `maw`,
  `menace`, `age`, `emissive`, `size`, `detail`) and a `class` preset layer
  (`predator`, `brute`, `elemental`, `undead`, `aberration`, `swarm`) that also
  picks a themed palette. Procedural clips per plan: `idle`, a gait-appropriate
  locomotion clip (`walk`/`slither`/`fly`/`crawl`/`pulse`), `attack`, `hurt`,
  `death`, and `roar` where the plan has a head.
- **`dungeon` recipe.** A themed, navigable underground layout. Six themes:
  `crypt`, `cavern`, `sewer`, `mine`, `temple`, `fortress` - `cavern` is meshed
  as organic SDF caves, the rest as orthogonal rooms with CSG-carved doorways.
  Seed-pure layout (BSP rooms + minimum-spanning-tree corridors with optional
  loops, integer-meter aligned), dressing props (pillars, emissive torch
  brackets, doors, portcullis, sarcophagi, chests, rubble), and spawn points
  (player/enemy/loot/boss). A one-room dungeon builds a single GLB; multi-room
  writes a directory with per-room GLBs and a `manifest.json`.
- **`dungeon` / `validate-dungeon` subcommands.** `imaginu dungeon <recipe> -o
  dir [--overview]` writes the manifest directory and an optional ceiling-less
  overview render; `imaginu validate-dungeon <dir>` structurally round-trips it.
- **Three palettes.** `necrotic` (undead/crypt), `infernal` (fire/forge),
  `fungal` (cavern/ooze), each with an emissive accent for glow markings.

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

[Unreleased]: https://github.com/vicotrbb/imaginu/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/vicotrbb/imaginu/compare/v0.2.0...v0.3.0
[0.1.0]: https://github.com/vicotrbb/imaginu/releases/tag/v0.1.0
