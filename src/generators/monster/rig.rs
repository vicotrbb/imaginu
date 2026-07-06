//! Monster rig: a data-driven creature skeleton + a fold-order-ranked set of
//! SDF primitives + a gait descriptor. This generalizes the fixed humanoid
//! `character::Rig` to arbitrary body plans. All eight `BodyPlan`s are modeled
//! (`plan_*`); each returns a [`MonsterRig`] fed to the same shared
//! body/skin/anim pipeline.

use glam::{Quat, Vec3};

use crate::gltf::{Joint, Skeleton};
use crate::recipe::{BodyPlan, MonsterParams};

use crate::generators::{range, rng};

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
    /// Explicit per-axis half-radii for an anisotropic ellipsoid (centered at
    /// the midpoint of the two joints). `None` = derive the radius from the
    /// joint span (spheres / joint-elongated ellipsoids / round cones). Used
    /// to build genuinely FLAT sheets (thin on one axis) like flyer wings.
    pub radii: Option<Vec3>,
}

/// Locomotion style — drives which procedural clip driver builds the
/// creature's movement clip and how idle behaves. Only `Walk` is emitted for
/// the M2-M5 quadruped scope; the other styles are consumed by the clip driver
/// and wired up as later body plans (serpent/flyer/ooze) land.
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

/// Dispatch on the body plan. Every plan builds a fold-ranked [`MonsterRig`]
/// fed to the same shared body/skin/anim pipeline.
pub fn build_rig(p: &MonsterParams) -> MonsterRig {
    match p.body {
        BodyPlan::QuadrupedBeast => plan_quadruped_beast(p),
        BodyPlan::Ooze => plan_ooze(p),
        BodyPlan::Serpent => plan_serpent(p),
        BodyPlan::BipedBrute => plan_biped_brute(p),
        BodyPlan::WingedFlyer => plan_winged_flyer(p),
        BodyPlan::Arachnid => plan_arachnid(p),
        BodyPlan::Insectoid => plan_insectoid(p),
        BodyPlan::Aberration => plan_aberration(p),
    }
}

/// Incremental rig builder: joints are added by WORLD position (converted to
/// parent-relative local translations, valid because all bind rotations are
/// identity) and primitives reference joint indices. Keeps the data-driven
/// plans terse and readable.
struct RigBuilder {
    joints: Vec<(Option<usize>, String, Vec3)>,
    world: Vec<Vec3>,
    prims: Vec<PrimitiveDesc>,
}

impl RigBuilder {
    fn new() -> Self {
        Self {
            joints: Vec::new(),
            world: Vec::new(),
            prims: Vec::new(),
        }
    }

    /// Add a joint at `world_pos`; returns its index.
    fn joint(&mut self, parent: Option<usize>, name: &str, world_pos: Vec3) -> usize {
        let local = match parent {
            Some(p) => world_pos - self.world[p],
            None => world_pos,
        };
        let i = self.joints.len();
        self.joints.push((parent, name.into(), local));
        self.world.push(world_pos);
        i
    }

    /// World position of an already-added joint.
    fn wpos(&self, i: usize) -> Vec3 {
        self.world[i]
    }

    fn cone(&mut self, a: usize, b: usize, r1: f32, r2: f32, fold_rank: u8, k: f32) {
        self.prims.push(PrimitiveDesc {
            kind: PrimKind::RoundCone,
            joint_a: a,
            joint_b: b,
            r1,
            r2,
            fold_rank,
            k,
            radii: None,
        });
    }

    fn ellip(&mut self, a: usize, b: usize, r1: f32, r2: f32, fold_rank: u8, k: f32) {
        self.prims.push(PrimitiveDesc {
            kind: PrimKind::Ellipsoid,
            joint_a: a,
            joint_b: b,
            r1,
            r2,
            fold_rank,
            k,
            radii: None,
        });
    }

    /// A flat/anisotropic ellipsoid centered between `a` and `b` with explicit
    /// per-axis half-radii (thin on one axis = a sheet, e.g. a wing membrane).
    fn flat(&mut self, a: usize, b: usize, radii: Vec3, fold_rank: u8, k: f32) {
        self.prims.push(PrimitiveDesc {
            kind: PrimKind::Ellipsoid,
            joint_a: a,
            joint_b: b,
            r1: 0.0,
            r2: 0.0,
            fold_rank,
            k,
            radii: Some(radii),
        });
    }

