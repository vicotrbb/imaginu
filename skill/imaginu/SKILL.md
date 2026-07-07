---
name: imaginu
description: Use when you need a 3D asset for a game or Babylon.js scene - a tree, rock, crystal, prop, building, terrain, an animated character, an animated monster, a themed navigable dungeon, a custom rigged object, or a whole streaming world. imaginu compiles a small JSON recipe into a deterministic, game-ready GLB (with PBR vertex colors and physics metadata) and can render a PNG so you can look at the result and iterate.
---

# imaginu - compile JSON recipes into game-ready GLB

imaginu turns a tiny JSON **recipe** into a `.glb` asset for Babylon.js (or any
glTF loader). You describe *what* you want; imaginu produces the mesh, colors,
skeleton, animations, and a collider - deterministically.

**The core discipline: generate → _look at the PNG_ → iterate.** Do not claim an
asset looks good without viewing its render.

## 1. Make sure the binary exists

```sh
imaginu --version
```

If that fails, install it (no Rust required):

```sh
curl -fsSL https://raw.githubusercontent.com/vicotrbb/imaginu/main/install.sh | sh
# or, if the user has cargo-binstall / cargo:
cargo binstall imaginu      #  prebuilt binary
cargo install imaginu       #  build from crates.io
```

## 2. Read the recipe contract - do NOT hardcode fields

The schema is authoritative and evolves. Always start from it rather than
guessing field names:

```sh
imaginu schema
```

It prints every `kind` (`terrain`, `tree`, `rock`, `crystal`, `building`,
`prop`, `character`, `monster`, `dungeon`, `boss`, `custom`, `world`), all
fields (all optional except `kind`), the palettes, animation clips, and the
physics/extras contract. Treat its output as ground truth for this version.

`boss` is a multi-part, multi-phase, weak-point-tagged encounter creature (5
archetypes: hydra/colossus/lich/swarm_queen/dragon_lord) that places into
dungeon boss rooms and world POIs. `imaginu schema` stays the field reference
for its full contract.

## 3. The core loop

Write a recipe, compile it with a preview, then **open and look at the PNG**:

```sh
imaginu generate '{"kind":"tree","style":"oak"}' -o tree.glb --preview
#   writes tree.glb  AND  tree.png  (same stem)
```

Look at `tree.png`. Adjust the recipe (style, size, seed, palette, …) and
regenerate until it looks right. `--preview` re-renders every time, so this is
your fast feedback loop. Recipes can be inline JSON (must start with `{`) or a
file path.

Score what you see against silhouette, color harmony, shading, detail density,
and game-readability. Change one thing at a time; a different `seed` gives a
different variation of the same recipe.

### Characters, animation, and worlds

```sh
# A rigged character with 8 clips (idle/walk/run/attack/sit/wave/death/dance):
imaginu generate '{"kind":"character","class":"warrior","animate":true}' -o hero.glb --preview

# Look at a specific animation pose (4 phases, or one --at time):
imaginu render '{"kind":"character","class":"mage","animate":true}' --animation walk -o frames/
imaginu render '{"kind":"character","class":"mage"}' --expression smile -o frames/

# A rigged, animated monster - 8 body plans (biped_brute/quadruped_beast/
# serpent(wyrm)/arachnid/winged_flyer/ooze(blob)/insectoid/aberration) + a
# `class` preset layer (predator/brute/elemental/undead/aberration/swarm),
# composable knobs (horns/spikes/plates/eyes/maw/wings/tail/emissive/size):
imaginu generate '{"kind":"monster","body":"wyrm","class":"elemental"}' -o wyrm.glb --preview

# A themed, navigable dungeon → directory of per-room GLBs + manifest.json
# (rooms/corridors/doors/spawn_points). Themes: crypt/cavern/sewer/mine/temple/
# fortress. --overview renders a ceiling-less top-down of the interior:
imaginu dungeon '{"kind":"dungeon","type":"crypt","size":"medium"}' -o crypt/ --overview
imaginu validate-dungeon crypt/

# A whole seamless streaming map → a directory of chunk GLBs + manifest.json:
imaginu world '{"kind":"world","name":"everdale","size":2048}' -o everdale/ --map
```

`render` writes turntable PNGs (`<name>_0..3.png`) without keeping a GLB - use
it to inspect angles or animation phases. `showcase` makes a loop-perfect MP4
but **requires `ffmpeg`** (see Gotchas).

## 4. Determinism - generate once, trust it

Generation is a pure function of `(recipe, seed)`: the same recipe always
produces the **byte-identical** GLB. So:

- Set an explicit `"seed"` when you want a specific result to be reproducible.
- You never need to "re-roll and hope" - once a recipe looks right, its output
  is locked. Commit the recipe, not just the GLB.

## 5. Using the output in a Babylon.js project

Load the GLB like any glTF. imaginu writes a collider into the **root node's
extras**:

```js
const { meshes } = await BABYLON.SceneLoader.ImportMeshAsync("", "", "tree.glb", scene);
const physics = meshes[0].metadata?.gltf?.extras?.imaginu_physics;
// → { collider: { type: "capsule"|"box"|"sphere"|"trimesh"|"heightfield", ... },
//     mass, friction, restitution }

// Characters ship animation groups by clip name:
scene.getAnimationGroupByName("walk")?.start(true);
```

For **worlds**, read `manifest.json`: each entry in `chunks[]` has a `file` and
a world-space `position` - load each chunk GLB (chunk-local origin) and place it
at its `position`. `pois[]` list placed structures with their own `file`,
`position`, and `spawn_points`; `roads`/`rivers` are polylines.

## 6. Gotchas worth knowing

- **`ffmpeg` is only needed for video** (`showcase`, world `--flyover`). GLB
  generation and PNG previews are fully standalone - never make video a
  dependency of producing an asset.
- **Worlds emit a directory**, not a single file: `manifest.json` + one GLB per
  chunk (+ per-POI GLBs). Point `-o` at a directory.
- **Validate** structural correctness when in doubt:
  `imaginu validate out.glb` (single/multiple GLBs) and
  `imaginu validate-world mapdir/` (manifest + every chunk).
- **`custom` builds anything**: a node-graph DSL (primitives, CSG, bevel,
  subdivision, bones, animations, baked PBR textures). Reach for it when no
  built-in `kind` fits. Its full field list is in `imaginu schema`.
- Malformed or invalid recipes exit non-zero with a clear `error: …` message -
  read it, fix the JSON, retry.

## Reference vs. workflow

This skill is the **workflow**. The **reference** is always `imaginu schema` -
consult it for exact fields rather than memorizing them here, because the schema
is versioned with the binary.
