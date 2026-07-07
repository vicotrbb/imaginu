# Premium Stylized Humanoids Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade `character` assets to premium stylized quality: proportion canon, sculpted SDF head, 4-finger hands, anatomical landmarks, draping cloth shells.

**Architecture:** All work lives in `src/generators/character.rs` plus a new `src/generators/proportions.rs`. The body stays a single blended SDF meshed by `crate::sdf::mesh_field` (surface nets); the head and hands become their own small SDF fields; garments become offset shells of the body field. All magic `h * 0.0xx` constants route through a `Proportions` struct.

**Tech Stack:** Rust, `glam`, existing modules: `sdf.rs` (smin, sd_sphere, sd_round_cone, sd_ellipsoid, mesh_field), `subdiv.rs`, `noise.rs`, `texture.rs` (bake_face), the built-in software renderer (`imaginu render`).

## Global Constraints

- Determinism: same recipe + seed → byte-identical GLB. All randomness through the seeded `Rand` from `super::rng`.
- No new dependencies; pure Rust.
- Poly budget: ≤ ~3× current character vertex count at `detail: 1.0`; `detail` remains the quality dial.
- No skeleton changes: 17 joints, existing joint indices/names. Animations must keep working untouched.
- Existing recipes must keep compiling (new params optional with defaults `average`/`neutral`).
- Version bump to 0.4.0 happens ONLY in Task 6, not before.
- Every task ends with `cargo test` green, `cargo clippy --all-targets -- -D warnings` clean, and a visual rubric check (see below).

**Visual rubric loop (used in every task):** render the fixed panel and eyeball it against the rubric.

```bash
# panel: 3 classes × 3 builds × seed 7, front+side; run from repo root
cargo build --release
for class in mage warrior villager; do for build in slim average heavy; do
  target/release/imaginu render "{\"kind\":\"character\",\"class\":\"$class\",\"build\":\"$build\",\"seed\":7}" \
    -o /tmp/panel_${class}_${build}
done; done
```