    /// Finalize into a bounds-computed [`MonsterRig`].
    fn finish(self, gait: GaitDesc) -> MonsterRig {
        let skeleton = Skeleton {
            joints: self
                .joints
                .iter()
                .map(|(parent, name, t)| Joint {
                    name: name.clone(),
                    parent: *parent,
                    translation: *t,
                    rotation: Quat::IDENTITY,
                })
                .collect(),
        };
        let mut rig = MonsterRig {
            skeleton,
            prims: self.prims,
            gait,
            bounds: (Vec3::ZERO, Vec3::ZERO),
        };
        rig.bounds = compute_bounds(&rig);
        rig
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
// appended leaf joints (kept at the end so leg indices stay stable)
const SNOUT: usize = 19; // muzzle tip, child of HEAD
const TAIL3: usize = 20; // tail point, child of TAIL2

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
        // neck reaches forward, head drops BELOW the back line (predatory)
        (Some(SPINE2), "neck", Vec3::new(0.0, 0.06 * s, 0.22 * s)),
        (Some(NECK), "head", Vec3::new(0.0, -0.06 * s, 0.20 * s)),
        // tapering tail chain sweeping back + down
        (Some(HIPS), "tail1", Vec3::new(0.0, 0.02 * s, -0.18 * s)),
        (Some(TAIL1), "tail2", Vec3::new(0.0, -0.03 * s, -0.22 * s)),
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
        // leaf joints (muzzle + tail point) drive geometry only
        (Some(HEAD), "snout", Vec3::new(0.0, -0.02 * s, 0.15 * s)),
        (Some(TAIL2), "tail3", Vec3::new(0.0, -0.05 * s, -0.20 * s)),
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
    let neck_r = 0.16 * s * bulk;
    let head_r = 0.16 * s;
    let leg_up_r = 0.14 * s * bulk; // beefy haunch
    let leg_lo_r = 0.08 * s;
    let foot_r = 0.055 * s;

    // fold ranks: 0 = core torso, 1 = neck/head, 2 = legs, 3 = tail. k is
    // large near the core (smooth flesh) and small at tips (crisp junctions).
    let rc = |a: usize, b: usize, r1: f32, r2: f32, fold_rank: u8, k: f32| PrimitiveDesc {
        kind: PrimKind::RoundCone,
        joint_a: a,
        joint_b: b,
        r1,
        r2,
        fold_rank,
        k,
        radii: None,
    };
    let el = |a: usize, b: usize, r1: f32, r2: f32, fold_rank: u8, k: f32| PrimitiveDesc {
        kind: PrimKind::Ellipsoid,
        joint_a: a,
        joint_b: b,
        r1,
        r2,
        fold_rank,
        k,
        radii: None,
    };
    let mut prims = vec![
        // rank 0: torso core (ellipsoid barrel + filling tube)
        el(HIPS, SPINE2, torso_r, 0.05 * s, 0, 0.11 * s),
        rc(HIPS, SPINE2, torso_r * 0.86, torso_r * 0.92, 0, 0.11 * s),
        // rank 1: thick neck bridge + head + elongated muzzle
        rc(SPINE2, NECK, torso_r * 0.6, neck_r * 1.05, 1, 0.06 * s),
        rc(NECK, HEAD, neck_r, head_r * 0.85, 1, 0.05 * s),
        el(HEAD, SNOUT, head_r, 0.015 * s, 1, 0.045 * s),
        // rank 3: 3-segment tapering tail
        rc(HIPS, TAIL1, torso_r * 0.5, 0.12 * s, 3, 0.03 * s),
        rc(TAIL1, TAIL2, 0.12 * s, 0.06 * s, 3, 0.028 * s),
        rc(TAIL2, TAIL3, 0.06 * s, 0.018 * s, 3, 0.022 * s),
    ];

    // rank 2: four legs (upper + lower round cones each). Small upper-leg k ->
    // a tight haunch/shoulder join (a defined crease, not a melted blob).
    let legs_joints = [
        (FL_UP, FL_LO, FL_FT),
        (FR_UP, FR_LO, FR_FT),
        (RL_UP, RL_LO, RL_FT),
        (RR_UP, RR_LO, RR_FT),
    ];
    for (up, lo, ft) in legs_joints {
        prims.push(rc(up, lo, leg_up_r, leg_lo_r * 1.15, 2, 0.018 * s));
        prims.push(rc(lo, ft, leg_lo_r, foot_r, 2, 0.026 * s));
    }

    let gait = GaitDesc {
        legs: legs_joints
            .iter()
            .map(|(u, l, f)| vec![*u, *l, *f])
            .collect(),
        spine: vec![HIPS, SPINE1, SPINE2, NECK, HEAD],
        wings: Vec::new(),
        tail: vec![TAIL1, TAIL2, TAIL3],
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
        let r = match d.radii {
            Some(rv) => rv.max_element(),
            None => d.r1.max(d.r2),
        } + d.k;
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

// ===================== the other 7 body plans =====================
// All reuse the shared body/skin/anim pipeline; each just lays out joints +
// fold-ranked primitives + a gait descriptor. Coordinate convention: Y up,
// +Z forward, ground at y = 0.

use core::f32::consts::{PI, TAU};

/// M6a — Ooze/blob: a low wide gelatinous mound (crossed horizontal capsules +
/// a top bulge + a seeded lump). No limbs; pulses in place.
fn plan_ooze(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let core = r.joint(None, "core", v(0.0, 0.30 * s, 0.0));
    let top = r.joint(Some(core), "top", v(0.0, 0.48 * s, 0.05 * s));
    let left = r.joint(Some(core), "l", v(-0.32 * s, 0.30 * s, 0.0));
    let right = r.joint(Some(core), "r", v(0.32 * s, 0.30 * s, 0.0));
    let front = r.joint(Some(core), "f", v(0.0, 0.30 * s, 0.30 * s));
    let back = r.joint(Some(core), "b", v(0.0, 0.30 * s, -0.30 * s));
    let mut rr = rng(p.seed);
    let lump = r.joint(
        Some(core),
        "lump",
        v(
            range(&mut rr, -0.2, 0.2) * s,
            0.44 * s,
            range(&mut rr, -0.15, 0.15) * s,
        ),
    );
    // crossed horizontal capsules form a wide low blob; bulge + lump add mass
    r.cone(left, right, 0.33 * s, 0.33 * s, 0, 0.14 * s);
    r.cone(front, back, 0.30 * s, 0.30 * s, 0, 0.14 * s);
    r.ellip(top, top, 0.24 * s, 0.0, 0, 0.10 * s);
    r.ellip(lump, lump, 0.16 * s, 0.0, 0, 0.08 * s);
    r.finish(GaitDesc {
        legs: Vec::new(),
        spine: vec![core, top],
        wings: Vec::new(),
        tail: Vec::new(),
        head: None,
        style: Gait::Pulse,
    })
}

/// M6b — Serpent/wyrm (the fire-wyrm hero base): a thick tapered spine (12
/// joints) in a clear horizontal S with the front third REARED up, ending in a
/// broad blunt-snouted head with an underslung jaw.
fn plan_serpent(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let n = 12usize;
    let mut chain = Vec::new();
    let mut prev = None;
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32; // 0 = tail .. 1 = head
        // body runs along +Z; the front third rears up off the ground
        let z = (t - 0.5) * 2.4 * s;
        let x = 0.42 * s * (t * PI * 2.2).sin(); // clear horizontal S
        let rear = (((t - 0.55) / 0.45).clamp(0.0, 1.0)).powi(2); // eased lift
        let y = 0.20 * s + 0.95 * s * rear;
        let idx = r.joint(prev, &format!("spine{i}"), v(x, y, z));
        chain.push(idx);
        prev = Some(idx);
    }
    // thick coil: fat mid-body (~0.28s) tapering to a pointed tail
    let rad = |t: f32| (0.28 * s * (1.0 - (2.0 * t - 1.0).powi(2) * 0.78)).max(0.04 * s);
    for i in 0..n - 1 {
        let ta = i as f32 / (n - 1) as f32;
        let tb = (i + 1) as f32 / (n - 1) as f32;
        r.cone(chain[i], chain[i + 1], rad(ta), rad(tb), 0, 0.05 * s);
    }
    // defined head: a broad blunt ellipsoid (distinctly wider than the neck)
    // pointing forward-down, plus an underslung lower jaw = a maw.
    let head = chain[n - 1];
    let hp = r.wpos(head);
    let snout = r.joint(Some(head), "snout", hp + v(0.0, -0.14 * s, 0.30 * s));
    let jaw = r.joint(Some(head), "jaw", hp + v(0.0, -0.20 * s, 0.20 * s));
    // wide/tall/long head block via explicit radii; centered a bit forward
    r.flat(head, snout, v(0.26 * s, 0.24 * s, 0.34 * s), 0, 0.05 * s);
    r.cone(head, jaw, 0.16 * s, 0.09 * s, 0, 0.04 * s); // lower jaw
    r.finish(GaitDesc {
        legs: Vec::new(),
        spine: chain,
        wings: Vec::new(),
        tail: Vec::new(),
        head: Some(head),
        style: Gait::Slither,
    })
}

/// M6c — Biped brute: hunched humanoid, 2 arms + 2 legs + torso + head. Arms
/// are their own skin families (connectivity); only the legs drive the walk.
fn plan_biped_brute(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    let bulk = 1.0 + 0.4 * p.menace.clamp(0.0, 1.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    // hunched, heavy build in the base bind (menace/size stack on later)
    let hips = r.joint(None, "hips", v(0.0, 0.88 * s, 0.0));
    let spine = r.joint(Some(hips), "spine", v(0.0, 1.02 * s, 0.0));
    let chest = r.joint(Some(spine), "chest", v(0.0, 1.20 * s, -0.05 * s));
    let neck = r.joint(Some(chest), "neck", v(0.0, 1.30 * s, 0.06 * s));
    let head = r.joint(Some(neck), "head", v(0.0, 1.40 * s, 0.12 * s));
    r.ellip(hips, chest, 0.24 * s * bulk, 0.05 * s, 0, 0.10 * s); // thick barrel torso
    r.cone(hips, chest, 0.20 * s * bulk, 0.24 * s * bulk, 0, 0.10 * s);
    r.flat(
        chest,
        chest,
        v(0.36 * s * bulk, 0.22 * s, 0.24 * s),
        0,
        0.09 * s,
    ); // broad shoulders
    r.cone(chest, neck, 0.16 * s * bulk, 0.12 * s, 1, 0.06 * s); // thick neck
    r.cone(neck, head, 0.10 * s, 0.13 * s, 1, 0.05 * s);
    r.ellip(head, head, 0.15 * s, 0.0, 1, 0.05 * s);
    // arms (own families) — heavy limbs off the broad shoulders
    for side in [-1.0f32, 1.0] {
        let sh = r.joint(Some(chest), "upperarm", v(side * 0.34 * s, 1.18 * s, 0.0));
        let el = r.joint(Some(sh), "forearm", v(side * 0.46 * s, 0.90 * s, 0.06 * s));
        let hn = r.joint(Some(el), "hand", v(side * 0.50 * s, 0.64 * s, 0.10 * s));
        r.cone(sh, el, 0.16 * s * bulk, 0.12 * s * bulk, 2, 0.02 * s);
        r.cone(el, hn, 0.12 * s, 0.09 * s, 2, 0.026 * s);
        r.ellip(hn, hn, 0.11 * s, 0.0, 2, 0.02 * s); // big fists
    }
    // legs (thick thighs; swing joints at the torso underside)
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        let th = r.joint(Some(hips), "thigh", v(side * 0.16 * s, 0.70 * s, 0.0));
        let sn = r.joint(Some(th), "shin", v(side * 0.17 * s, 0.38 * s, 0.02 * s));
        let ft = r.joint(Some(sn), "foot", v(side * 0.18 * s, 0.05 * s, 0.14 * s));
        r.cone(th, sn, 0.18 * s * bulk, 0.12 * s, 2, 0.018 * s);
        r.cone(sn, ft, 0.12 * s, 0.07 * s, 2, 0.026 * s);
        legs.push(vec![th, sn, ft]);
    }
    r.finish(GaitDesc {
        legs,
        spine: vec![hips, spine, chest, neck, head],
        wings: Vec::new(),
        tail: Vec::new(),
        head: Some(head),
        style: Gait::Walk,
    })
}

/// M6d — Winged flyer: a slim biped core with two large wings (thin-cone frame
/// + membrane lobe, fold-ranked last) and small tucked legs.
fn plan_winged_flyer(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let hips = r.joint(None, "hips", v(0.0, 0.85 * s, 0.0));
    let chest = r.joint(Some(hips), "chest", v(0.0, 1.02 * s, 0.08 * s));
    let neck = r.joint(Some(chest), "neck", v(0.0, 1.10 * s, 0.20 * s));
    let head = r.joint(Some(neck), "head", v(0.0, 1.12 * s, 0.36 * s));
    let hp = r.wpos(head);
    let snout = r.joint(Some(head), "snout", hp + v(0.0, -0.02 * s, 0.16 * s));
    r.ellip(hips, chest, 0.17 * s, 0.04 * s, 0, 0.09 * s);
    r.cone(hips, chest, 0.15 * s, 0.15 * s, 0, 0.09 * s);
    r.cone(chest, neck, 0.11 * s, 0.09 * s, 1, 0.05 * s);
    r.cone(neck, head, 0.08 * s, 0.09 * s, 1, 0.045 * s);
    r.ellip(head, snout, 0.10 * s, 0.015 * s, 1, 0.04 * s);
    // small tucked legs
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        let th = r.joint(Some(hips), "thigh", v(side * 0.10 * s, 0.72 * s, -0.05 * s));
        let sn = r.joint(Some(th), "shin", v(side * 0.11 * s, 0.50 * s, 0.02 * s));
        let ft = r.joint(Some(sn), "foot", v(side * 0.12 * s, 0.16 * s, 0.14 * s));
        r.cone(th, sn, 0.07 * s, 0.05 * s, 2, 0.018 * s);
        r.cone(sn, ft, 0.05 * s, 0.035 * s, 2, 0.02 * s);
        legs.push(vec![th, sn, ft]);
    }
    // wings: each a large ellipsoid FLATTENED on the vertical axis (a genuine
    // thin sheet — wide span, ~0.045s thin, deep chord), spread wide from the
    // shoulders and swept slightly forward. Fold-ranked last and fused with a
    // LOW k (near hard-union) so smooth-min does NOT inflate them back to lobes.
    let mut wings = Vec::new();
    for side in [-1.0f32, 1.0] {
        let root = r.joint(Some(chest), "wing", v(side * 0.16 * s, 1.06 * s, 0.02 * s));
        // wingtip out to the side + slightly forward (swept leading edge)
        let tip = r.joint(
            Some(root),
            "wingtip",
            v(side * 1.15 * s, 1.14 * s, 0.12 * s),
        );
        // membrane center ~mid-span; radii = (half-span, thin, half-chord)
        r.flat(root, tip, v(0.62 * s, 0.045 * s, 0.42 * s), 4, 0.012 * s);
        // a thin leading-edge spar for a defined bone
        r.cone(root, tip, 0.05 * s, 0.02 * s, 4, 0.02 * s);
        wings.push(root);
    }
    r.finish(GaitDesc {
        legs,
        spine: vec![hips, chest, neck, head],
        wings,
        tail: Vec::new(),
        head: Some(head),
        style: Gait::Fly,
    })
}

/// M6e — Arachnid: two-part body (cephalothorax + abdomen) with 8 radial legs
/// bent up at the knee and planted wide on the ground.
fn plan_arachnid(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let ceph = r.joint(None, "cephalothorax", v(0.0, 0.34 * s, 0.18 * s));
    let abdo = r.joint(Some(ceph), "abdomen", v(0.0, 0.40 * s, -0.35 * s));
    r.ellip(ceph, ceph, 0.20 * s, 0.0, 0, 0.07 * s);
    r.ellip(abdo, abdo, 0.26 * s, 0.0, 0, 0.08 * s);
    r.cone(ceph, abdo, 0.13 * s, 0.15 * s, 0, 0.05 * s);
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        for kk in 0..4 {
            let zc = 0.30 * s - kk as f32 * 0.20 * s;
            let spread = 0.16 * s * (1.5 - kk as f32);
            let root = r.joint(Some(ceph), "coxa", v(side * 0.20 * s, 0.33 * s, zc));
            let knee = r.joint(
                Some(root),
                "knee",
                v(side * 0.52 * s, 0.58 * s, zc + spread * 0.4),
            );
            let foot = r.joint(Some(knee), "foot", v(side * 0.80 * s, 0.0, zc + spread));
            r.cone(root, knee, 0.06 * s, 0.05 * s, 2, 0.02 * s);
            r.cone(knee, foot, 0.05 * s, 0.03 * s, 2, 0.02 * s);
            legs.push(vec![root, knee, foot]);
        }
    }
    r.finish(GaitDesc {
        legs,
        spine: vec![ceph, abdo],
        wings: Vec::new(),
        tail: Vec::new(),
        head: None,
        style: Gait::Crawl,
    })
}

