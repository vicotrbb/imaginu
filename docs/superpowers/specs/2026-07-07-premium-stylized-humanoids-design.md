# Premium Stylized Humanoids — Design

**Date:** 2026-07-07
**Status:** Approved
**Scope:** `src/generators/character.rs` and supporting modules (`sdf.rs`, `subdiv.rs`, `noise.rs`, `recipe.rs`). First phase of a broader asset-quality push; monsters/bosses, terrain, and texturing are separate future specs.

## Goal

Raise humanoid character quality from "chunky primitives" to **premium stylized** (modern-indie / Fortnite-adjacent): correct human proportions, smooth blended anatomy, sculpted faces, real stylized hands, and cloth that drapes. Keep everything that makes imaginu special: seeded determinism, full procedural variety, no baked assets, vertex-color PBR.

**Compatibility decision:** better output *is* the point. Existing character recipes re-render with the new bodies (same JSON → better-looking result). Determinism is preserved (same recipe + seed → byte-identical GLB), but output differs from v0.3.0 → **minor version bump to v0.4.0**, gallery regenerated.

## Non-goals

- Semi-realistic anatomy, muscle definition, or close-up facial topology.
- Finger bones / hand animation changes (hands stay skinned to the existing hand bone).
- Changes to monsters, bosses, terrain, or the texture pipeline beyond what garments need.

## Design

### 1. Proportion canon

New `Proportions` struct computed once per character from `(seed, height, bulk, build, frame)`:

- Head-height unit: characters are ~7.2–7.6 heads tall (stylized-heroic canon).
- Derived measures: shoulder width, hip width, waist, limb lengths/radii, hand/foot size, neck length.
- Replaces the scattered `h * 0.0xx` magic constants in `build_rig` and `organic_body` so all anatomy derives from one coherent system.

New optional recipe params:

- `build`: `slim | average | heavy | heroic` — limb thickness, waist taper, mass distribution.
- `frame`: `masculine | feminine | neutral` — shoulder-to-hip ratio, waist, jaw/brow deltas.

Recipes without these params get `average` / `neutral` — tuned to look *better* than today, not identical.

### 2. Sculpted head

Replace the sphere head (`head_shape`) with a blended SDF skull:

- Cranium ellipsoid + jaw/chin wedge + brow ridge + cheek masses + nose (round cone) + ears (flattened ellipsoids) + shallow eye sockets.
- Eyes: inset white spheres with iris disc and upper-lid overhang — set *in* the face, not dots painted on a ball.
- `frame` drives jaw/brow deltas; existing `expressions`, hair, and beard systems keep working on top.
- Highest-iteration item: driven by the render → rubric loop (section 7).

### 3. Hands

Mittens become stylized 4-finger hands: palm box-ellipsoid + thumb round-cone + three grouped finger cones, blended with tight `smin`. Skinned to the existing hand bone. A grip-pose variant curls the fingers so staffs/weapons read as held rather than floated.

### 4. Body landmarks & silhouette

- Add/tune SDF parts: clavicle ridge, waist taper, glute/calf shaping, elbow/knee narrowing, forearm taper.
- Raise the marching-cubes field resolution for the body, keyed off the existing `det` detail factor, then run the existing subdivision pass for smooth silhouettes.
- Keep the existing hierarchical soft/tight family blending (deep armpit/groin creases, soft flesh fillets within a mass).

### 5. Cloth that drapes — DEFERRED (2026-07-07)

The offset-shell approach (body SDF inflated by cloth thickness + hem flare + fold noise) failed its visual gate in three implementation rounds (full-torso shell, hugging shell, and hybrid bodice+skirt-shell all rendered as sacks/bells with walk-animation tearing). The feature is deferred to a future spec with a different technique (e.g. a parametric skirt cage/lattice mesh). Garments ship with the v0.3.0 lathe construction. The reusable `body_parts`/`body_sdf` extraction from this work is kept.

### 6. Performance, determinism, compatibility

- All randomness flows through the seeded `Rand`; determinism byte-identity is preserved and tested.
- Poly budget: ≤ ~2–3× current vertex count per character; `det` remains the quality dial.
- v0.4.0; regenerate gallery and hero shots; update CHANGELOG, README, site, and skill docs.

### 7. Verification

Each feature lands with:

- **Unit tests:** proportion math invariants; per-seed determinism (byte-identical GLB).
- **Visual rubric loop:** render a fixed panel — seeds × classes (mage, knight, villager) × builds — at fixed camera angles; score against a written rubric (proportions, face readability, hand shape, cloth drape, silhouette smoothness); iterate until the rubric passes.

## Implementation order

1. Proportion canon (foundation everything else reads from)
2. Body landmarks & silhouette resolution
3. Sculpted head
4. Hands
5. Cloth shells
6. Panel render, rubric pass, gallery regen, version bump

Each step is independently renderable and verifiable via the rubric panel.
