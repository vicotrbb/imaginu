//! Shared organic body pass for monsters: compose the rig's ranked SDF
//! primitives into one smoothly-blended field, mesh it with surface nets,
//! color it from the palette, and fit a collider. This is the generalization
//! of `character::organic_body` past the humanoid — the fold ORDER (ascending
//! `fold_rank`, tight across bands away from the core) is what keeps limbs
//! from webbing to the trunk when they later swing.

use glam::Vec3;

use crate::gltf::{Collider, Physics};
use crate::mesh::Mesh;
use crate::palette::Palette;
use crate::recipe::MonsterParams;
use crate::sdf::{mesh_field, sd_ellipsoid, sd_round_cone, smin};

use super::rig::{MonsterRig, PrimKind, PrimitiveDesc};

/// Evaluate one primitive at world point `p` given the joint world positions.
pub(super) fn eval_prim(d: &PrimitiveDesc, world: &[Vec3], p: Vec3) -> f32 {
    let a = world[d.joint_a];
    let b = world[d.joint_b];
    match d.kind {
        PrimKind::RoundCone => sd_round_cone(p, a, b, d.r1, d.r2),
        PrimKind::Ellipsoid => {
            // axis-aligned ellipsoid centered between the joints. Explicit
            // `radii` give a flat sheet (thin on one axis); otherwise derive
            // radii from the joint span (sphere / joint-elongated ellipsoid).
            let c = (a + b) * 0.5;
            let r = match d.radii {
                Some(rv) => rv,
                None => Vec3::splat(d.r1) + (b - a).abs() * 0.5 + Vec3::splat(d.r2),
            };
            sd_ellipsoid(p, c, r)
        }
    }
}

/// Compose the rig's primitives into a single continuous field. Primitives
/// are folded in ascending `fold_rank`: everything in a rank band is joined
/// with each primitive's own `k` (soft flesh), then bands are merged into the
/// core with a *tight* cross-band blend so limbs meet the trunk at a crisp
/// crease instead of a stretchy membrane.
pub fn organic_field(rig: &MonsterRig) -> impl Fn(Vec3) -> f32 {
    let world = rig.world();
    let prims = rig.prims.clone();
    let max_rank = prims.iter().map(|d| d.fold_rank).max().unwrap_or(0);
    // cross-band blend radius per band: the softest (smallest) k in the band,
    // scaled down so junctions stay crisp. Band 0 (core) has no cross merge.
    let cross_k: Vec<f32> = (0..=max_rank as usize)
        .map(|r| {
            if r == 0 {
                return 0.0;
            }
            let min_k = prims
                .iter()
                .filter(|d| d.fold_rank as usize == r)
                .map(|d| d.k)
                .fold(f32::INFINITY, f32::min);
            if min_k.is_finite() { min_k * 0.55 } else { 0.0 }
        })
        .collect();
    move |p: Vec3| -> f32 {
        let mut band = vec![f32::INFINITY; max_rank as usize + 1];
        for d in &prims {
            let e = eval_prim(d, &world, p);
            let r = d.fold_rank as usize;
            band[r] = smin(band[r], e, d.k);
        }
        let mut acc = band[0];
        for r in 1..=max_rank as usize {
            if band[r].is_finite() {
                acc = smin(acc, band[r], cross_k[r]);
            }
        }
        acc
    }
}

/// Nearest-primitive family classifier used for coloring: which body region
/// (0 = torso, 1 = head/neck, 2 = leg, 3 = tail) owns point `p`.
fn region_of(rig: &MonsterRig, world: &[Vec3], p: Vec3) -> u8 {
    let mut best = (f32::INFINITY, 0u8);
    for d in &rig.prims {
        let dist = eval_prim(d, world, p);
        if dist < best.0 {
            best = (dist, d.fold_rank);
        }
    }
    best.1
}

