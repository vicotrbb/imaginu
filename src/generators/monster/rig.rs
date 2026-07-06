//! Monster rig: a data-driven creature skeleton + a fold-order-ranked set of
//! SDF primitives + a gait descriptor. This generalizes the fixed humanoid
//! `character::Rig` to arbitrary body plans. For now the `QuadrupedBeast`
//! plan is fully realized; the other plans fall through to it so the recipe
//! surface always compiles and builds.

use glam::{Quat, Vec3};

use crate::gltf::{Joint, Skeleton};
use crate::recipe::{BodyPlan, MonsterParams};

/// Which SDF primitive a body part is built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimKind {
    RoundCone,
    Ellipsoid,
}

/// One flesh primitive spanning two skeleton joints. `fold_rank` orders the
/// smooth-min compose (0 = core torso, higher = later/outer). `k` is the
/// smooth-min blend radius used when this primitive folds into its rank band.
#[derive(Clone, Copy, Debug)]
pub struct PrimitiveDesc {
    pub kind: PrimKind,
    pub joint_a: usize,
    pub joint_b: usize,
    pub r1: f32,
    pub r2: f32,
    pub fold_rank: u8,
    pub k: f32,
}

/// Locomotion style — drives which procedural clip driver builds the
/// creature's movement clip and how idle behaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Walk,
    Slither,
    Fly,
    Crawl,
    Pulse,
}

/// The moving parts of a body plan, as joint-index chains. `legs` holds one
/// chain per leg (root..foot); `spine` runs hips..head; `tail`/`wings` are
/// optional chains; `head` is the head joint if the plan has a distinct one.
#[derive(Clone, Debug)]
pub struct GaitDesc {
    pub legs: Vec<Vec<usize>>,
    pub spine: Vec<usize>,
    pub wings: Vec<usize>,
    pub tail: Vec<usize>,
    pub head: Option<usize>,
    pub style: Gait,
}

/// A fully-planned monster: bind-pose skeleton, ranked flesh primitives, gait
/// descriptor, and a padded world-space bounding box for meshing.
#[derive(Clone, Debug)]
pub struct MonsterRig {
    pub skeleton: Skeleton,
    pub prims: Vec<PrimitiveDesc>,
    pub gait: GaitDesc,
    pub bounds: (Vec3, Vec3),
}

impl MonsterRig {
    /// World-space bind position of joint `i`.
    pub fn joint_world(&self, i: usize) -> Vec3 {
        self.skeleton.global(i).transform_point3(Vec3::ZERO)
    }

    /// World-space bind positions of every joint.
    pub fn world(&self) -> Vec<Vec3> {
        (0..self.skeleton.joints.len())
            .map(|i| self.joint_world(i))
            .collect()
    }
}

/// Dispatch on the body plan. Only `QuadrupedBeast` is fully modeled for
/// M2-M5; every other plan currently reuses the quadruped template so the
/// pipeline stays green end-to-end.
pub fn build_rig(p: &MonsterParams) -> MonsterRig {
    match p.body {
        BodyPlan::QuadrupedBeast => plan_quadruped_beast(p),
        _ => plan_quadruped_beast(p),
    }
}

// ----- quadruped joint indices -----
const HIPS: usize = 0;
const SPINE1: usize = 1;
const SPINE2: usize = 2;
const NECK: usize = 3;
const HEAD: usize = 4;
const TAIL1: usize = 5;
const TAIL2: usize = 6;
// front-left / front-right / rear-left / rear-right legs, each (upper,lower,foot)
const FL_UP: usize = 7;
const FL_LO: usize = 8;
const FL_FT: usize = 9;
const FR_UP: usize = 10;
const FR_LO: usize = 11;
const FR_FT: usize = 12;
const RL_UP: usize = 13;
const RL_LO: usize = 14;
const RL_FT: usize = 15;
const RR_UP: usize = 16;
const RR_LO: usize = 17;
const RR_FT: usize = 18;

