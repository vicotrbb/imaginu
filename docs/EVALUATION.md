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