/// Mesh the body from the composed field, colored by region from the palette.
/// `emissive > 0` tints a deterministic fraction of vertices with the accent.
pub fn build_body(rig: &MonsterRig, p: &MonsterParams, pal: &Palette) -> Mesh {
    let world = rig.world();
    let field = organic_field(rig);
    let (lo, hi) = rig.bounds;
    let s = p.size.clamp(0.2, 4.0);
    let detail = p.detail.clamp(0.5, 2.0);
    // cell scaled by detail; clamp so huge/tiny creatures stay affordable.
    let cell = (0.028 * s / detail).clamp(0.012, 0.12);

    // ONE cohesive hide colour: a mid palette tone for the back/flanks, a
    // darker shade of the SAME hue for the belly and the extremities (legs,
    // feet) so the creature reads as one animal, not two materials.
    let base = pal.terrain[2];
    let dark = base * 0.6; // belly + limbs, same hue
    let head_c = base * 0.88; // subtle definition, same hue
    let accent = pal.accent;
    let mid_y = (lo.y + hi.y) * 0.5;
    let seed = p.seed;
    // clamp already yields 0.0 for the negative sentinel/class-default
    let emissive_frac = p.emissive.clamp(0.0, 1.0);

    // deterministic 0..1 hash of a quantized world point (+ seed).
    let hash01 = |q: Vec3| -> f32 {
        let key = ((q.x * 53.0) as i32).wrapping_mul(73856093)
            ^ ((q.y * 53.0) as i32).wrapping_mul(19349663)
            ^ ((q.z * 53.0) as i32).wrapping_mul(83492791)
            ^ (seed as i32).wrapping_mul(2654435761u32 as i32);
        (key.rem_euclid(1000) as f32) / 1000.0
    };

    let color = |q: Vec3| -> Vec3 {
        let c = match region_of(rig, &world, q) {
            // torso: darker belly underside, mid back
            0 => {
                if q.y < mid_y {
                    dark
                } else {
                    base
                }
            }
            1 => head_c, // head/neck/muzzle
            2 => dark,   // legs + feet (same hue, darker)
            _ => base,   // tail reads like the body (no accent leak)
        };
        // subtle seeded value jitter for an organic, non-plastic hide
        let c = crate::palette::vary(c, 0.07, hash01(q));
        if emissive_frac > 0.0 && hash01(q + Vec3::splat(11.0)) < emissive_frac {
            return accent;
        }
        c
    };

    mesh_field(lo, hi, cell, &field, &color)
}

/// Fit a physics collider around the rig, shaped per body plan. Mass scales
/// with the cube of size.
pub fn fit_collider(rig: &MonsterRig, p: &MonsterParams) -> Physics {
    use crate::recipe::BodyPlan;
    let s = p.size.clamp(0.2, 4.0);
    let (lo, hi) = rig.bounds;
    let ext = hi - lo;
    let half = ext * 0.5;
    let collider = match p.body {
        // sprawling / spindly plans: use the render mesh as a static collider
        BodyPlan::Arachnid | BodyPlan::Aberration => Collider::TriMesh,
        // low squat blob / bug: an axis-aligned box hugs the silhouette
        BodyPlan::Ooze | BodyPlan::Insectoid => Collider::Box {
            half_extents: half.max(Vec3::splat(0.05)),
        },
        // everything with a clear long axis: a capsule along the body
        _ => {
            let radius = (ext.y.min(ext.x) * 0.42).max(0.05);
            let height = ext.z.max(ext.y).max(radius * 2.0);
            Collider::Capsule { radius, height }
        }
    };
    Physics {
        collider,
        mass: 55.0 * s * s * s,
        friction: 0.6,
        restitution: 0.15,
    }
}

#[cfg(test)]
mod tests {
    use super::super::rig::build_rig;
    use super::*;

    #[test]
    fn quadruped_body_meshes_and_is_watertight_ish() {
        let p = MonsterParams::default();
        let pal = crate::palette::by_name("verdant");
        let rig = build_rig(&p);
        let mesh = build_body(&rig, &p, &pal);
        assert!(mesh.positions.len() > 500, "non-trivial mesh");
        assert!(mesh.indices.len().is_multiple_of(3), "triangulated");
        // bounds sanity: mesh sits within padded rig bounds
        let (lo, hi) = rig.bounds;
        for v in &mesh.positions {
            assert!(v.x >= lo.x - 0.5 && v.x <= hi.x + 0.5);
        }
    }
}