/// M6f — Insectoid: head + thorax + elongated abdomen, 6 legs off the thorax,
/// 2 antennae off the head.
fn plan_insectoid(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let thorax = r.joint(None, "thorax", v(0.0, 0.42 * s, 0.10 * s));
    let head = r.joint(Some(thorax), "head", v(0.0, 0.44 * s, 0.55 * s));
    let abdo = r.joint(Some(thorax), "abdomen", v(0.0, 0.44 * s, -0.55 * s));
    let abtip = r.joint(Some(abdo), "abtip", v(0.0, 0.40 * s, -1.05 * s));
    r.ellip(thorax, thorax, 0.20 * s, 0.0, 0, 0.06 * s);
    r.ellip(head, head, 0.15 * s, 0.0, 0, 0.05 * s);
    r.ellip(abdo, abtip, 0.18 * s, 0.02 * s, 0, 0.06 * s);
    r.cone(thorax, head, 0.10 * s, 0.12 * s, 0, 0.04 * s);
    r.cone(thorax, abdo, 0.11 * s, 0.14 * s, 0, 0.04 * s);
    // antennae (part of the trunk/head)
    let hp = r.wpos(head);
    let al = r.joint(Some(head), "antL", hp + v(0.10 * s, 0.28 * s, 0.20 * s));
    let ar = r.joint(Some(head), "antR", hp + v(-0.10 * s, 0.28 * s, 0.20 * s));
    r.cone(head, al, 0.03 * s, 0.012 * s, 1, 0.02 * s);
    r.cone(head, ar, 0.03 * s, 0.012 * s, 1, 0.02 * s);
    // 6 legs off the thorax
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        for kk in 0..3 {
            let zc = 0.28 * s - kk as f32 * 0.22 * s;
            let root = r.joint(Some(thorax), "coxa", v(side * 0.16 * s, 0.40 * s, zc));
            let knee = r.joint(
                Some(root),
                "knee",
                v(side * 0.42 * s, 0.55 * s, zc + 0.05 * s),
            );
            let foot = r.joint(
                Some(knee),
                "foot",
                v(side * 0.58 * s, 0.0, zc + 0.12 * s * (1.0 - kk as f32)),
            );
            r.cone(root, knee, 0.05 * s, 0.04 * s, 2, 0.018 * s);
            r.cone(knee, foot, 0.04 * s, 0.028 * s, 2, 0.02 * s);
            legs.push(vec![root, knee, foot]);
        }
    }
    r.finish(GaitDesc {
        legs,
        spine: vec![head, thorax, abdo, abtip],
        wings: Vec::new(),
        tail: Vec::new(),
        head: Some(head),
        style: Gait::Crawl,
    })
}

