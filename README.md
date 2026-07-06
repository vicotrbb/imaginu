<div align="center">

# imaginu

**An AI-drivable procedural 3D asset compiler.**
JSON recipes → deterministic, game-ready **GLB** for [Babylon.js](https://www.babylonjs.com/) — meshes, PBR vertex colors, skeletal animation, and physics metadata. No textures to wrangle, no C dependencies, byte-identical every time.

[![CI](https://github.com/vicotrbb/imaginu/actions/workflows/ci.yml/badge.svg)](https://github.com/vicotrbb/imaginu/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/imaginu.svg)](https://crates.io/crates/imaginu)
[![docs.rs](https://img.shields.io/docsrs/imaginu)](https://docs.rs/imaginu)
[![license](https://img.shields.io/crates/l/imaginu.svg)](LICENSE)

**[🌐 Live site & Babylon viewer](https://vicotrbb.github.io/imaginu/)** · **[📖 Docs](https://docs.rs/imaginu)** · **[🤖 Use it with your AI agent](#-use-it-with-your-ai-agent)**

<table>
  <tr>
    <td><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/char_frost_knight.png" width="240"></td>
    <td><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/terrain_island.png" width="240"></td>
    <td><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/crystal.png" width="240"></td>
  </tr>
  <tr>
    <td><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/char_hedge_mage.png" width="240"></td>
    <td><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/tavern.png" width="240"></td>
    <td><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/tree_oak.png" width="240"></td>
  </tr>
</table>

</div>

## What & why

imaginu compiles a small JSON **recipe** into a `.glb` asset you can drop
straight into a Babylon.js scene. It exists because *describing* an asset is far
cheaper than *modeling* one — especially for an AI agent, which can write a
recipe, look at a render, and iterate, all without a DCC tool.

LLMs are great at structured parameters and terrible at raw vertex data. imaginu
is the bridge: the agent decides *what* ("a windswept autumn oak, seed 42") and
imaginu produces the *how* (a crafted mesh with harmonious colors, correct
normals, a skeleton, walk/idle clips, and a collider).

- **AI-native.** `imaginu schema` prints the full recipe contract as a
  cheat-sheet. An agent reads it, writes JSON, and compiles — the
  [skill](#-use-it-with-your-ai-agent) wires this into Claude Code / Codex.
- **Vertex-color PBR, no texture pipeline.** Color lives in the mesh, so assets
  are small, self-contained, and load anywhere glTF loads. (Baked procedural PBR
  texture sets are available in the `custom` DSL when you want them.)
- **Deterministic.** Same recipe + seed → **byte-identical** GLB, on every OS.
  This is what lets worlds tile seamlessly and makes agent output reproducible.
- **Zero C dependencies.** Pure Rust (`glam`, `serde`, `clap`, `png`, `rand`).
  Static cross-compilation — including fully static musl — is trivial.
- **Batteries included.** A built-in software renderer lets you *look* at your
  output (PNG or MP4 turntable) without a GPU or a running engine.

## Install

**Prebuilt binary (no Rust needed):**

```sh
curl -fsSL https://raw.githubusercontent.com/vicotrbb/imaginu/main/install.sh | sh
```

**With cargo:**

```sh
cargo install imaginu           # build from crates.io
cargo binstall imaginu          # or grab a prebuilt binary via cargo-binstall
```

**From source:**

```sh
git clone https://github.com/vicotrbb/imaginu && cd imaginu
cargo install --path .
```

> `ffmpeg` on your `PATH` is **optional** — needed only for video
> (`imaginu showcase`, world `--flyover`). Everything else is standalone.

## 60-second quickstart

```sh
# Compile a recipe to GLB and render a PNG to look at:
imaginu generate '{"kind":"tree","style":"oak"}' -o tree.glb --preview

# The whole recipe contract, any time:
imaginu schema

# A rigged, animated character:
imaginu generate '{"kind":"character","class":"warrior","animate":true}' -o hero.glb

# A seamless streaming world (manifest.json + one GLB per chunk):
imaginu world '{"kind":"world","name":"everdale","size":2048}' -o everdale/

# Verify structure:
imaginu validate tree.glb
```

Load it in Babylon.js like any glTF — colliders ride along in the extras:

```js
const { meshes } = await BABYLON.SceneLoader.ImportMeshAsync("", "", "tree.glb", scene);
const physics = meshes[0].metadata?.gltf?.extras?.imaginu_physics;
// → { collider: { type: "capsule", radius, height }, mass, friction, restitution }
```

## Recipe gallery

Each of these is one line of JSON. Full field reference: `imaginu schema`.

| Recipe | Result |
| --- | --- |
| `{"kind":"tree","style":"oak"}` | <img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/tree_oak.png" width="180"> |
| `{"kind":"terrain","shape":"canyon"}` | <img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/terrain_canyon.png" width="180"> |
| `{"kind":"terrain","shape":"mesa"}` | <img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/terrain_mesa_strata.png" width="180"> |
| `{"kind":"crystal","palette":"mystic"}` | <img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/crystal.png" width="180"> |
| `{"kind":"character","class":"mage"}` | <img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/char_hedge_mage.png" width="180"> |
| `{"kind":"prop","prop":"barrel"}` | <img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/barrel.png" width="180"> |

Whole worlds, too — Voronoi biome zones, traced rivers, A\*-routed roads, and a
POI solver placing cities/castles/dungeons, all seamless across chunk borders:

<table>
  <tr>
    <td align="center"><b>Ravenspire</b><br><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/ravenspire_map.png" width="360"></td>
    <td align="center"><b>Everdale</b><br><img src="https://raw.githubusercontent.com/vicotrbb/imaginu/main/gallery/everdale_map.png" width="360"></td>
  </tr>
</table>

See the **[live site](https://vicotrbb.github.io/imaginu/)** for these GLBs
running in a real Babylon.js viewer, plus looping video showcases. Recipes for
everything above live in [`gallery/recipes/`](gallery/recipes/) and
[`examples/`](examples/).

## 🤖 Use it with your AI agent

imaginu ships a first-class **agent skill** so Claude Code or Codex can go from
"make me a 3D oak tree" to a loadable GLB on its own — write recipe → generate →
**look at the PNG** → iterate.

```sh
# Claude Code:
cp -r skill/imaginu ~/.claude/skills/imaginu
```

See [`skill/imaginu/SKILL.md`](skill/imaginu/SKILL.md) for the workflow and Codex
install instructions. The skill stays DRY against `imaginu schema` — the schema
command is the reference, the skill is the workflow.

## Determinism & the seam law

Generation is a pure function of `(recipe, seed)`. Two consequences you can rely
on:

1. **Reproducible output** — the same recipe yields a byte-identical GLB
   anywhere. CI enforces this on every push (generate twice, diff bytes, on
   both Linux and macOS to guard a known platform float heisenbug).
2. **The seam law** — terrain heights and colors are pure functions of world
   coordinates, so adjacent world chunks share **bit-identical edges** and tile
   with no cracks. Any chunk builds lazily (`--chunk x,z`) or in parallel with
   identical output. This is what makes streaming worlds possible.

## Architecture

Pure-Rust library (`src/lib.rs`) + CLI (`src/main.rs`), cleanly modular:

| Area | Modules |
| --- | --- |
| Recipes & errors | `recipe`, `error` |
| Generators | `generators/{terrain,tree,rock,crystal,building,prop,character,custom}` |
| Geometry | `mesh`, `sdf`, `csg`, `subdiv`, `uv`, `noise` |
| Rigging & animation | `skinning`, `anim` |
| Materials | `palette`, `texture` |
| Export & checking | `gltf`, `validate`, `render` |
| Worlds | `world/{chunk,zones,erosion,network,poi,manifest,minimap,overview,model}` |

Library entry points: `imaginu::compile(recipe_json)` and
`imaginu::compile_to_glb(recipe_json)`, both returning `Result<_, imaginu::Error>`
— the public boundary returns errors instead of panicking on malformed JSON.

## CLI reference

| Command | What it does |
| --- | --- |
| `generate` | Compile a recipe to a GLB (`--preview` for a PNG, `--lods N` for LODs) |
| `render` | Turntable PNGs, or animation frames with `--animation <clip>` |
| `showcase` | Loop-perfect turntable MP4 (needs `ffmpeg`) |
| `world` | Compile a `world` recipe to a chunk directory + `manifest.json` |
| `schema` | Print the full recipe cheat-sheet (the agent contract) |
| `validate` / `validate-world` | Byte-level structural validation |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The two rules: **determinism is sacred**
and **render and look** before claiming visual quality. `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test` must all be clean.
Changelog: [CHANGELOG.md](CHANGELOG.md).

## License

[MIT](LICENSE) © 2026 Victor Bona.
