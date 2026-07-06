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

## Phase 4 — world-scale maps & terrain fidelity (same rubric)

Target: ONE recipe compiles into a complete streaming map (tens of km²) —
biome zones, cities, castles, villages, dungeon mouths, roads, rivers with
bridges — with per-chunk ground quality worth screenshotting, under the
seam law (every height/color a pure function of world coordinates + seed).

| view | score | notes |
|---|---|---|
| minimap (zones + hillshade + networks + POI markers) | 4.5 | reads like a hand-drawn fantasy map; THE layout debugging tool |
| overview beauty shot (stitched world) | 4.2 | lakes/rivers/roads/settlements all legible at 6 km |
| mountain chunk (erosion + crags + micro-relief + strata + scree) | 4.4 | biggest fidelity jump of the phase — rugged eroded rock, not wax |
| forest/lake chunk | 4.3 | dense instanced forest, crisp shorelines with foam bands |
| plains chunk (adaptive res 64) | 4.0 | dry-grass patches break monotony; cheap where flat |
| river chunk (carve + ribbon crossing borders) | 4.4 | channel + water ribbon continue bit-exactly across chunk seams |
| road descending to a bridge crossing | 4.3 | switchbacked dirt road, gap under the span for the bridge GLB |
| walled city (3 building rings, plaza, gate towers) | 4.2 | reads at both street and map scale; terrain flattened seamlessly |
| castle / watchtower / dungeon barrow | 4.0–4.2 | CSG gate arch, brazier glow, emissive barrow shards |

Bugs found only by rendering and looking:
- Zone Gaussian blending at σ=0.42·cell turned the whole map to mush;
  σ=0.26 keeps borders organic but regions distinct.
- Altitude ramp saturated on tall massifs → entire mountains snow-white.
  Root causes (found by numeric probing after three wrong guesses): ramp
  span too small for stacked zone amplitudes AND slope→rock overlay
  triggering at ordinary mountain gradients. Snow is now an absolute
  elevation band; cliffs need slope > 0.85.
- Naive steepest-descent rivers died in the first noise basin (~200 m).
  Priority-flood depression filling before tracing sends every river to a
  lake, the sea, or off the map.
- CSG cutters built from uncapped lathe tubes (profile not touching the
  axis) silently failed to carve — the dungeon mouth survived two
  "fixes" before the open-solid cause was spotted; the barrow is now
  composed geometry, and the castle gate bore is a capped cylinder.
- Flat-shaded chunks tripled vertices: 652 MB for 9.4 km² (would blow the
  2 GB / 50 km² budget). Smooth indexed meshes with ring-derived normals
  are 6× smaller AND make normals/colors bit-identical at seams.
- The rasterizer's hard-coded 500 m far plane z-fought km-scale maps into
  blue speckle; near/far now fit the scene.

Determinism & seams (enforced by tests): 3×3-world bit-identical edges
(positions AND colors) with zones + pinned lake crossing every seam;
chunk built alone == chunk built in a full run, across processes;
adaptive-resolution neighbors stitch crack-free (coarse vertices bit-equal,
fine midpoints exactly collinear); global erosion deterministic across
rebuilds and processes. Budgets: 6×6 km Everdale = 576 chunks + 23 POIs +
5 bridges in ~3 s wall clock, 791 MB on disk (≈1 GB / 50 km²), largest
chunk 131 k tris (≪ 2 M budget), single lazy chunk ≪ 30 s.

## Body v4 — bodies & feet (same rubric)

User report: bodies read as wooden mannequins — slab feet, boxy hip block,
seamy joints. Root cause found by reading, not looking: `cuboid()` emits
flat-shaded faces with duplicated vertices, so `subdivide(smooth)` treats
each face as an island and the pelvis/feet "rounding" had NEVER worked —
they were always boxes. Fixes: sculpted boots (shared-vertex icosphere:
flattened sole, tapered toe box, ankle cuff swallowing the shin), rounded
shorts-style pelvis with a crotch split hint, single continuous tapered
tubes per limb (thigh→knee→calf→ankle and shoulder→elbow→wrist, color
switching mid-surface — no cap seams or lips), hemmed sleeve edges,
shoulder balls tucked into a widened torso shoulder slope, warrior
pauldrons as flat armor caps + a collar ring instead of a floating slab.

| view | before | after |
|---|---|---|
| villager/rogue standing | 3.0 | 4.3 |
| feet/boots close-up | 2.0 | 4.3 |
| warrior armor | 3.2 | 4.1 |
| walk/run/dance deformation | 3.8 | 4.3 (crotch + shoulders hold) |

Bug found only by byte-comparison: an in-process determinism heisenbug —
the auto-vectorized Sobel normal pass returned *different bytes for
identical inputs* depending on what had run earlier in the process
(macOS ARM float-state sensitivity; two stable outcomes; disappeared under
instrumentation). Fixed with f64 gradients + `std::hint::black_box`
pinning the codegen; CLI output was always deterministic across processes.
