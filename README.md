# imaginu

**AI-drivable procedural 3D asset compiler, in Rust.** An AI agent writes a tiny
JSON *recipe*; imaginu deterministically compiles it into a beautiful, game-ready
**glTF 2.0 (GLB)** asset — terrain dioramas, trees, rocks, crystals, buildings,
props, and **animated characters** — ready to drop into a Babylon.js game, with
physics metadata included.

LLMs are great at structured parameters and terrible at raw vertex data.
imaginu is the bridge: the agent decides *what* ("a windswept autumn oak,
seed 42"), imaginu produces the *how* (a crafted mesh with harmonious colors,
correct normals, a skeleton, walk/idle clips, and a collider).

## Quick start

```sh
cargo build --release

# one-liner: recipe in, GLB + PNG preview out
target/release/imaginu generate '{"kind":"character","class":"mage","palette":"mystic","seed":15}' \
  -o mage.glb --preview

# 4-angle turntable renders (no GPU needed — built-in software rasterizer)
target/release/imaginu render '{"kind":"terrain","palette":"verdant","seed":7}' -o shots/

# cheat-sheet for agents
target/release/imaginu schema

# loop-perfect turntable video for showcasing (requires ffmpeg on PATH)
target/release/imaginu showcase examples/windmill.json -o windmill.mp4
```

`showcase` renders a full 360° spin with the built-in rasterizer and encodes
an h264 MP4 (`--size`, `--duration`, `--fps`, `--pitch`, `--keep-frames`).
The last frame stops one step short of 360°, so the video loops seamlessly —
ready to post as-is.

## Recipes

All fields except `kind` are optional. Same recipe + seed → byte-identical GLB.

| kind | key params | notes |
|---|---|---|
| `terrain` | `size`, `resolution`, `mountainousness`, `water_level`, `scatter` | diorama slab with biomes, water/lava, scattered vegetation; heightfield collider |
| `tree` | `style`: `oak` `pine` `palm` `dead`, `height` | capsule collider |
| `rock` | `size`, `jaggedness` | boulder + satellite stones, moss on top |
| `crystal` | `size`, `count` | emissive faceted shards on a rock base |
| `building` | `width`, `floors` (1–3) | timber-framed cottage, box collider |
| `prop` | `prop`: `barrel` `crate` `lantern` `campfire`, `size` | lantern/campfire glow (emissive) |
| `character` | `class`: `villager` `warrior` `mage` `rogue`, `height`, `bulk`, `animate` | 17-joint skeleton, skinned, `idle` + `walk` clips |
| `custom` | see below | **build anything**: declarative geometry DSL |

Palettes: `verdant`, `autumn`, `arctic`, `volcanic`, `desert`, `mystic`.

### Terrains: any size, any shape, seamless worlds

`terrain` supports `shape` masks — `hills`, `mountains`, `island`,
`archipelago`, `canyon`, `mesa`, `crater`, `valley`, `dunes` — plus `terrace`
(stepped strata), sizes up to 4096 units / 1024×1024 resolution, and
**seamless tiling**: set `skirt: false` and give each chunk its world
`offset_x`/`offset_z`; noise is sampled in world space, so adjacent chunks
share bit-identical edge heights (covered by a unit test). Your world can be
as big as you want, one GLB chunk at a time.

### `custom`: build anything

A declarative scene DSL for arbitrary objects — primitives (`box`, `sphere`,
`lathe`, `cylinder`, `cone`, `tube`, `prism`), per-node `transform`
(translate/rotate/scale), noise `displace`, vertical color gradients, radial
and linear `repeat` arrays, `flat`/smooth shading, arbitrary **bones** with
rigid binding, arbitrary keyframe **animations** (rotation about any axis,
translation paths), emissive materials, and any physics collider (`auto` fits
a box to the result). See [examples/windmill.json](examples/windmill.json) —
a windmill with spinning blades and a glowing lamp — and run
`imaginu schema` for the full cheat-sheet.

## Babylon.js integration

```ts
const res = await BABYLON.SceneLoader.ImportMeshAsync("", "/assets/", "mage.glb", scene);

// physics metadata written by imaginu at the root node's extras
const phys = res.meshes[1].metadata?.gltf?.extras?.imaginu_physics;
if (phys) {
  const shape = phys.collider.type === "box"
    ? new BABYLON.PhysicsShapeBox(/* … phys.collider.halfExtents … */)
    : /* sphere | capsule | trimesh | heightfield */;
  new BABYLON.PhysicsBody(res.meshes[1], BABYLON.PhysicsMotionType.STATIC, false, scene);
}

// characters ship with clips named "idle" and "walk"
scene.getAnimationGroupByName("walk")?.start(true);
```

Assets use **vertex-color PBR** (no textures): tiny files, zero texture requests,
and a cohesive stylized look across every asset the AI generates.

## Architecture

```
recipe JSON ──▶ generators (terrain/tree/rock/crystal/building/prop/character)
                   │  deterministic ChaCha8 RNG + hand-rolled gradient noise
                   ▼
                Mesh (+ Skeleton + AnimationClips + Physics)
                   ▼
                glTF 2.0 GLB writer (skins, animations, extras)   ──▶ .glb
                   ▼
                software rasterizer (z-buffer, Lambert+hemi+rim, 2x SSAA) ──▶ .png
```

- `src/mesh.rs` — mesh builder: lathe, tube, icosphere, cuboid, flat-shading, merging
- `src/noise.rs` — seeded Perlin/fBm/ridged/domain-warp (platform-independent)
- `src/generators/*` — one module per asset family
- `src/gltf.rs` — hand-written GLB exporter
- `src/render.rs` — headless renderer for visual verification (no GPU)
- `src/recipe.rs` — the JSON schema agents write

## Quality process

Every generator was iterated against rendered output using a 6-point rubric
(silhouette, color harmony, shading integrity, detail density, game readability,
technical correctness) until all assets scored ≥4/5 — see
[docs/EVALUATION.md](docs/EVALUATION.md) and the [gallery](gallery/) PNGs.
Structural validity (GLB header, accessor counts, animation sampler pairing,
determinism) is enforced by `cargo test` plus a byte-level validator.

## License

MIT