/// M6g — Aberration: a central mass with N drooping tentacle chains radiating
/// out (seeded angles). Tentacles bind like legs; the mass pulses.
fn plan_aberration(p: &MonsterParams) -> MonsterRig {
    let s = p.size.clamp(0.2, 4.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let core = r.joint(None, "core", v(0.0, 0.60 * s, 0.0));
    let lower = r.joint(Some(core), "lower", v(0.0, 0.42 * s, 0.0));
    r.ellip(core, core, 0.34 * s, 0.0, 0, 0.10 * s);
    r.ellip(lower, lower, 0.26 * s, 0.0, 0, 0.09 * s);
    let mut rr = rng(p.seed);
    let ntent = 6usize;
    let mut legs = Vec::new();
    for i in 0..ntent {
        let ang = i as f32 / ntent as f32 * TAU + range(&mut rr, -0.3, 0.3);
        let dir = v(ang.cos(), 0.0, ang.sin());
        let h = 0.55 * s + range(&mut rr, -0.1, 0.1) * s;
        let p0 = v(0.0, h, 0.0) + dir * 0.30 * s;
        let j0 = r.joint(Some(core), "t0", p0);
        let p1 = p0 + dir * 0.28 * s + v(0.0, -0.18 * s, 0.0);
        let j1 = r.joint(Some(j0), "t1", p1);
        let p2 = p1 + dir * 0.24 * s + v(0.0, -0.28 * s, 0.0);
        let j2 = r.joint(Some(j1), "t2", p2);
        let p3 = p2 + dir * 0.18 * s + v(0.0, -0.30 * s, 0.0);
        let j3 = r.joint(Some(j2), "t3", p3);
        r.cone(j0, j1, 0.09 * s, 0.07 * s, 2, 0.02 * s);
        r.cone(j1, j2, 0.07 * s, 0.05 * s, 2, 0.02 * s);
        r.cone(j2, j3, 0.05 * s, 0.02 * s, 2, 0.02 * s);
        legs.push(vec![j0, j1, j2, j3]);
    }
    r.finish(GaitDesc {
        legs,
        spine: vec![core, lower],
        wings: Vec::new(),
        tail: Vec::new(),
        head: None,
        style: Gait::Pulse,
    })
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

    fn rig_for(body: &str) -> MonsterRig {
        let p: MonsterParams = serde_json::from_str(&format!("{{\"body\":\"{body}\"}}")).unwrap();
        build_rig(&p)
    }

    /// Every plan must reference only valid joints and have a rank-0 core.
    fn assert_wellformed(rig: &MonsterRig) {
        let n = rig.skeleton.joints.len();
        assert!(rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(rig.prims.iter().any(|d| d.fold_rank == 0));
        for chain in &rig.gait.legs {
            assert!(chain.iter().all(|&j| j < n));
        }
        assert!(rig.gait.spine.iter().all(|&j| j < n));
    }

    #[test]
    fn ooze_plan() {
        let rig = rig_for("ooze");
        assert!(rig.gait.legs.is_empty());
        assert!(matches!(rig.gait.style, Gait::Pulse));
        assert_wellformed(&rig);
    }

    #[test]
    fn serpent_plan() {
        let rig = rig_for("serpent");
        assert!(rig.gait.spine.len() >= 8, "spine {}", rig.gait.spine.len());
        assert!(matches!(rig.gait.style, Gait::Slither));
        assert_wellformed(&rig);
    }

    #[test]
    fn biped_plan() {
        let rig = rig_for("biped_brute");
        assert_eq!(rig.gait.legs.len(), 2);
        assert!(matches!(rig.gait.style, Gait::Walk));
        assert_wellformed(&rig);
    }

    #[test]
    fn flyer_plan() {
        let rig = rig_for("winged_flyer");
        assert!(!rig.gait.wings.is_empty());
        assert!(matches!(rig.gait.style, Gait::Fly));
        assert_wellformed(&rig);
    }

    #[test]
    fn arachnid_plan() {
        let rig = rig_for("arachnid");
        assert!(rig.gait.legs.len() >= 6, "legs {}", rig.gait.legs.len());
        assert!(matches!(rig.gait.style, Gait::Crawl));
        assert_wellformed(&rig);
    }

    #[test]
    fn insectoid_plan() {
        let rig = rig_for("insectoid");
        assert_eq!(rig.gait.legs.len(), 6);
        assert!(matches!(rig.gait.style, Gait::Crawl));
        assert_wellformed(&rig);
    }

    #[test]
    fn aberration_plan() {
        let rig = rig_for("aberration");
        assert!(
            rig.gait.legs.len() >= 4,
            "tentacles {}",
            rig.gait.legs.len()
        );
        assert!(matches!(rig.gait.style, Gait::Pulse));
        assert_wellformed(&rig);
    }
}
