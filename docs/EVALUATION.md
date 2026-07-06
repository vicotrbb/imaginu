# imaginu visual quality evaluation

Rubric (1–5 each, target ≥4): silhouette, color harmony, shading integrity,
detail density, game readability, technical correctness.

Process: generate diverse gallery → render with built-in rasterizer → score →
fix generators → repeat. Three rounds were required.

| asset | round 1 | round 3 | key fixes |
|---|---|---|---|
| terrain (verdant) | 4.0 | 4.5 | diorama skirt, water inset, closer framing |
| terrain (volcanic) | 3.0 | 4.0 | emissive lava, skirt, tuned jaggedness |
| tree: oak | 4.0 | 4.3 | branch-tip anchor bug fixed |
| tree: pine | 4.0 | 4.0 | — |
| tree: palm | 2.5 | 4.5 | full crown, kinked wide fronds, bent ringed trunk |
| tree: dead | 2.0 | 4.0 | tips anchored to tube ends (bug), bleached bark, attached twigs |
| rock | 3.0 | 4.0 | chisel quantization, anisotropic stretch, moss confined to tops |
| crystal | 4.5 | 4.5 | — |
| building | 4.0 | 4.0 | — |
| barrel / crate | 4.0 | 4.0 | — |
| lantern | 4.0 | 4.2 | brighter, larger emissive core |
| campfire | 3.5 | 3.9 | acceptable (stylized flame) |
| character: warrior | 3.0 | 4.2 | stockier rig, wrap cuirass + trim, bracers, bigger feet/head |
| character: mage | 3.0 | 4.3 | robe skirt, hat, connected wrists |
| character: rogue/villager | 3.0 | 4.0 | shared rig fixes, hood |

Technical validation (automated, all 17 gallery GLBs): GLB header/length,
JSON chunk parses, no zero-count accessors, animation input/output sampler
counts match, physics extras present, determinism (same recipe+seed → same bytes).

Bug found by validation: empty foliage part in dead tree produced zero-count
accessors — exporter now skips empty parts.

## Round 4 — generality upgrade (same rubric)

| asset | score | notes |
|---|---|---|
| terrain: island | 4.5 | radial falloff, snow cap, water ring, diorama base |
| terrain: canyon (terraced) | 4.0 | carved channel, strata walls, river |
| terrain: crater | 4.3 | volcanic caldera with lava moat |
| terrain: archipelago | 4.0 | scattered islets |
| custom: windmill | 4.0 | pure-DSL build; animated hub, emissive lamp |

Bug found by visual review: custom nodes without an explicit `transform`
collapsed to a point (derived `Default` zeroed `scale`); fixed with a manual
`Default` and pinned by a regression test. Seamless chunk tiling is verified
numerically (identical edge vertices) in `terrain_tiles_seamlessly`.

## Phase 2 — quality ceiling (same rubric)

Every feature was iterated against rendered output (previews, animation
phase frames via `render --animation`, expression frames via
`render --expression`, contact sheets of the full gallery).

| asset | score | notes |
|---|---|---|
| textured cube: wood/rock/fabric/metal/plaster | 4.0–4.5 | baked baseColor + normal + ORM; three seam bugs found *only* by looking (below) |
| tavern (custom DSL, 4 texture sets) | 4.4 | stone strata base, plaster, timber frame, plank roof, glowing windows |
| archway bridge (CSG) | 4.3 | subtract-carved arches, beveled parapets, curve lamp posts |
| character v2 (villager/warrior/mage/rogue) | 4.2–4.3 | smooth subdivision bodies, faces with eyes/brows/nose/mouth, hair styles |
| morph expressions (smile/blink/angry/surprised) | 4.0 | verified frame-by-frame at full weight |
| clips: walk/run/attack/sit/wave/death/dance | 4.0–4.4 | every clip screenshotted at 4 phases; three skinning bugs found visually (below) |
| terrain: eroded river valley | 4.5 | droplet erosion gullies + twin lakes + dense instanced scatter |
| terrain: mesa with strata texture | 4.5 | box-projected rock strata on cliff walls — biggest single visual upgrade |
| refreshed v1 gallery (terrains/trees/rocks/props) | 4.0–4.5 | regenerated from committed recipes (`gallery/recipes/`) |

Bugs found only by rendering and looking:
- Texture tiling: 4-corner noise blends are value-continuous but leave a
  *derivative* seam plus an amplitude dip mid-tile → glint bands along every
  plank seam. Fixed with true lattice-periodic fBm (`Noise2::fbm_tiled`).
- Height ramps at texture seams render as bright normal-mapped walls facing
  the sun; seams must darken albedo only.
- Smooth skinning: torso vertices near shoulders grabbed arm-bone weights and
  flew off with raised arms (dance); opposing thigh weights tore the crotch
  on wide strides (run). Fixed with per-region binding + rigid pelvis.
- Hydraulic erosion fed back exponentially (deposit walls grow their own
  gradient) → 1e21-unit spikes. Fixed with normalized heights, per-step caps
  and a per-cell displacement budget.

Technical validation now ships as `imaginu validate <glb…>` (chunk layout,
accessor bounds & counts, attribute/morph/skin/sampler consistency, embedded
PNG magic, instancing attributes) and passes on all 27 gallery GLBs.
Determinism (same recipe+seed → identical bytes) remains enforced by tests,
including textures, erosion, rivers and instanced scatter.

## Phase 3 — painted garments & hero characters (same rubric)

Target: hand-painted-MMO fidelity (reference: layered-robe elder sage).
The insight driving the phase: that look is ~70% *placement-aware painted
texture* (hem borders, brocade, fold shading) on modest lofted geometry.

| asset | score | notes |
|---|---|---|
| painted loft (band + motifs + folds demo) | 4.3 | greek-key hem trim reads exactly like the reference genre |
| character v3: robe outfit (mage/warrior) | 4.3 | under-robe + open coat + sleeves + sash + mantle, all painted & skinned |
| character v3: tunic (villager) | 4.1 | knee tunic, hem motif band, belt |
| long hair + beard ribbon cards | 4.2 | white-haired elder heads; face kept clear |
| painted faces + age | 4.2 | forehead lines / crow's feet / nasolabial at age 0.85 |
| **elder sage hero** | **4.4** | necklace + pendant over mantle, belt knot, meander trims, walks with flowing robes |

Bugs found only by rendering: garment radii vs. the elliptical torso
(poke-through), sleeve tops as open tubes then as shoulder "chimneys",
necklace buried under the coat, brocade motifs at wallpaper scale.

Bug found only by byte-comparison: an in-process determinism heisenbug —
the auto-vectorized Sobel normal pass returned *different bytes for
identical inputs* depending on what had run earlier in the process
(macOS ARM float-state sensitivity; two stable outcomes; disappeared under
instrumentation). Fixed with f64 gradients + `std::hint::black_box`
pinning the codegen; CLI output was always deterministic across processes.