Rubric (all must hold before a task's final commit): proportions read human (~7.2–7.6 heads), silhouette smooth with no primitive seams, deep armpit/groin creases preserved, face readable at game distance, hands read as hands, cloth follows anatomy with a flared hem, nothing intersects or floats.

---

### Task 1: Proportion canon (`Proportions` struct + `build`/`frame` recipe params)

**Files:**
- Create: `src/generators/proportions.rs`
- Modify: `src/generators/mod.rs` (add `pub mod proportions;`)
- Modify: `src/recipe.rs:207-254` (add `build`, `frame` to `CharacterParams`)
- Modify: `src/generators/character.rs` (`build_rig`, `generate`, `organic_body` read from `Proportions`)
- Test: unit tests inside `src/generators/proportions.rs`

**Interfaces:**
- Produces: `pub struct Proportions` with `pub fn derive(height: f32, bulk: f32, build: Build, frame: Frame) -> Proportions`; fields (all `f32` unless noted): `head_h` (height of one head unit), `head_r`, `shoulder_w` (half-width, hips-to-shoulder), `hip_w`, `waist` (0..1 taper factor), `arm_r`, `leg_r`, `arm_len`, `leg_len`, `neck_len`, `hand_r`, `torso_lean`. Also `pub enum Build { Slim, Average, Heavy, Heroic }` and `pub enum Frame { Masculine, Feminine, Neutral }`, both `Deserialize` with `#[serde(rename_all = "lowercase")]` and `Default` = `Average`/`Neutral`.
- Later tasks consume `Proportions` fields instead of `h * 0.0xx` literals.

- [ ] **Step 1: Write failing tests** in `src/generators/proportions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canon_head_count_in_range() {
        for (b, f) in [
            (Build::Slim, Frame::Feminine),
            (Build::Average, Frame::Neutral),
            (Build::Heavy, Frame::Masculine),
            (Build::Heroic, Frame::Masculine),
        ] {
            let p = Proportions::derive(1.7, 1.0, b, f);
            let heads = 1.7 / p.head_h;
            assert!((7.2..=7.6).contains(&heads), "{heads} heads");
        }
    }
    #[test]
    fn frame_drives_shoulder_hip_ratio() {
        let m = Proportions::derive(1.7, 1.0, Build::Average, Frame::Masculine);
        let f = Proportions::derive(1.7, 1.0, Build::Average, Frame::Feminine);
        assert!(m.shoulder_w / m.hip_w > f.shoulder_w / f.hip_w);
    }
    #[test]
    fn build_scales_limb_radius_monotonically() {
        let s = Proportions::derive(1.7, 1.0, Build::Slim, Frame::Neutral);
        let a = Proportions::derive(1.7, 1.0, Build::Average, Frame::Neutral);
        let h = Proportions::derive(1.7, 1.0, Build::Heavy, Frame::Neutral);
        assert!(s.arm_r < a.arm_r && a.arm_r < h.arm_r);
        assert!(s.leg_r < a.leg_r && a.leg_r < h.leg_r);
    }
}
```

- [ ] **Step 2:** `cargo test proportions` → FAIL (module doesn't exist).

- [ ] **Step 3: Implement** `src/generators/proportions.rs`. Sketch (tune constants against the rubric, keep the invariants the tests pin):

```rust
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Build { Slim, #[default] Average, Heavy, Heroic }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frame { Masculine, Feminine, #[default] Neutral }

pub struct Proportions {
    pub head_h: f32, pub head_r: f32,
    pub shoulder_w: f32, pub hip_w: f32, pub waist: f32,
    pub arm_r: f32, pub leg_r: f32,
    pub arm_len: f32, pub leg_len: f32, pub neck_len: f32,
    pub hand_r: f32, pub torso_lean: f32,
}

impl Proportions {
    pub fn derive(height: f32, bulk: f32, build: Build, frame: Frame) -> Self {
        let head_h = height / 7.4;
        let (bw, ww) = match build { // limb-radius / waist multipliers
            Build::Slim => (0.85, 0.86), Build::Average => (1.0, 1.0),
            Build::Heavy => (1.22, 1.22), Build::Heroic => (1.12, 0.94),
        };
        let (sh, hp) = match frame { // shoulder / hip multipliers
            Frame::Masculine => (1.08, 0.94), Frame::Feminine => (0.90, 1.06),
            Frame::Neutral => (1.0, 1.0),
        };
        Self {
            head_h, head_r: head_h * 0.62,
            shoulder_w: height * 0.118 * sh * (0.9 + 0.1 * bulk),
            hip_w: height * 0.095 * hp,
            waist: 0.82 * ww,
            arm_r: height * 0.036 * bulk * bw,
            leg_r: height * 0.052 * bulk * bw,
            arm_len: height * 0.44, leg_len: height * 0.47,
            neck_len: height * 0.045, hand_r: height * 0.040,
            torso_lean: 0.008 * height,
        }
    }
}
```

- [ ] **Step 4:** Add `pub mod proportions;` to `src/generators/mod.rs`; `cargo test proportions` → PASS.

- [ ] **Step 5: Recipe params.** In `CharacterParams` (`src/recipe.rs`), after `bulk`:

```rust
    /// Body build: slim | average | heavy | heroic (default average).
    #[serde(default)]
    pub build: crate::generators::proportions::Build,
    /// Skeletal frame: masculine | feminine | neutral (default neutral).
    #[serde(default)]
    pub frame: crate::generators::proportions::Frame,
```

- [ ] **Step 6: Thread through.** In `character.rs::generate`, compute `let pr = Proportions::derive(p.height, p.bulk, p.build, p.frame);` and pass `&pr` into `build_rig` and `organic_body`; replace the equivalent literals (`shoulder_w`, `arm_r = h * 0.036 * bulk`, `leg_r = h * 0.052 * bulk`, hip ellipsoid x-radius → `pr.hip_w`, chest cone x-scale gets `sh` via `pr.shoulder_w`, belly/waist radii × `pr.waist`). Keep every other constant as-is for now — this task is plumbing plus the build/frame deltas, not a re-sculpt.

- [ ] **Step 7: Determinism test.** In `character.rs` tests:

```rust
#[test]
fn character_deterministic_per_seed() {
    let p: crate::recipe::CharacterParams =
        serde_json::from_str(r#"{"seed":7,"build":"heroic","frame":"masculine"}"#).unwrap();
    let pal = crate::palette::Palette::named("verdant").unwrap();
    let a = crate::gltf::to_glb(&generate(&p, &pal));
    let b = crate::gltf::to_glb(&generate(&p, &pal));
    assert_eq!(a, b);
}
```

(Adjust `Palette::named`/`to_glb` call names to whatever the existing tests in the repo use — copy from an existing determinism test.)

- [ ] **Step 8:** `cargo test && cargo clippy --all-targets -- -D warnings` → green. Render the rubric panel; confirm slim/heavy/heroic visibly differ and average looks no worse than v0.3.0.

- [ ] **Step 9: Commit** — `feat(character): proportion canon + build/frame params`.

---

### Task 2: Body landmarks & silhouette resolution

**Files:**
- Modify: `src/generators/character.rs::organic_body` (lines ~211–617)

**Interfaces:**
- Consumes: `Proportions` from Task 1 (passed as `pr: &Proportions`).
- Produces: same `organic_body` signature otherwise; downstream (garments, hands) unchanged.

- [ ] **Step 1: Raise field resolution.** Change `let cell = (h * 0.011 / det).min(h * 0.015);` to `let cell = (h * 0.0085 / det).min(h * 0.012);`. Build a `detail:1.0` character, print vertex count, confirm ≤ 3× the pre-change count (record both numbers in the commit message).

- [ ] **Step 2: Add landmark SDF parts** inside `organic_body`, following the existing `parts.push` pattern:
  - **Clavicle ridge:** thin `Shape::Cone` per side from sternum notch `Vec3::new(0.0, chest.y + h*0.070, h*0.030)` to shoulder `Vec3::new(±sw*0.85, chest.y + h*0.078, h*0.012)`, radii `h*0.012 → h*0.010`, family `Torso`, color `w.shirt`.
  - **Waist taper:** shrink the belly ellipsoid x/z radii by `pr.waist` and add a `ConeScaled` waist segment between hips and spine with x-scale `pr.waist`.
  - **Calf:** replace the straight shin cone with two cones — calf bulge `Cone(s + (f-s)*0.08, s + (f-s)*0.45, leg_r*0.92, leg_r*0.70)` and taper `Cone(s + (f-s)*0.40, s + (f-s)*0.62, leg_r*0.68, leg_r*0.52)`.
  - **Elbow/knee narrowing:** at each joint, end the upper cone and start the lower cone with radii pinched ~12% (e.g. upper-arm end `arm_r*0.70`, forearm start `arm_r*0.72`) so joints read as joints.
  - **Forearm taper:** forearm cone end radius `arm_r*0.50` (was `0.58`).

- [ ] **Step 3:** `cargo test` → green (determinism test from Task 1 still passes — it only checks self-consistency).

- [ ] **Step 4: Rubric panel.** Render; check silhouette smoothness, visible waist/calf/clavicle, no membranes in armpit/groin (the soft/tight family blending must be untouched). Iterate constants until rubric passes.

- [ ] **Step 5: Commit** — `feat(character): anatomical landmarks + higher body field resolution`.

---

### Task 3: Sculpted head

**Files:**
- Modify: `src/generators/character.rs::head_shape` (~line 705) → replace with `sculpted_head`; update the call site at ~line 1225 and the `face`/`bake_face` integration (~lines 1252–1262).

**Interfaces:**
- Consumes: `Proportions` (`head_r`), `Frame` (jaw/brow deltas).
- Produces: `fn sculpted_head(pr: &Proportions, frame: Frame, skin: Vec3, det: f32) -> Mesh` — mesh centered at origin like `head_shape` was, so the existing translate/skin/texture code keeps working.

- [ ] **Step 1: Write failing test:**

```rust
#[test]
fn sculpted_head_has_nose_and_ears() {
    let pr = Proportions::derive(1.7, 1.0, Build::Average, Frame::Neutral);
    let m = sculpted_head(&pr, Frame::Neutral, glam::Vec3::ONE, 1.0);
    let r = pr.head_r;
    // nose: at least one vertex protrudes forward of the face plane at eye-nose height
    assert!(m.positions.iter().any(|v| v.z > r * 0.98 && v.y.abs() < r * 0.35));
    // ears: vertices wider than the cranium ellipsoid on both sides
    assert!(m.positions.iter().any(|v| v.x > r * 0.90));
    assert!(m.positions.iter().any(|v| v.x < -r * 0.90));
}
```

- [ ] **Step 2:** `cargo test sculpted_head` → FAIL (function not defined).

- [ ] **Step 3: Implement** as a small SDF field meshed with `mesh_field` (same machinery as the body), cell `pr.head_r * 0.055 / det`:

```rust
fn sculpted_head(pr: &Proportions, frame: Frame, skin: Vec3, det: f32) -> Mesh {
    use crate::sdf::{sd_ellipsoid, sd_round_cone, sd_sphere, smin};
    let r = pr.head_r;
    let (jaw_w, brow) = match frame {
        Frame::Masculine => (0.78, 0.10), Frame::Feminine => (0.66, 0.04),
        Frame::Neutral => (0.72, 0.07),
    };
    let field = |p: glam::Vec3| -> f32 {
        // cranium
        let mut d = sd_ellipsoid(p, glam::Vec3::new(0.0, r * 0.12, -r * 0.05),
                                 glam::Vec3::new(r * 0.86, r * 0.92, r * 0.90));
        // jaw wedge → chin
        d = smin(d, sd_round_cone(p,
            glam::Vec3::new(0.0, -r * 0.05, r * 0.15),
            glam::Vec3::new(0.0, -r * 0.62, r * 0.30), r * jaw_w, r * 0.28), r * 0.18);
        // cheeks
        for s in [-1.0f32, 1.0] {
            d = smin(d, sd_sphere(p, glam::Vec3::new(s * r * 0.42, -r * 0.08, r * 0.52), r * 0.24), r * 0.14);
        }
        // brow ridge
        d = smin(d, sd_round_cone(p,
            glam::Vec3::new(-r * 0.42, r * 0.28, r * 0.72),
            glam::Vec3::new(r * 0.42, r * 0.28, r * 0.72), r * brow, r * brow), r * 0.10);
        // nose
        d = smin(d, sd_round_cone(p,
            glam::Vec3::new(0.0, r * 0.18, r * 0.80),
            glam::Vec3::new(0.0, -r * 0.16, r * 1.02), r * 0.10, r * 0.13), r * 0.08);
        // ears
        for s in [-1.0f32, 1.0] {
            d = smin(d, sd_ellipsoid(p, glam::Vec3::new(s * r * 0.92, 0.0, -r * 0.05),
                glam::Vec3::new(r * 0.10, r * 0.26, r * 0.20)), r * 0.06);
        }
        // shallow eye sockets (subtract)
        for s in [-1.0f32, 1.0] {
            let socket = sd_sphere(p, glam::Vec3::new(s * r * 0.34, r * 0.10, r * 0.92), r * 0.17);
            d = -smin(-d, socket, r * 0.05); // smooth subtraction
        }
        d
    };
    let lo = glam::Vec3::splat(-r * 1.3);
    let hi = glam::Vec3::new(r * 1.3, r * 1.35, r * 1.35);
    crate::sdf::mesh_field(lo, hi, r * 0.055 / det, &field, &|_| skin)
}
```

- [ ] **Step 4:** `cargo test sculpted_head` → PASS.

- [ ] **Step 5: Integrate.** At the call site (~line 1225) replace `head_shape(head_r, Vec3::ONE, …)` with `sculpted_head(&pr, p.frame, Vec3::ONE, det)` (color stays white — the baked face texture carries skin, unchanged). In `face()`: keep painted brows/mouth/age layers and morph targets, but replace the painted eye dots with **geometry eyes** — inset white icospheres `icosphere(r*0.14, 2, white)` at the socket centers, iris disc `icosphere(r*0.06, 1, iris)` proud of the eyeball front, and an upper-lid overhang (skin-colored half-shell: icosphere clipped `v.y > 0.0`, scaled `1.06`, same center). If `bake_face` also paints eyes, remove that layer so they aren't doubled.

- [ ] **Step 6:** `cargo test` → green. Rubric panel + closeup renders (`imaginu render '{"kind":"character","seed":7,"detail":2.0}'`); iterate SDF constants until the head reads as a stylized human face from front and side, and hair/beard/hat still sit correctly. Verify masculine vs feminine frame gives visibly different jaws.

- [ ] **Step 7: Commit** — `feat(character): sculpted SDF head with geometry eyes, ears, nose`.

---

### Task 4: Hands

**Files:**
- Modify: `src/generators/character.rs::mitten` (~line 672) → replace with `hand`; call site ~line 1325.

**Interfaces:**
- Consumes: `Proportions::hand_r`.
- Produces: `fn hand(at: Vec3, r: f32, side: f32, skin: Vec3, grip: bool, det: f32) -> Mesh` — same placement contract as `mitten` (translated to `at`, `side` = +1 left / −1 right). `grip: true` when the character holds a staff/weapon accessory.

- [ ] **Step 1: Write failing test:**

```rust
#[test]
fn hand_has_separated_fingers() {
    let m = hand(glam::Vec3::ZERO, 0.05, 1.0, glam::Vec3::ONE, false, 1.0);
    // fingers extend below the palm noticeably further than a mitten blob
    let min_y = m.positions.iter().map(|v| v.y).fold(f32::INFINITY, f32::min);
    assert!(min_y < -0.05 * 1.6, "fingers should extend below palm: {min_y}");
}
```

- [ ] **Step 2:** `cargo test hand_has` → FAIL.

- [ ] **Step 3: Implement** as a tight-`smin` SDF field (cell `r * 0.09 / det`): palm ellipsoid `(r*0.72, r*0.95, r*0.55)`; three fingers as `sd_round_cone` from palm base `(fx*r*0.55, -r*0.75, r*0.10)` down to tips `(fx*r*0.60, -r*1.9*len, r*0.15)` with per-finger length `len ∈ {0.92, 1.0, 0.88}`, radii `r*0.16 → r*0.12`, blended with `smin(k = r*0.05)` so they read as separate fingers with webbing only at the base; thumb `sd_round_cone` angled forward-inward from `(side*r*0.55, -r*0.15, r*0.35)` to `(side*r*0.85, -r*0.75, r*0.75)`, radii `r*0.17 → r*0.12`. `grip: true` curls fingers: rotate finger axes ~70° about x toward the palm (move the tip endpoints, not a post-transform, so the SDF blends stay clean) and pulls the thumb across. Keep the mitten's final relaxed-pose transform (`rot_x(0.16)`, `rot_z(-side*0.10)`) and translate to `at`.

- [ ] **Step 4:** `cargo test` → PASS.

- [ ] **Step 5: Integrate:** at the call site pass `grip = p.accessories.iter().any(|a| a == "staff")` for the staff-side hand (staff is held in one hand — check `accessory()` to see which side and match it). Position the staff shaft through the curled grip.

- [ ] **Step 6:** Rubric panel + walk-clip render (`--animation walk`) — hands must not intersect thighs mid-stride. Iterate.

- [ ] **Step 7: Commit** — `feat(character): four-finger SDF hands with grip pose`.

---

### Task 5: Cloth that drapes (garment offset shells)

**Files:**
- Modify: `src/generators/character.rs::garment` (~line 1689) and `outfit_parts` (~line 1715); body field must be reusable — extract the `field` closure from `organic_body` into `fn body_field(parts: &[(Fam, Vec3, Shape)], …) -> impl Fn(Vec3) -> f32` or simply have `organic_body` also return the part list.

**Interfaces:**
- Consumes: the body SDF field; `noise.rs` (`fbm`/value noise — use whatever `terrain.rs` already calls); `garment_tex`/`hem_band` unchanged.
- Produces: garment meshes with the same `Part` names/materials as today, so `outfit_parts` consumers and skinning (`segs_of`) are unchanged.

- [ ] **Step 1: Refactor** `organic_body` to build its `parts` vec via a new `fn body_parts(rig, pr, w, …) -> Vec<(Fam, Vec3, Shape)>` and expose `fn body_sdf(parts: &…, h: f32) -> impl Fn(Vec3) -> f32 + '_` containing the existing hierarchical-blend logic (move, don't duplicate). `cargo test` → green, panel unchanged (pure refactor). Commit — `refactor(character): extract reusable body SDF field`.

- [ ] **Step 2: Shell garments.** For each cloth garment (robe, coat, tunic — not belts/sashes), replace the lathe/tube construction with an offset-shell field meshed over the garment's vertical span `[y0, y1]`:

```rust
// cloth = body surface inflated by thickness, flared toward the hem,
// with low-frequency vertical fold ripples
let cloth = |p: Vec3| -> f32 {
    let t = ((y1 - p.y) / (y1 - y0)).clamp(0.0, 1.0); // 0 at top, 1 at hem
    let flare = h * 0.010 + h * 0.055 * t * t * flare_amt; // hem swings out
    let ang = p.z.atan2(p.x);
    let folds = h * 0.012 * t * (ang * fold_n as f32).sin(); // fold_n seeded 5..8
    body(p) - (thickness + flare + folds)
};
```

Mesh with `mesh_field` over the garment bounds, then drop triangles whose centroid is outside `[y0, y1]` (open top and hem) and re-index. Skin shell verts with the same family-restricted weight logic as the body (reuse `seg_w` via the extracted helpers) so robes move with stride.

- [ ] **Step 3:** Keep `garment_tex` painting (trim bands land on the hem edge — band test: `t > 0.9`). `fold_n` and `flare_amt` come from the seeded `Rand` in `generate`, threaded into `outfit_parts` via `GarmentCtx`.

- [ ] **Step 4:** `cargo test` → green (determinism holds since folds are seeded).

- [ ] **Step 5:** Rubric panel — robe follows waist/hip anatomy, flares at hem, folds visible, no body poke-through at `detail: 1.0` (increase `thickness` floor if the body clips through). Render `--animation walk`: hem must not tear.

- [ ] **Step 6: Commit** — `feat(character): draping offset-shell garments with hem flare + folds`.

---

### Task 6: Panel, gallery regen, version bump

**Files:**
- Modify: `Cargo.toml` (version → `0.4.0`), `CHANGELOG.md`, `README.md`, `skill/` docs (mention `build`/`frame`), `gallery/` (regenerated renders)

**Interfaces:** none new.

- [ ] **Step 1:** Full rubric pass: render the panel (all 9 combos) at `detail: 1.0` and `2.0`, all animation clips for one character (`idle walk run attack sit wave death dance`). Fix any regressions found; each fix is its own commit.
- [ ] **Step 2:** Vertex-count audit: assert `detail:1.0` character ≤ 3× v0.3.0 count (compare against a `git stash`/main build or the recorded Task 2 numbers). If over, raise `cell` sizes until within budget without failing the rubric.
- [ ] **Step 3:** Regenerate gallery renders for all character recipes (follow the same procedure used in commit `e841c61` — see `gallery/` scripts/recipes). Character entries change; non-character gallery hashes must be **unchanged** — verify with `scripts/determinism_baseline.sh` run before/after on non-character recipes.
- [ ] **Step 4:** `Cargo.toml` → `0.4.0`; CHANGELOG entry summarizing the five features; README + site + skill docs updated with `build`/`frame` params and new hero shots.
- [ ] **Step 5:** `cargo test && cargo clippy --all-targets -- -D warnings` → green. Commit — `feat(character)!: premium stylized humanoids; bump to v0.4.0`.