/// Build the quadruped-beast template: a horizontal torso along +Z, a neck +
/// head reaching forward-up, a two-segment tail, and four three-joint legs.
pub fn plan_quadruped_beast(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    // menace fattens/lowers the build a touch (0..1 -> +0..0.35 bulk).
    let menace = p.menace.clamp(0.0, 1.0);
    let bulk = 1.0 + 0.35 * menace;

    let hy = 0.62 * s; // hip/shoulder height
    let leg_seg = 0.21 * s; // per leg-segment drop (upper, lower)
    // The leg *swing* joints sit at the torso underside, NOT at the buried
    // shoulder/hip, so the exposed leg top rides its own pivot and motion
    // grows toward the foot — otherwise the whole visible leg swings a large
    // arc against the static belly and shatters the mesh (stretch probe).
    let fl_drop = -0.24 * s; // spine2 world y (~0.66s) -> ~0.42s
    let rl_drop = -0.20 * s; // hips world y (0.62s) -> ~0.42s

    // (parent, name, local translation). Parents always precede children.
    let joints: Vec<(Option<usize>, &str, Vec3)> = vec![
        (None, "hips", Vec3::new(0.0, hy, -0.35 * s)),
        (Some(HIPS), "spine1", Vec3::new(0.0, 0.02 * s, 0.30 * s)),
        (Some(SPINE1), "spine2", Vec3::new(0.0, 0.02 * s, 0.32 * s)),
        (Some(SPINE2), "neck", Vec3::new(0.0, 0.10 * s, 0.20 * s)),
        (Some(NECK), "head", Vec3::new(0.0, 0.06 * s, 0.16 * s)),
        (Some(HIPS), "tail1", Vec3::new(0.0, 0.05 * s, -0.20 * s)),
        (Some(TAIL1), "tail2", Vec3::new(0.0, 0.01 * s, -0.24 * s)),
        // front-left leg (swing joint at the torso underside)
        (Some(SPINE2), "fl_upper", Vec3::new(0.18 * s, fl_drop, 0.0)),
        (Some(FL_UP), "fl_lower", Vec3::new(0.0, -leg_seg, 0.0)),
        (Some(FL_LO), "fl_foot", Vec3::new(0.0, -leg_seg, 0.04 * s)),
        // front-right leg
        (Some(SPINE2), "fr_upper", Vec3::new(-0.18 * s, fl_drop, 0.0)),
        (Some(FR_UP), "fr_lower", Vec3::new(0.0, -leg_seg, 0.0)),
        (Some(FR_LO), "fr_foot", Vec3::new(0.0, -leg_seg, 0.04 * s)),
        // rear-left leg
        (Some(HIPS), "rl_upper", Vec3::new(0.18 * s, rl_drop, 0.0)),
        (Some(RL_UP), "rl_lower", Vec3::new(0.0, -leg_seg, 0.0)),
        (Some(RL_LO), "rl_foot", Vec3::new(0.0, -leg_seg, -0.02 * s)),
        // rear-right leg
        (Some(HIPS), "rr_upper", Vec3::new(-0.18 * s, rl_drop, 0.0)),
        (Some(RR_UP), "rr_lower", Vec3::new(0.0, -leg_seg, 0.0)),
        (Some(RR_LO), "rr_foot", Vec3::new(0.0, -leg_seg, -0.02 * s)),
    ];

    let skeleton = Skeleton {
        joints: joints
            .iter()
            .map(|(parent, name, t)| Joint {
                name: (*name).into(),
                parent: *parent,
                translation: *t,
                rotation: Quat::IDENTITY,
            })
            .collect(),
    };

    // radii (scaled by size + menace bulk)
    let torso_r = 0.28 * s * bulk;
    let neck_r = 0.15 * s * bulk;
    let head_r = 0.17 * s;
    let leg_up_r = 0.11 * s * bulk;
    let leg_lo_r = 0.075 * s;
    let foot_r = 0.05 * s;

    // fold ranks: 0 = core torso, 1 = neck/head, 2 = legs, 3 = tail. k is
    // large near the core (smooth flesh) and small at tips (crisp junctions).
    let prims = vec![
        // --- rank 0: torso core (ellipsoid barrel + filling tube) ---
        PrimitiveDesc {
            kind: PrimKind::Ellipsoid,
            joint_a: HIPS,
            joint_b: SPINE2,
            r1: torso_r,
            r2: 0.05 * s,
            fold_rank: 0,
            k: 0.11 * s,
        },
        PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: HIPS,
            joint_b: SPINE2,
            r1: torso_r * 0.86,
            r2: torso_r * 0.92,
            fold_rank: 0,
            k: 0.11 * s,
        },
        // --- rank 1: neck + head ---
        PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: SPINE2,
            joint_b: NECK,
            r1: torso_r * 0.66,
            r2: neck_r,
            fold_rank: 1,
            k: 0.06 * s,
        },
        PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: NECK,
            joint_b: HEAD,
            r1: neck_r,
            r2: head_r * 0.9,
            fold_rank: 1,
            k: 0.05 * s,
        },
        PrimitiveDesc {
            kind: PrimKind::Ellipsoid,
            joint_a: HEAD,
            joint_b: HEAD,
            r1: head_r,
            r2: 0.0,
            fold_rank: 1,
            k: 0.05 * s,
        },
        // --- rank 3: tail (declared before legs; rank drives order) ---
        PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: HIPS,
            joint_b: TAIL1,
            r1: torso_r * 0.42,
            r2: 0.08 * s,
            fold_rank: 3,
            k: 0.03 * s,
        },
        PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: TAIL1,
            joint_b: TAIL2,
            r1: 0.08 * s,
            r2: 0.025 * s,
            fold_rank: 3,
            k: 0.025 * s,
        },
    ];
    let mut prims = prims;

    // --- rank 2: four legs (upper + lower round cones each) ---
    let legs_joints = [
        (FL_UP, FL_LO, FL_FT),
        (FR_UP, FR_LO, FR_FT),
        (RL_UP, RL_LO, RL_FT),
        (RR_UP, RR_LO, RR_FT),
    ];
    for (up, lo, ft) in legs_joints {
        prims.push(PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: up,
            joint_b: lo,
            r1: leg_up_r,
            r2: leg_lo_r * 1.05,
            fold_rank: 2,
            k: 0.03 * s,
        });
        prims.push(PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: lo,
            joint_b: ft,
            r1: leg_lo_r,
            r2: foot_r,
            fold_rank: 2,
            k: 0.028 * s,
        });
    }

    let gait = GaitDesc {
        legs: legs_joints
            .iter()
            .map(|(u, l, f)| vec![*u, *l, *f])
            .collect(),
        spine: vec![HIPS, SPINE1, SPINE2, NECK, HEAD],
        wings: Vec::new(),
        tail: vec![TAIL1, TAIL2],
        head: Some(HEAD),
        style: Gait::Walk,
    };

    let mut rig = MonsterRig {
        skeleton,
        prims,
        gait,
        bounds: (Vec3::ZERO, Vec3::ZERO),
    };
    rig.bounds = compute_bounds(&rig);
    rig
}

/// Padded world-space AABB enclosing every primitive.
fn compute_bounds(rig: &MonsterRig) -> (Vec3, Vec3) {
    let world = rig.world();
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for d in &rig.prims {
        let r = d.r1.max(d.r2) + d.k;
        for j in [d.joint_a, d.joint_b] {
            let w = world[j];
            lo = lo.min(w - Vec3::splat(r));
            hi = hi.max(w + Vec3::splat(r));
        }
    }
    // ensure feet reach ground plane and a little air headroom
    lo.y = lo.y.min(0.0);
    let pad = Vec3::splat(0.06 * (hi - lo).length().max(1.0) * 0.1 + 0.05);
    (lo - pad, hi + pad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadruped_rig_is_wellformed() {
        let p = MonsterParams::default(); // body = QuadrupedBeast
        let rig = build_rig(&p);
        assert_eq!(rig.gait.legs.len(), 4, "quadruped has 4 legs");
        assert!(matches!(rig.gait.style, Gait::Walk));
        // fold ranks: at least one core prim (rank 0) exists
        assert!(rig.prims.iter().any(|d| d.fold_rank == 0));
        // every prim references valid joints
        let n = rig.skeleton.joints.len();
        assert!(rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
    }
}
