//! Animated humanoid characters: parameterized proportions, class gear,
//! 17-joint skeleton, rigid-bound low-poly body, idle + walk clips.

use glam::{Mat4, Quat, Vec3};
use rand::Rng;

use crate::gltf::{
    AnimationClip, Asset, Channel, ChannelData, Collider, Joint, Material, Part, Physics,
    Skeleton,
};
use crate::mesh::{Mesh, cuboid, icosphere, lathe, to_flat_shaded, tube};
use crate::palette::{Palette, lerp, srgb, vary};
use crate::recipe::{CharacterClass, CharacterParams};

use super::{Rand, range, rng};

// joint indices
const HIPS: usize = 0;
const SPINE: usize = 1;
const CHEST: usize = 2;
const NECK: usize = 3;
const HEAD: usize = 4;
const ARM_L: usize = 5;
const FOREARM_L: usize = 6;
const HAND_L: usize = 7;
const ARM_R: usize = 8;
const FOREARM_R: usize = 9;
const HAND_R: usize = 10;
const THIGH_L: usize = 11;
const SHIN_L: usize = 12;
const FOOT_L: usize = 13;
const THIGH_R: usize = 14;
const SHIN_R: usize = 15;
const FOOT_R: usize = 16;

struct Rig {
    skeleton: Skeleton,
    /// world-space bind position of each joint
    world: Vec<Vec3>,
}

fn build_rig(h: f32, shoulder_w: f32) -> Rig {
    let mut joints: Vec<(usize, Option<usize>, &str, Vec3)> = Vec::new();
    let hip_y = h * 0.47;
    joints.push((HIPS, None, "hips", Vec3::new(0.0, hip_y, 0.0)));
    joints.push((SPINE, Some(HIPS), "spine", Vec3::new(0.0, h * 0.09, 0.0)));
    joints.push((CHEST, Some(SPINE), "chest", Vec3::new(0.0, h * 0.12, 0.0)));
    joints.push((NECK, Some(CHEST), "neck", Vec3::new(0.0, h * 0.10, 0.0)));
    joints.push((HEAD, Some(NECK), "head", Vec3::new(0.0, h * 0.045, 0.0)));
    let sw = shoulder_w;
    // relaxed stance: the upper arm drifts slightly outward, the forearm
    // returns inward with a soft elbow bend, the hand settles by the thigh
    joints.push((ARM_L, Some(CHEST), "upperarm_l", Vec3::new(sw * 0.94, h * 0.075, 0.0)));
    joints.push((FOREARM_L, Some(ARM_L), "forearm_l", Vec3::new(h * 0.015, -h * 0.148, h * 0.010)));
    joints.push((HAND_L, Some(FOREARM_L), "hand_l", Vec3::new(h * 0.014, -h * 0.126, h * 0.016)));
    joints.push((ARM_R, Some(CHEST), "upperarm_r", Vec3::new(-sw * 0.94, h * 0.075, 0.0)));
    joints.push((FOREARM_R, Some(ARM_R), "forearm_r", Vec3::new(-h * 0.015, -h * 0.148, h * 0.010)));
    joints.push((HAND_R, Some(FOREARM_R), "hand_r", Vec3::new(-h * 0.014, -h * 0.126, h * 0.016)));
    joints.push((THIGH_L, Some(HIPS), "thigh_l", Vec3::new(h * 0.068, -h * 0.02, 0.0)));
    joints.push((SHIN_L, Some(THIGH_L), "shin_l", Vec3::new(0.0, -h * 0.22, 0.0)));
    joints.push((FOOT_L, Some(SHIN_L), "foot_l", Vec3::new(0.0, -h * 0.20, 0.0)));
    joints.push((THIGH_R, Some(HIPS), "thigh_r", Vec3::new(-h * 0.068, -h * 0.02, 0.0)));
    joints.push((SHIN_R, Some(THIGH_R), "shin_r", Vec3::new(0.0, -h * 0.22, 0.0)));
    joints.push((FOOT_R, Some(SHIN_R), "foot_r", Vec3::new(0.0, -h * 0.20, 0.0)));

    let skeleton = Skeleton {
        joints: joints
            .iter()
            .map(|(_, parent, name, t)| Joint {
                name: (*name).into(),
                parent: *parent,
                translation: *t,
                rotation: Quat::IDENTITY,
            })
            .collect(),
    };
    let world = (0..skeleton.joints.len())
        .map(|i| skeleton.global(i).transform_point3(Vec3::ZERO))
        .collect();
    Rig { skeleton, world }
}

struct Wardrobe {
    skin: Vec3,
    hair: Vec3,
    shirt: Vec3,
    pants: Vec3,
    boots: Vec3,
    accent: Vec3,
}

fn wardrobe(r: &mut Rand, pal: &Palette, class: CharacterClass, tone: Option<u32>) -> Wardrobe {
    let skins = [
        srgb(236, 188, 152),
        srgb(210, 158, 118),
        srgb(172, 120, 84),
        srgb(120, 80, 56),
    ];
    let hairs = [
        srgb(48, 36, 30),
        srgb(140, 96, 52),
        srgb(210, 180, 120),
        srgb(90, 90, 96),
        srgb(150, 60, 40),
    ];
    let pick = r.gen_range(0..skins.len());
    let skin = skins[tone.map(|t| t as usize % skins.len()).unwrap_or(pick)];
    let hair = hairs[r.gen_range(0..hairs.len())];
    match class {
        CharacterClass::Warrior => Wardrobe {
            skin,
            hair,
            shirt: srgb(120, 124, 134),
            pants: pal.trunk * 0.9,
            boots: srgb(70, 54, 42),
            accent: pal.accent,
        },
        CharacterClass::Mage => Wardrobe {
            skin,
            hair,
            shirt: lerp(pal.accent, srgb(60, 50, 110), 0.5),
            pants: srgb(56, 48, 88),
            boots: srgb(60, 48, 40),
            accent: lerp(pal.accent, Vec3::ONE, 0.3),
        },
        CharacterClass::Rogue => Wardrobe {
            skin,
            hair,
            shirt: srgb(64, 70, 62),
            pants: srgb(52, 48, 46),
            boots: srgb(40, 36, 34),
            accent: pal.accent,
        },
        CharacterClass::Villager => Wardrobe {
            skin,
            hair,
            shirt: vary(pal.foliage[1], 0.15, range(r, 0.0, 1.0)),
            pants: srgb(110, 90, 66),
            boots: srgb(84, 64, 50),
            accent: pal.accent,
        },
    }
}


/// Body v7: ONE smoothly-blended SDF (pelvis, glutes, belly, chest, traps,
/// deltoids, tapered limbs under smooth-min) meshed with surface nets — a
/// single continuous organic surface, no part seams. Colors resolve by the
/// dominant sub-field; skin weights are family-restricted (torso verts can
/// never grab arm bones — the phase-2 flying-shoulder lesson) with the
/// pelvis/glutes rigid to HIPS so strides can't tear the crotch.
#[allow(clippy::too_many_arguments)]
fn organic_body(
    rig: &Rig,
    h: f32,
    bulk: f32,
    sw: f32,
    w: &Wardrobe,
    forearm_col: Vec3,
    det: f32,
) -> Mesh {
    use crate::sdf::{sd_ellipsoid, sd_round_cone, sd_sphere, smin};
    let jw = |i: usize| rig.world[i];
    let (hips, spine, chest, neck) = (jw(HIPS), jw(SPINE), jw(CHEST), jw(NECK));
    let arm_r = h * 0.036 * bulk;
    let leg_r = h * 0.052 * bulk;

    #[derive(Clone, Copy, PartialEq)]
    enum Fam {
        Pelvis,
        Torso,
        ArmL,
        ArmR,
        LegL,
        LegR,
    }
    enum Shape {
        Sphere(Vec3, f32),
        Cone(Vec3, Vec3, f32, f32),
        ConeScaled(Vec3, Vec3, f32, f32, Vec3, Vec3),
        Ellipsoid(Vec3, Vec3),
    }
    impl Shape {
        fn eval(&self, p: Vec3) -> f32 {
            match *self {
                Shape::Sphere(c, r) => sd_sphere(p, c, r),
                Shape::Cone(a, b, r1, r2) => sd_round_cone(p, a, b, r1, r2),
                Shape::ConeScaled(a, b, r1, r2, o, s) => {
                    crate::sdf::sd_round_cone_scaled(p, a, b, r1, r2, o, s)
                }
                Shape::Ellipsoid(c, r) => sd_ellipsoid(p, c, r),
            }
        }
    }
    let mut parts: Vec<(Fam, Vec3, Shape)> = Vec::new();
    // hips & seat
    parts.push((
        Fam::Pelvis,
        w.pants,
        Shape::Ellipsoid(
            hips + Vec3::new(0.0, -h * 0.020, 0.0),
            Vec3::new(h * 0.106 * bulk, h * 0.068, h * 0.078 * bulk),
        ),
    ));
    for s in [-1.0f32, 1.0] {
        parts.push((
            Fam::Pelvis,
            w.pants,
            Shape::Sphere(
                hips + Vec3::new(s * h * 0.046 * bulk, -h * 0.048, -h * 0.036 * bulk),
                h * 0.049 * bulk,
            ),
        ));
    }
    // belly + chest (elliptical cross-sections), subtle forward lean
    parts.push((
        Fam::Torso,
        w.shirt,
        Shape::Ellipsoid(
            Vec3::new(0.0, spine.y + 0.005 * h, 0.006 * h),
            Vec3::new(h * 0.096 * bulk, h * 0.075, h * 0.070 * bulk),
        ),
    ));
    {
        let a = Vec3::new(0.0, spine.y + h * 0.060, h * 0.008);
        let b = Vec3::new(0.0, chest.y + h * 0.055, h * 0.012);
        let o = (a + b) * 0.5;
        parts.push((
            Fam::Torso,
            w.shirt,
            Shape::ConeScaled(a, b, h * 0.060 * bulk, h * 0.068 * bulk, o, Vec3::new(1.32, 1.0, 0.86)),
        ));
    }
    // trapezius wedges into the neck
    for s in [-1.0f32, 1.0] {
        parts.push((
            Fam::Torso,
            w.shirt,
            Shape::Cone(
                Vec3::new(0.0, neck.y + h * 0.004, -h * 0.008),
                Vec3::new(s * sw * 0.78, chest.y + h * 0.075, 0.0),
                h * 0.024,
                h * 0.030 * bulk,
            ),
        ));
    }
    // shoulder bar ties the deltoids through the chest
    parts.push((
        Fam::Torso,
        w.shirt,
        Shape::Cone(
            Vec3::new(-sw * 0.55, chest.y + h * 0.068, 0.0),
            Vec3::new(sw * 0.55, chest.y + h * 0.068, 0.0),
            h * 0.040 * bulk,
            h * 0.040 * bulk,
        ),
    ));
    // neck column
    parts.push((
        Fam::Torso,
        w.skin,
        Shape::Cone(
            Vec3::new(0.0, neck.y - h * 0.020, 0.0),
            Vec3::new(0.0, neck.y + h * 0.050, h * 0.004),
            h * 0.036,
            h * 0.029,
        ),
    ));
    // arms: deltoid + tapered upper/forearm
    for (fam, aj, fj, hj) in
        [(Fam::ArmL, ARM_L, FOREARM_L, HAND_L), (Fam::ArmR, ARM_R, FOREARM_R, HAND_R)]
    {
        let (a, f, hd) = (jw(aj), jw(fj), jw(hj));
        parts.push((fam, w.shirt, Shape::Sphere(a + Vec3::new(0.0, h * 0.008, 0.0), h * 0.046 * bulk)));
        parts.push((fam, w.shirt, Shape::Cone(a + Vec3::new(0.0, -h * 0.010, 0.0), f, arm_r * 0.94, arm_r * 0.78)));
        parts.push((fam, forearm_col, Shape::Cone(f, hd + Vec3::new(0.0, h * 0.010, 0.0), arm_r * 0.80, arm_r * 0.58)));
    }
    // legs: thigh, calf, boot shaft
    for (fam, tj, sj, fj) in
        [(Fam::LegL, THIGH_L, SHIN_L, FOOT_L), (Fam::LegR, THIGH_R, SHIN_R, FOOT_R)]
    {
        let (t, s, f) = (jw(tj), jw(sj), jw(fj));
        parts.push((fam, w.pants, Shape::Cone(t + Vec3::new(0.0, h * 0.055, 0.0), s, leg_r * 0.97, leg_r * 0.80)));
        parts.push((fam, w.pants, Shape::Cone(s, s + (f - s) * 0.55, leg_r * 0.86, leg_r * 0.72)));
        parts.push((fam, w.boots, Shape::Cone(s + (f - s) * 0.45, f + Vec3::new(0.0, h * 0.012, 0.0), leg_r * 0.72, leg_r * 0.50)));
    }

    // hierarchical blending: soft flesh fillets WITHIN the trunk and within
    // each limb, but a tight junction BETWEEN trunk and limbs — a deep
    // armpit/groin crease instead of a webbed bridge that stretches when
    // the limb swings
    let k_soft = h * 0.022;
    let k_tight = h * 0.008;
    let field = |p: Vec3| -> f32 {
        let mut groups = [f32::INFINITY; 6];
        for (f, _, s) in &parts {
            let i = match f {
                Fam::Pelvis => 0usize,
                Fam::Torso => 0, // pelvis+torso are one flesh mass
                Fam::ArmL => 2,
                Fam::ArmR => 3,
                Fam::LegL => 4,
                Fam::LegR => 5,
            };
            groups[i] = smin(groups[i], s.eval(p), k_soft);
        }
        let mut d = groups[0];
        // ORDER MATTERS: legs fold into the trunk FIRST (soft flesh at the
        // groin), and arms fold LAST with a shoulder-ramped fillet that
        // hits hard-min below the chest — otherwise the hand hanging beside
        // the thigh fuses to it through the leg blend and every big arm
        // swing drags a membrane along (found via stretched-triangle probe:
        // forearm-weighted verts sharing triangles with thigh-weighted ones)
        for g in [groups[4], groups[5]] {
            d = smin(d, g, h * 0.016);
        }
        let shoulder_t = ((p.y - spine.y) / (chest.y - spine.y)).clamp(0.0, 1.0);
        let k_arm = k_tight * shoulder_t * shoulder_t;
        for g in [groups[2], groups[3]] {
            d = smin(d, g, k_arm);
        }
        d
    };
    // family classifier: min distance per family, with limbs biased so
    // smooth-min blend zones (hand-near-hip, neck-chest) resolve to the
    // trunk instead of flipping noisily — this drives BOTH color regions
    // and weight families, so a hip vertex can never grab an arm bone
    let fi = |f: Fam| match f {
        Fam::Pelvis => 0,
        Fam::Torso => 1,
        Fam::ArmL => 2,
        Fam::ArmR => 3,
        Fam::LegL => 4,
        Fam::LegR => 5,
    };
    let fam_dists = |p: Vec3| -> [f32; 6] {
        let mut d = [f32::INFINITY; 6];
        for (f, _, s) in &parts {
            let i = fi(*f);
            d[i] = d[i].min(s.eval(p));
        }
        d
    };
    let fam_of = |p: Vec3| -> Fam {
        let d = fam_dists(p);
        let bias = h * 0.008;
        let ranked = [
            (d[0], Fam::Pelvis),
            (d[1], Fam::Torso),
            (d[2] + bias, Fam::ArmL),
            (d[3] + bias, Fam::ArmR),
            (d[4] + bias * 0.5, Fam::LegL),
            (d[5] + bias * 0.5, Fam::LegR),
        ];
        let mut best = ranked[0];
        for r in &ranked[1..] {
            if r.0 < best.0 {
                best = *r;
            }
        }
        best.1
    };
    // tailored color boundaries: garments end on clean lines (belt, elbow,
    // mid-shin), not on noisy field-dominance blobs
    let along = |p: Vec3, a: Vec3, b: Vec3| -> f32 {
        let ab = b - a;
        (p - a).dot(ab) / ab.length_squared().max(1e-12)
    };
    let color = |p: Vec3| -> Vec3 {
        match fam_of(p) {
            Fam::Pelvis | Fam::Torso => {
                // skin only on the neck column itself, not the chest slope
                let neck_r = (p.x * p.x + p.z * p.z).sqrt();
                if p.y > neck.y - h * 0.002 && neck_r < h * 0.048 {
                    w.skin
                } else if p.y > hips.y + h * 0.030 {
                    w.shirt
                } else {
                    w.pants
                }
            }
            Fam::ArmL | Fam::ArmR => {
                let (aj, fj) = if p.x > 0.0 { (ARM_L, FOREARM_L) } else { (ARM_R, FOREARM_R) };
                if along(p, jw(aj), jw(fj)) < 1.04 { w.shirt } else { forearm_col }
            }
            Fam::LegL | Fam::LegR => {
                let (sj, fj) = if p.x > 0.0 { (SHIN_L, FOOT_L) } else { (SHIN_R, FOOT_R) };
                if along(p, jw(sj), jw(fj)) > 0.42 { w.boots } else { w.pants }
            }
        }
    };

    let cell = (h * 0.011 / det).min(h * 0.015);
    let lo = Vec3::new(-sw - h * 0.07, -cell, -h * 0.13 * bulk - h * 0.02);
    let hi = Vec3::new(sw + h * 0.07, neck.y + h * 0.09, h * 0.13 * bulk + h * 0.03);
    let mut m = crate::sdf::mesh_field(lo, hi, cell, &field, &color);

    // family-restricted smooth weights: inverse-distance over the family's
    // bone segments only; pelvis/glutes rigid to HIPS
    let seg_w = |p: Vec3, pairs: &[(usize, usize)]| -> ([u16; 4], [f32; 4]) {
        let mut joints = [0u16; 4];
        let mut weights = [0f32; 4];
        let mut sum = 0.0;
        for (i, &(ja, jb)) in pairs.iter().enumerate().take(4) {
            let (a, b) = (jw(ja), jw(jb));
            let ab = b - a;
            let t = if ab.length_squared() < 1e-12 {
                0.0
            } else {
                ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
            };
            let d = p.distance(a + ab * t);
            let wgt = 1.0 / (d + 1e-4).powi(4);
            joints[i] = ja as u16;
            weights[i] = wgt;
            sum += wgt;
        }
        for w in &mut weights {
            *w /= sum;
        }
        (joints, weights)
    };
    m.joints = Vec::with_capacity(m.positions.len());
    m.weights = Vec::with_capacity(m.positions.len());
    // strongest two influences of a 4-slot weight set
    let top2 = |j: [u16; 4], w: [f32; 4]| -> [(u16, f32); 2] {
        let mut v: Vec<(u16, f32)> = j.iter().copied().zip(w.iter().copied()).collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        [v[0], v[1]]
    };
    for &p in &m.positions.clone() {
        let d = fam_dists(p);
        // trunk weights (pelvis rigid, torso on the spine chain)
        let trunk_d = d[0].min(d[1]);
        let trunk: ([u16; 4], [f32; 4]) = if d[0] < d[1] {
            ([HIPS as u16, 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
        } else {
            seg_w(p, &[(HIPS, SPINE), (SPINE, CHEST), (CHEST, NECK)])
        };
        // nearest limb family
        let limbs = [
            (d[2], [(ARM_L, FOREARM_L), (FOREARM_L, HAND_L)]),
            (d[3], [(ARM_R, FOREARM_R), (FOREARM_R, HAND_R)]),
            (d[4], [(THIGH_L, SHIN_L), (SHIN_L, FOOT_L)]),
            (d[5], [(THIGH_R, SHIN_R), (SHIN_R, FOOT_R)]),
        ];
        let (limb_d, limb_pairs) = limbs
            .iter()
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(dd, pr)| (*dd, *pr))
            .unwrap();
        // smooth transition across the junction: hard switches leave a
        // stretched membrane in the armpit the moment an arm is raised
        // blend ONLY at true junctions (both families within reach) —
        // otherwise even 10% cross-weight smears a surface into a web the
        // moment the limb rotates far from its bind pose
        let near = h * 0.012;
        let tw = h * 0.020;
        let s = ((trunk_d - limb_d) / tw * 0.5 + 0.5).clamp(0.0, 1.0);
        let s = s * s * (3.0 - 2.0 * s);
        let (j, wt) = if trunk_d <= limb_d && limb_d > near {
            trunk
        } else if limb_d < trunk_d && trunk_d > near {
            seg_w(p, &limb_pairs)
        } else if s <= 0.001 {
            trunk
        } else if s >= 0.999 {
            seg_w(p, &limb_pairs)
        } else {
            let t2 = top2(trunk.0, trunk.1);
            let l = seg_w(p, &limb_pairs);
            let l2 = top2(l.0, l.1);
            let joints = [t2[0].0, t2[1].0, l2[0].0, l2[1].0];
            let mut weights = [
                t2[0].1 * (1.0 - s),
                t2[1].1 * (1.0 - s),
                l2[0].1 * s,
                l2[1].1 * s,
            ];
            let sum: f32 = weights.iter().sum();
            for w in &mut weights {
                *w /= sum;
            }
            (joints, weights)
        };
        m.joints.push(j);
        m.weights.push(wt);
    }
    m
}

/// Boot: sculpted from a shared-vertex icosphere (cuboids are flat-shaded
/// islands, so subdivision can never round them — learned the hard way):
/// flattened sole, rounded heel, tapered toe box, plus an ankle cuff that
/// overlaps the shin so the leg never floats.
fn boot(ankle: Vec3, shin_r: f32, h: f32, color: Vec3) -> Mesh {
    let len = h * 0.062; // half-length heel→toe
    let wdt = shin_r * 1.55;
    let top = ankle.y + shin_r * 1.05; // cuff height above the sole
    let mut m = icosphere(1.0, 3, color);
    for v in m.positions.iter_mut() {
        // flatten the sole: clamp the lower cap, keep verts shared
        let yu = if v.y < -0.45 { -0.45 - (v.y + 0.45) * 0.06 } else { v.y };
        let tz = v.z.max(0.0); // 0 at heel/mid, →1 at toe
        // wedge: the toe box sits lower than the ankle
        let height = top * (1.0 - 0.52 * tz.powf(1.6));
        v.y = (yu + 0.47) / 1.47 * height;
        // toe narrows slightly, heel tucks in
        v.x *= wdt * (1.0 - 0.18 * tz) * (1.0 - 0.10 * (-v.z).max(0.0));
        v.z = v.z * len + len * 0.52; // shift so the heel sits behind the ankle
    }
    // leather detailing: dark sole line, burnished toe cap
    for i in 0..m.positions.len() {
        let v = m.positions[i];
        if v.y < top * 0.10 {
            m.colors[i] = color * 0.55;
        } else if v.z > len * 1.10 {
            m.colors[i] = color * 1.18;
        }
    }
    m.recompute_smooth_normals();
    m.translate(Vec3::new(ankle.x, 0.0, ankle.z - len * 0.18));
    // ankle cuff: rolled boot top swallowing the shin bottom
    let mut cuff = lathe(
        &[
            (shin_r * 1.18, ankle.y + shin_r * 0.20),
            (shin_r * 1.30, ankle.y + shin_r * 0.85),
            (shin_r * 1.22, ankle.y + shin_r * 1.35),
            (shin_r * 0.95, ankle.y + shin_r * 1.45),
        ],
        10,
        |_, _| color * 0.88,
    );
    cuff = crate::subdiv::subdivide(&cuff, false);
    cuff.translate(Vec3::new(ankle.x, 0.0, ankle.z));
    m.merge(&cuff);
    m
}

/// Mitten hand: squashed palm sphere + thumb blob.
fn mitten(at: Vec3, r: f32, side: f32, skin: Vec3, subdiv: u32) -> Mesh {
    let mut palm = icosphere(r, subdiv, skin);
    for v in palm.positions.iter_mut() {
        v.x *= 0.70;
        v.y *= 1.18;
        v.z *= 0.88;
    }
    palm.recompute_smooth_normals();
    // three chunky fingers hanging from the palm + a thumb: reads as a real
    // hand at game distance without per-finger rigging
    for (fx, s) in [(-0.40f32, 0.29f32), (0.0, 0.33), (0.40, 0.27)] {
        let mut f = icosphere(r * s, 1, skin);
        for v in f.positions.iter_mut() {
            v.y *= 1.85;
        }
        f.recompute_smooth_normals();
        f.translate(Vec3::new(fx * r * 0.68, -r * 1.02, r * 0.12));
        palm.merge(&f);
    }
    let mut thumb = icosphere(r * 0.30, 1, skin);
    for v in thumb.positions.iter_mut() {
        v.y *= 1.5;
    }
    thumb.recompute_smooth_normals();
    thumb.translate(Vec3::new(side * r * 0.60, -r * 0.30, r * 0.45));
    palm.merge(&thumb);
    // relaxed curl: fingertips drift in toward the leg, palm faces the thigh
    palm.transform(Mat4::from_rotation_x(0.16) * Mat4::from_rotation_z(-side * 0.10));
    palm.translate(at);
    palm
}

/// Shaped head: tapered-jaw ellipsoid, smooth-shaded.
fn head_shape(r: f32, skin: Vec3, subdiv: u32) -> Mesh {
    let mut head = icosphere(r, subdiv, skin);
    for v in head.positions.iter_mut() {
        // ellipsoid base
        v.x *= 0.88;
        v.y *= 1.08;
        v.z *= 0.95;
        // jaw taper below the midline, chin stays forward
        if v.y < 0.0 {
            let t = (-v.y / r).min(1.0);
            v.x *= 1.0 - 0.32 * t;
            v.z *= 1.0 - 0.18 * t;
        }
    }
    head.recompute_smooth_normals();
    head
}

/// Face features around head center `c` with per-feature morph deltas
/// (smile, blink, angry, surprised) when `expressions` is on.
fn face(c: Vec3, r: f32, w: &Wardrobe, expressions: bool) -> Mesh {
    use crate::mesh::MorphTarget;
    let mut out = Mesh::new();
    let lip = w.skin * 0.62;
    let white = srgb(245, 243, 238);
    let iris = srgb(48, 40, 36);

    let mut push = |mut m: Mesh, morph_fn: &dyn Fn(&Mesh) -> Vec<(&'static str, Vec<Vec3>)>| {
        if expressions {
            let morphs = morph_fn(&m);
            m.morphs = morphs
                .into_iter()
                .map(|(name, deltas)| MorphTarget { name: name.into(), deltas })
                .collect();
        }
        out.merge(&m);
    };

    // eyes: white + pupil, blink flattens, surprised widens
    for sx in [-1.0f32, 1.0] {
        let ec = c + Vec3::new(sx * r * 0.30, r * 0.10, r * 0.83);
        let mut white_m = icosphere(r * 0.16, 2, white);
        for v in white_m.positions.iter_mut() {
            v.y *= 0.8;
            v.z *= 0.5;
        }
        white_m.recompute_smooth_normals();
        white_m.translate(ec);
        let mut pupil = icosphere(r * 0.07, 1, iris);
        for v in pupil.positions.iter_mut() {
            v.z *= 0.5;
        }
        pupil.recompute_smooth_normals();
        pupil.translate(ec + Vec3::new(0.0, 0.0, r * 0.085));
        white_m.merge(&pupil);
        push(white_m, &|m: &Mesh| {
            let blink: Vec<Vec3> =
                m.positions.iter().map(|p| Vec3::new(0.0, (ec.y - p.y) * 0.85, -0.001)).collect();
            let surprised: Vec<Vec3> =
                m.positions.iter().map(|p| (*p - ec) * 0.22).collect();
            vec![("blink", blink), ("surprised", surprised)]
        });
    }

    // brows: angry tilts inner ends down, surprised raises
    for sx in [-1.0f32, 1.0] {
        let bc = c + Vec3::new(sx * r * 0.30, r * 0.32, r * 0.85);
        let mut brow = cuboid(Vec3::ZERO, Vec3::new(r * 0.16, r * 0.035, r * 0.04), w.hair * 0.8);
        brow.transform(
            Mat4::from_translation(bc) * Mat4::from_rotation_z(sx * 0.1) * Mat4::from_rotation_x(-0.25),
        );
        push(brow, &|m: &Mesh| {
            let angry: Vec<Vec3> = m
                .positions
                .iter()
                .map(|p| {
                    // inner end (toward face midline) dips
                    let inner = 1.0 - ((p.x - bc.x) * sx / (r * 0.32) + 0.5).clamp(0.0, 1.0);
                    Vec3::new(0.0, -r * 0.10 * inner, 0.0)
                })
                .collect();
            let surprised: Vec<Vec3> =
                m.positions.iter().map(|_| Vec3::new(0.0, r * 0.09, 0.0)).collect();
            vec![("angry", angry), ("surprised", surprised)]
        });
    }

    // nose: small smooth wedge
    let mut nose = icosphere(r * 0.085, 2, w.skin * 1.04);
    for v in nose.positions.iter_mut() {
        v.x *= 0.7;
        v.z *= 1.25;
    }
    nose.recompute_smooth_normals();
    nose.translate(c + Vec3::new(0.0, -r * 0.08, r * 0.94));
    push(nose, &|_| vec![]);

    // mouth: smile lifts corners, angry drops them, surprised opens
    let mc = c + Vec3::new(0.0, -r * 0.38, r * 0.82);
    let mw = r * 0.24;
    let mut mouth = cuboid(Vec3::ZERO, Vec3::new(mw, r * 0.035, r * 0.03), lip);
    mouth = crate::subdiv::subdivide(&mouth, false); // verts along the lip line
    mouth.transform(Mat4::from_translation(mc) * Mat4::from_rotation_x(-0.35));
    push(mouth, &|m: &Mesh| {
        let corner = |p: &Vec3| (p.x - mc.x).abs() / mw;
        let smile: Vec<Vec3> = m
            .positions
            .iter()
            .map(|p| Vec3::new(0.0, r * 0.09 * corner(p).powf(1.5), r * 0.02 * corner(p)))
            .collect();
        let angry: Vec<Vec3> = m
            .positions
            .iter()
            .map(|p| Vec3::new(0.0, -r * 0.06 * corner(p).powf(1.5), 0.0))
            .collect();
        let surprised: Vec<Vec3> = m
            .positions
            .iter()
            .map(|p| {
                if p.y < mc.y { Vec3::new(0.0, -r * 0.10, 0.0) } else { Vec3::ZERO }
            })
            .collect();
        vec![("smile", smile), ("angry", angry), ("surprised", surprised)]
    });

    // ears
    for sx in [-1.0f32, 1.0] {
        let mut ear = icosphere(r * 0.10, 1, w.skin);
        for v in ear.positions.iter_mut() {
            v.x *= 0.5;
        }
        ear.recompute_smooth_normals();
        ear.translate(c + Vec3::new(sx * r * 0.84, 0.02 * r, 0.0));
        push(ear, &|_| vec![]);
    }

    out
}

/// Tapered double-sided ribbon strip along a guide curve — one hair card.
fn ribbon(guide: &[Vec3], w0: f32, w1: f32, color: Vec3, center: Vec3) -> Mesh {
    let mut m = Mesh::new();
    let n = guide.len();
    for (i, &p) in guide.iter().enumerate() {
        let t = i as f32 / (n - 1) as f32;
        let dir = if i + 1 < n { guide[i + 1] - p } else { p - guide[i - 1] };
        let dir = dir.normalize_or(Vec3::NEG_Y);
        // width direction lies on the scalp surface
        let out = (p - center).normalize_or(Vec3::Z);
        let side = dir.cross(out).normalize_or(Vec3::X);
        let w = (w0 + (w1 - w0) * t) / 2.0;
        m.push_vertex(p - side * w, out, color);
        m.push_vertex(p + side * w, out, color);
    }
    for i in 0..n - 1 {
        let a = (i * 2) as u32;
        m.push_tri(a, a + 1, a + 3);
        m.push_tri(a, a + 3, a + 2);
        // reversed faces so cards read from both sides
        m.push_tri(a, a + 3, a + 1);
        m.push_tri(a, a + 2, a + 3);
    }
    m
}

/// Strand guide: hug the skull early, hang toward `len` below with `drift`.
fn strand_guide(start: Vec3, center: Vec3, len: f32, drift: Vec3, samples: usize) -> Vec<Vec3> {
    let out = (start - center).normalize_or(Vec3::Z);
    (0..samples)
        .map(|i| {
            let t = i as f32 / (samples - 1) as f32;
            let hug = center + out * (start - center).length() * (1.0 + 0.16 * t);
            let hang = Vec3::new(start.x + drift.x, start.y - len, start.z + drift.z);
            let k = t * t * (3.0 - 2.0 * t);
            hug.lerp(hang, k * k)
        })
        .collect()
}

/// Long flowing hair / topknot built from clumped ribbon cards.
fn hair_cards(style: &str, c: Vec3, r: f32, color: Vec3, seed: u64) -> Mesh {
    let mut rr = rng(seed ^ 0x4A17);
    let mut m = Mesh::new();
    let strands = 26;
    for i in 0..strands {
        // golden-angle spread over the upper hemisphere
        let a = i as f32 * 2.399963;
        let y = 0.35 + 0.6 * (i as f32 / strands as f32);
        let ring = (1.0 - y * y).max(0.05).sqrt();
        let start = c + Vec3::new(a.cos() * ring, y, a.sin() * ring * 0.9) * r * 1.04;
        // front strands stay short (clear the face), back strands flow long
        let backness = ((c.z - start.z) / r).clamp(-1.0, 1.0);
        let base_len = match style {
            "topknot" => r * 0.5,
            _ => r * (1.6 + 1.3 * (backness * 0.5 + 0.5)),
        };
        let len = base_len * range(&mut rr, 0.85, 1.15);
        if style != "topknot" && start.z > c.z + r * 0.45 && start.y < c.y + r * 0.55 {
            continue; // never grow over the face
        }
        let drift = Vec3::new(
            range(&mut rr, -0.12, 0.12) * r,
            0.0,
            -r * (0.3 + 0.4 * (backness * 0.5 + 0.5)),
        );
        let guide = strand_guide(start, c, len, drift, 7);
        m.merge(&ribbon(
            &guide,
            r * 0.34,
            r * 0.10,
            vary(color, 0.12, range(&mut rr, 0.0, 1.0)),
            c,
        ));
    }
    if style == "topknot" {
        let mut knot = icosphere(r * 0.26, 2, color);
        knot.translate(c + Vec3::new(0.0, r * 0.95, -r * 0.15));
        m.merge(&knot);
        let start = c + Vec3::new(0.0, r * 0.95, -r * 0.35);
        let guide = strand_guide(start, c, r * 1.7, Vec3::new(0.0, 0.0, -r * 0.5), 7);
        m.merge(&ribbon(&guide, r * 0.28, r * 0.08, color * 0.94, c));
    }
    m
}

/// Ribbon-card facial hair: mustache and/or chin beard.
fn beard_cards(style: &str, c: Vec3, r: f32, color: Vec3, seed: u64) -> Mesh {
    let mut rr = rng(seed ^ 0xBEA6D);
    let mut m = Mesh::new();
    if style == "none" {
        return m;
    }
    // mustache: two arcs from under the nose, out and down
    for sx in [-1.0f32, 1.0] {
        let start = c + Vec3::new(sx * r * 0.10, -r * 0.22, r * 0.94);
        let guide: Vec<Vec3> = (0..5)
            .map(|i| {
                let t = i as f32 / 4.0;
                start
                    + Vec3::new(
                        sx * r * 0.30 * t,
                        -r * 0.16 * t * t - r * 0.05 * t,
                        -r * 0.12 * t,
                    )
            })
            .collect();
        m.merge(&ribbon(&guide, r * 0.18, r * 0.06, color, c));
    }
    if style == "mustache" {
        return m;
    }
    // chin/jaw beard: strands hanging from the jawline, center longest
    let len_mul = if style == "long" { 2.6 } else { 1.0 };
    let strands = 13;
    for i in 0..strands {
        let t = i as f32 / (strands - 1) as f32; // 0..1 across the jaw
        let ang = (t - 0.5) * 1.9;
        let start = c
            + Vec3::new(
                ang.sin() * r * 0.66,
                -r * 0.58 - (1.0 - ang.cos()) * r * 0.1,
                ang.cos() * r * 0.68,
            );
        let center_ness = 1.0 - (t - 0.5).abs() * 2.0;
        let len = r * (0.5 + 0.55 * center_ness) * len_mul * range(&mut rr, 0.9, 1.1);
        let guide = strand_guide(
            start,
            c + Vec3::new(0.0, -r * 0.3, 0.0),
            len,
            Vec3::new(0.0, 0.0, r * 0.16),
            6,
        );
        m.merge(&ribbon(
            &guide,
            r * 0.30,
            r * 0.08,
            vary(color, 0.10, range(&mut rr, 0.0, 1.0)),
            c,
        ));
    }
    m
}

/// Hair styles built from a clipped shell over the skull.
fn hair_mesh(style: &str, c: Vec3, r: f32, color: Vec3) -> Mesh {
    let mut cap = icosphere(r * 1.08, 3, color);
    for v in cap.positions.iter_mut() {
        v.x *= 0.92;
        v.y *= 1.04;
        // carve the face open
        if v.z > r * 0.30 && v.y < r * 0.42 {
            v.z = r * 0.30;
        }
        if v.y < -r * 0.18 {
            // tuck the lower rim inward so it doesn't flare like a brim
            v.y = -r * 0.18;
            v.x *= 0.92;
            v.z *= 0.92;
        }
    }
    cap.recompute_smooth_normals();
    let mut m = Mesh::new();
    match style {
        "bald" => {}
        "ponytail" => {
            m.merge(&cap);
            let tail = tube(
                &[
                    (c + Vec3::new(0.0, r * 0.55, -r * 0.85), r * 0.16),
                    (c + Vec3::new(0.0, -r * 0.1, -r * 1.15), r * 0.13),
                    (c + Vec3::new(0.0, -r * 0.9, -r * 1.0), r * 0.07),
                ],
                7,
                |_| color * 0.92,
            );
            let mut tail = crate::subdiv::subdivide(&tail, false);
            tail.translate(-c); // cap is positioned later relative to c
            m.merge(&tail);
        }
        "bun" => {
            m.merge(&cap);
            let mut bun = icosphere(r * 0.30, 2, color * 0.95);
            bun.translate(Vec3::new(0.0, r * 0.75, -r * 0.75));
            m.merge(&bun);
        }
        // "short" default
        _ => {
            m.merge(&cap);
        }
    }
    m.translate(c);
    m
}

/// Influence segments for one body region. Regions are bound independently:
/// binding the torso against arm segments makes shoulder-adjacent torso
/// vertices fly off with the arms (and vice versa).
fn segs(rig: &Rig, pairs: &[(usize, usize)]) -> Vec<crate::skinning::BoneSeg> {
    let jw = |i: usize| rig.world[i];
    pairs
        .iter()
        .map(|&(j, child)| crate::skinning::BoneSeg { joint: j as u16, a: jw(j), b: jw(child) })
        .collect()
}

pub fn generate(p: &CharacterParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let h = p.height.clamp(0.8, 3.0);
    let bulk = p.bulk.clamp(0.6, 1.6);
    // tessellation multiplier: 1.0 = default quality, 2.0 = hero close-ups
    let det = p.detail.clamp(0.5, 2.0);
    let seg = |n: f32| ((n * det).round() as u32).max(6);
    let sw = h * 0.135 * bulk;
    let rig = build_rig(h, sw);
    let mut w = wardrobe(&mut r, pal, p.class, p.skin_tone);
    let jw = |i: usize| rig.world[i];
    let dressed = p.outfit.as_deref().is_some_and(|s| s != "plain");
    if let Some(hc) = &p.hair_color {
        if let Ok(c) = crate::palette::hex(hc) {
            w.hair = c;
        }
    }
    let hair_styles = ["short", "ponytail", "bun", "bald"];
    let hair_style = p
        .hair
        .clone()
        .unwrap_or_else(|| hair_styles[r.gen_range(0..hair_styles.len())].to_string());

    let mut body = Mesh::new();
    // smooth-skinned torso core (limbs are bound per-region below)
    let mut core = Mesh::new();

    // body v7: the whole torso+arms+legs as ONE smoothly-blended SDF
    // surface — shoulders, neck, hips and crotch are continuous flesh,
    // not assembled primitives
    let forearm_col = match p.class {
        CharacterClass::Warrior | CharacterClass::Rogue => w.boots * 1.15,
        _ => w.skin,
    };
    body.merge(&organic_body(&rig, h, bulk, sw, &w, forearm_col, det));

    // reference profile (kept for placing surface overlays like buttons)
    let hips_y = jw(HIPS).y;
    let profile: Vec<(f32, f32)> = vec![
        (h * 0.070 * bulk, hips_y - h * 0.005),
        (h * 0.092 * bulk, hips_y + h * 0.035),
        (h * 0.083 * bulk, jw(SPINE).y + h * 0.030), // waist pinch
        (h * 0.094 * bulk, jw(CHEST).y + h * 0.015),
        (h * 0.100 * bulk, jw(CHEST).y + h * 0.062), // chest
        (h * 0.088 * bulk, jw(CHEST).y + h * 0.090), // shoulder slope
        (h * 0.052 * bulk, jw(CHEST).y + h * 0.104), // trapezius
        (h * 0.036 * bulk, jw(NECK).y + h * 0.008),
    ];
    // belt: elliptical ring hugging the lathe waist (hidden under outfits)
    if !dressed {
    let belt_y = jw(HIPS).y + h * 0.030;
    let belt_r = h * 0.090 * bulk;
    let mut belt = lathe(
        &[(belt_r, belt_y - h * 0.022), (belt_r * 1.02, belt_y), (belt_r, belt_y + h * 0.022)],
        14,
        |_, _| w.boots * 0.7,
    );
    for v in belt.positions.iter_mut() {
        v.x *= 1.30;
        v.z *= 0.90;
    }
    belt.recompute_smooth_normals();
    belt.bind_all_to_joint(HIPS as u16);
    body.merge(&belt);
    // buckle: gold frame ring + tongue instead of a floating cube
    let gold = srgb(212, 175, 55);
    let bz = belt_r * 0.92 + h * 0.005;
    let mut buckle = lathe(
        &[(h * 0.010, -h * 0.005), (h * 0.0135, 0.0), (h * 0.010, h * 0.005)],
        10,
        |_, _| gold,
    );
    buckle.transform(Mat4::from_rotation_translation(
        Quat::from_rotation_x(core::f32::consts::FRAC_PI_2),
        Vec3::new(0.0, belt_y, bz),
    ));
    buckle.merge(&cuboid(
        Vec3::new(0.0, belt_y, bz),
        Vec3::new(h * 0.0022, h * 0.010, h * 0.0032),
        gold * 0.9,
    ));
    buckle.bind_all_to_joint(HIPS as u16);
    body.merge(&buckle);
    // leather pouch on the right hip
    let mut pouch = icosphere(h * 0.021, 2, w.boots * 1.3);
    for v in pouch.positions.iter_mut() {
        v.z *= 0.55;
        v.y *= 1.15;
        if v.y > 0.0 {
            v.x *= 0.82;
        }
    }
    pouch.recompute_smooth_normals();
    pouch.transform(Mat4::from_rotation_translation(
        Quat::from_rotation_y(-0.55),
        Vec3::new(-belt_r * 1.12, belt_y - h * 0.024, belt_r * 0.42),
    ));
    pouch.bind_all_to_joint(HIPS as u16);
    body.merge(&pouch);
    // shirt collar ring at the neckline
    let mut collar = lathe(
        &[
            (h * 0.031, jw(NECK).y - h * 0.004),
            (h * 0.037, jw(NECK).y + h * 0.005),
            (h * 0.030, jw(NECK).y + h * 0.013),
        ],
        seg(12.0),
        |_, _| w.shirt * 0.80,
    );
    collar.recompute_smooth_normals();
    collar.bind_all_to_joint(NECK as u16);
    body.merge(&collar);
    // buttons down the chest for cloth shirts
    if matches!(p.class, CharacterClass::Villager | CharacterClass::Mage) {
        let torso_r_at = |y: f32| -> f32 {
            let mut r0 = profile[0].0;
            for pair in profile.windows(2) {
                let (ra, ya) = pair[0];
                let (rb, yb) = pair[1];
                if y >= ya && y <= yb {
                    r0 = ra + (rb - ra) * ((y - ya) / (yb - ya + 1e-6));
                }
            }
            r0
        };
        for k in 0..3 {
            let y = jw(SPINE).y + h * (0.040 + 0.034 * k as f32);
            let z = torso_r_at(y) * 0.88 + h * 0.0045;
            let mut btn = icosphere(h * 0.0055, 1, w.shirt * 0.5);
            btn.translate(Vec3::new(0.0, y, z));
            btn.bind_all_to_joint(CHEST as u16);
            body.merge(&btn);
        }
    }
    }

    // head: shaped skull as its own part with a painted face texture
    // (skin mottling, blush, socket shading, age wrinkles); geometry
    // eyes/brows/mouth stay in the body part so morphs keep working
    let head_r = h * 0.075;
    let head_c = jw(HEAD) + Vec3::new(0.0, head_r * 0.9, 0.0);
    let mut head = head_shape(head_r, Vec3::ONE, if det >= 1.5 { 4 } else { 3 }); // texture carries the skin
    head.translate(head_c);
    // spherical unwrap: u = azimuth (face at 0.5), v = crown(0) -> chin(1)
    head.uvs = head
        .positions
        .iter()
        .map(|p| {
            let d = (*p - head_c).normalize_or(Vec3::Z);
            glam::Vec2::new(
                0.5 + d.x.atan2(d.z) / core::f32::consts::TAU,
                d.y.clamp(-1.0, 1.0).acos() / core::f32::consts::PI,
            )
        })
        .collect();
    head.tangents = head
        .normals
        .iter()
        .map(|n| {
            let t = Vec3::new(-n.z, 0.0, n.x).normalize_or(Vec3::X);
            glam::Vec4::new(t.x, t.y, t.z, 1.0)
        })
        .collect();
    head.bind_all_to_joint(HEAD as u16);
    let head_part = Part {
        mesh: head,
        material: Material {
            roughness: 0.75,
            texture: Some(std::sync::Arc::new(crate::texture::bake_face(
                w.skin,
                p.age.clamp(0.0, 1.0),
                p.seed,
                512,
            ))),
            ..Default::default()
        },
    };

    let mut features = face(head_c, head_r, &w, p.expressions);
    features.bind_all_to_joint(HEAD as u16);
    body.merge(&features);

    let hair_c = head_c + Vec3::new(0.0, head_r * 0.12, -head_r * 0.04);
    let mut hair = match hair_style.as_str() {
        "long" | "topknot" => {
            // scalp cap + flowing ribbon cards
            let mut m = hair_mesh("short", hair_c, head_r, w.hair);
            m.merge(&hair_cards(&hair_style, hair_c, head_r, w.hair, p.seed));
            m
        }
        other => hair_mesh(other, hair_c, head_r, w.hair),
    };
    if hair.vertex_count() > 0 {
        hair.bind_all_to_joint(HEAD as u16);
        body.merge(&hair);
    }
    if let Some(beard) = &p.beard {
        let mut b = beard_cards(beard, head_c, head_r, w.hair, p.seed);
        if b.vertex_count() > 0 {
            b.bind_all_to_joint(HEAD as u16);
            body.merge(&b);
        }
    }

    // arms: the surface comes from the fused body; only cloth overlays,
    // hands and gear remain as separate pieces
    let arm_r0 = h * 0.036 * bulk;
    for (aj, fj, hj) in [(ARM_L, FOREARM_L, HAND_L), (ARM_R, FOREARM_R, HAND_R)] {
        let (a, f, hd) = (jw(aj), jw(fj), jw(hj));
        let mut arm = Mesh::new();
        let _ = hd;
        // sleeve hem so the shirt/forearm boundary reads as cloth
        let mut hem = lathe(
            &[
                (arm_r0 * 0.79, -arm_r0 * 0.10),
                (arm_r0 * 0.85, 0.0),
                (arm_r0 * 0.79, arm_r0 * 0.14),
            ],
            10,
            |_, _| w.shirt * 0.92,
        );
        hem.translate(f + (a - f) * 0.10);
        arm.merge(&hem);
        // leather bracer straps on martial classes
        if matches!(p.class, CharacterClass::Warrior | CharacterClass::Rogue) {
            for t in [0.42f32, 0.78] {
                let mut strap = lathe(
                    &[
                        (arm_r0 * 0.78, -h * 0.0032),
                        (arm_r0 * 0.83, 0.0),
                        (arm_r0 * 0.78, h * 0.0032),
                    ],
                    10,
                    |_, _| forearm_col * 0.65,
                );
                strap.translate(f + (hd - f) * t);
                arm.merge(&strap);
            }
        }
        crate::skinning::smooth_bind(&mut arm, &segs(&rig, &[(aj, fj), (fj, hj)]), 4.0);
        body.merge(&arm);
        let mut hand = mitten(
            jw(hj) - Vec3::Y * arm_r0 * 0.25,
            arm_r0 * 0.95,
            jw(aj).x.signum(),
            w.skin,
            if det >= 1.4 { 3 } else { 2 },
        );
        hand.bind_all_to_joint(hj as u16);
        body.merge(&hand);
        // pauldrons: warriors only — the smooth torso needs no sleeve puffs
        if p.class == CharacterClass::Warrior && !dressed {
            // pauldron: dome capping the shoulder ball, wrapping down over
            // the upper arm instead of hovering above it
            let mut pad = icosphere(arm_r0 * 1.24, 2, w.shirt * 0.7);
            for v in pad.positions.iter_mut() {
                if v.y < 0.0 {
                    v.y *= 0.6;
                }
                v.y *= 0.78; // flat armor cap, not a balloon
            }
            pad.recompute_smooth_normals();
            let side = jw(aj).x.signum();
            pad.translate(jw(aj) + Vec3::new(side * arm_r0 * 0.05, -arm_r0 * 0.02, 0.0));
            pad.bind_all_to_joint(aj as u16);
            body.merge(&pad);
        }
    }

    // feet + a garter strap covering the pants/boot-shaft boundary
    let leg_r = h * 0.052 * bulk;
    for (sj, fj) in [(SHIN_L, FOOT_L), (SHIN_R, FOOT_R)] {
        let mut foot = boot(jw(fj), leg_r * 0.52, h, w.boots);
        foot.bind_all_to_joint(fj as u16);
        body.merge(&foot);
        let mut strap = lathe(
            &[
                (leg_r * 0.72, -h * 0.010),
                (leg_r * 0.78, 0.0),
                (leg_r * 0.72, h * 0.010),
            ],
            12,
            |_, _| w.boots * 0.72,
        );
        strap.recompute_smooth_normals();
        strap.translate(jw(sj) + (jw(fj) - jw(sj)) * 0.42);
        crate::skinning::smooth_bind(&mut strap, &segs(&rig, &[(sj, fj)]), 4.0);
        body.merge(&strap);
    }

    // class gear
    match p.class {
        CharacterClass::Mage => {
            // wizard hat
            let mut hat = to_flat_shaded(&lathe(
                &[
                    (head_r * 1.35, 0.0),
                    (head_r * 1.30, head_r * 0.14),
                    (head_r * 0.72, head_r * 0.35),
                    (head_r * 0.30, head_r * 1.15),
                    (0.0, head_r * 1.7),
                ],
                9,
                |ri, _| if ri == 0 { w.accent } else { w.shirt * 0.85 },
            ));
            hat.transform(
                Mat4::from_translation(jw(HEAD) + Vec3::new(0.0, head_r * 1.7, -head_r * 0.1))
                    * Mat4::from_rotation_x(-0.12),
            );
            hat.bind_all_to_joint(HEAD as u16);
            body.merge(&hat);
            // gold band where the cone meets the brim
            let mut band = lathe(
                &[
                    (head_r * 0.72, head_r * 0.16),
                    (head_r * 0.70, head_r * 0.28),
                ],
                12,
                |_, _| srgb(212, 175, 55),
            );
            band.recompute_smooth_normals();
            band.transform(
                Mat4::from_translation(jw(HEAD) + Vec3::new(0.0, head_r * 1.7, -head_r * 0.1))
                    * Mat4::from_rotation_x(-0.12),
            );
            band.bind_all_to_joint(HEAD as u16);
            body.merge(&band);
            // flowing robe skirt from the hips (superseded by outfits)
            if !dressed {
            let mut robe = lathe(
                &[
                    (h * 0.145 * bulk, -h * 0.30),
                    (h * 0.125 * bulk, -h * 0.18),
                    (h * 0.10 * bulk, -h * 0.05),
                    (h * 0.095 * bulk, h * 0.03),
                ],
                seg(14.0),
                |ri, _| if ri == 0 { w.accent * 0.7 } else { w.shirt * 0.9 },
            );
            robe.recompute_smooth_normals();
            robe = crate::subdiv::subdivide(&robe, false);
            robe.translate(jw(HIPS));
            robe.bind_all_to_joint(HIPS as u16);
            body.merge(&robe);
            }
        }
        CharacterClass::Warrior if !dressed => {
            // wrap-around cuirass following the torso's rounded section
            let mut plate = lathe(
                &[
                    (h * 0.096 * bulk, jw(SPINE).y + h * 0.02),
                    (h * 0.104 * bulk, jw(CHEST).y + h * 0.01),
                    (h * 0.108 * bulk, jw(CHEST).y + h * 0.062),
                    (h * 0.085 * bulk, jw(CHEST).y + h * 0.095),
                ],
                14,
                |_, _| w.shirt * 0.65,
            );
            for v in plate.positions.iter_mut() {
                v.x *= 1.28;
                v.z *= 0.88;
            }
            plate.recompute_smooth_normals();
            plate.bind_all_to_joint(CHEST as u16);
            body.merge(&plate);
            // accent collar: an elliptical ring hugging the cuirass top
            let mut trim = lathe(
                &[
                    (h * 0.082 * bulk, jw(CHEST).y + h * 0.086),
                    (h * 0.088 * bulk, jw(CHEST).y + h * 0.098),
                    (h * 0.070 * bulk, jw(CHEST).y + h * 0.110),
                ],
                14,
                |_, _| w.accent * 0.85,
            );
            for v in trim.positions.iter_mut() {
                v.x *= 1.28;
                v.z *= 0.88;
            }
            trim.recompute_smooth_normals();
            trim.bind_all_to_joint(CHEST as u16);
            body.merge(&trim);
            // plate rim band + front rivets
            let mut rim = lathe(
                &[
                    (h * 0.0965 * bulk, jw(SPINE).y + h * 0.016),
                    (h * 0.1015 * bulk, jw(SPINE).y + h * 0.026),
                    (h * 0.0965 * bulk, jw(SPINE).y + h * 0.036),
                ],
                14,
                |_, _| w.shirt * 0.45,
            );
            for v in rim.positions.iter_mut() {
                v.x *= 1.28;
                v.z *= 0.88;
            }
            rim.recompute_smooth_normals();
            rim.bind_all_to_joint(CHEST as u16);
            body.merge(&rim);
            let steel = srgb(222, 224, 232);
            for k in 0..5 {
                let ang = (k as f32 - 2.0) * 0.42;
                let rr = h * 0.104 * bulk;
                let mut rivet = icosphere(h * 0.0048, 1, steel);
                rivet.translate(Vec3::new(
                    ang.sin() * rr * 1.28,
                    jw(CHEST).y + h * 0.052,
                    ang.cos() * rr * 0.88 + h * 0.002,
                ));
                rivet.bind_all_to_joint(CHEST as u16);
                body.merge(&rivet);
            }
        }
        CharacterClass::Rogue => {
            // hood: smooth draped cloth with a slight peak and open face
            let mut hood = icosphere(head_r * 1.18, 3, w.shirt * 0.8);
            for v in hood.positions.iter_mut() {
                if v.z > head_r * 0.4 {
                    v.z = head_r * 0.4;
                }
                // drape: pull the back down and peak the crown slightly
                if v.z < 0.0 && v.y < 0.0 {
                    v.y *= 1.22;
                }
                if v.y > head_r * 0.6 {
                    v.z *= 0.92;
                }
            }
            hood.recompute_smooth_normals();
            hood.translate(jw(HEAD) + Vec3::new(0.0, head_r * 1.05, -head_r * 0.15));
            hood.bind_all_to_joint(HEAD as u16);
            body.merge(&hood);
            // sheathed dagger on the left hip
            let belt_y = jw(HIPS).y + h * 0.030;
            let hip_x = h * 0.126 * bulk;
            let mut dagger = cuboid(
                Vec3::new(0.0, -h * 0.026, 0.0),
                Vec3::new(h * 0.0065, h * 0.026, h * 0.009),
                w.boots * 0.7,
            );
            dagger.merge(&cuboid(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(h * 0.014, h * 0.0035, h * 0.011),
                srgb(150, 150, 160),
            ));
            dagger.merge(&cuboid(
                Vec3::new(0.0, h * 0.012, 0.0),
                Vec3::new(h * 0.004, h * 0.009, h * 0.006),
                w.boots * 1.2,
            ));
            let mut pommel = icosphere(h * 0.006, 1, srgb(212, 175, 55));
            pommel.translate(Vec3::new(0.0, h * 0.024, 0.0));
            dagger.merge(&pommel);
            dagger.transform(Mat4::from_rotation_translation(
                Quat::from_rotation_z(-0.45),
                Vec3::new(hip_x, belt_y - h * 0.022, h * 0.030),
            ));
            dagger.bind_all_to_joint(HIPS as u16);
            body.merge(&dagger);
        }
        _ => {}
    }

    // smooth-skin the torso core and fold it into the body
    crate::skinning::smooth_bind(
        &mut core,
        &segs(&rig, &[(SPINE, CHEST), (CHEST, NECK), (NECK, HEAD)]),
        4.0,
    );
    body.merge(&core);

    // accessories (rigid-bound props)
    for acc in &p.accessories {
        if let Some(a) = accessory(acc, &rig, h, &w, pal) {
            body.merge(&a);
        }
    }

    // baked ambient occlusion: crevices read in any light
    crate::mesh::bake_ao(&mut body, 0.55);

    let mut animations = Vec::new();
    if p.animate {
        animations.push(idle_clip(&rig, h));
        animations.push(walk_clip(&rig, h));
        animations.push(run_clip(&rig, h));
        animations.push(attack_clip(&rig, h));
        animations.push(sit_clip(&rig, h));
        animations.push(wave_clip(&rig, h));
        animations.push(death_clip(&rig, h));
        animations.push(dance_clip(&rig, h));
    }

    body.validate().expect("character mesh invalid");

    let mut parts = vec![
        Part {
            mesh: body,
            material: Material { roughness: 0.85, ..Default::default() },
        },
        head_part,
    ];
    if dressed {
        let ctx = GarmentCtx {
            rig: &rig,
            h,
            bulk,
            w: &w,
            orn: p.ornamentation.clamp(0.0, 1.0),
            motif: p.trim_motif.clone().unwrap_or_else(|| "meander".into()),
            seed: p.seed,
        };
        parts.extend(outfit_parts(&ctx, p.outfit.as_deref().unwrap_or("robe")));
    }

    Asset {
        name: "character".into(),
        parts,
        skeleton: Some(rig.skeleton),
        animations,
        physics: Some(Physics {
            collider: Collider::Capsule { radius: h * 0.16, height: h },
            mass: 70.0 * h / 1.7,
            friction: 0.4,
            restitution: 0.0,
        }),
        lods: Vec::new(),
        instanced: Vec::new(),
    }
}

// ---------- outfit system: lofted, painted garment stacks ----------

struct GarmentCtx<'a> {
    rig: &'a Rig,
    h: f32,
    bulk: f32,
    w: &'a Wardrobe,
    orn: f32,
    motif: String,
    seed: u64,
}

fn garment_tex(
    base: Vec3,
    seed: u64,
    folds: f32,
    layers: Vec<crate::texture::PaintLayer>,
) -> Option<std::sync::Arc<crate::texture::BakedTexture>> {
    use crate::texture::{PaintLayer, TextureSpec, bake};
    let mut paint = vec![PaintLayer::Folds { strength: folds, count: 11 }];
    paint.extend(layers);
    let spec = TextureSpec {
        pattern: "none".into(),
        base: Some(crate::palette::to_hex(base)),
        paint,
        scale: 1.0,
        seed,
        normal_strength: 1.0,
        resolution: 512,
        colors: Vec::new(),
    };
    bake(&spec).ok().map(std::sync::Arc::new)
}

fn hem_band(motif: &str, trim: Vec3, gold: Vec3, orn: f32) -> Vec<crate::texture::PaintLayer> {
    use crate::texture::PaintLayer;
    let mut v = vec![PaintLayer::Band {
        v: 0.0,
        height: 0.07 + orn * 0.04,
        color: crate::palette::to_hex(trim),
        motif: (orn > 0.25).then(|| motif.to_string()),
        motif_color: Some(crate::palette::to_hex(gold)),
        motif_scale: 1.0,
    }];
    if orn > 0.4 {
        v.push(PaintLayer::Band {
            v: 0.08 + orn * 0.04,
            height: 0.015,
            color: crate::palette::to_hex(gold),
        motif: None,
            motif_color: None,
            motif_scale: 1.0,
        });
    }
    v
}

/// Build a skinned, painted garment part.
fn garment(
    ctx: &GarmentCtx,
    stations: &[crate::mesh::LoftStation],
    arc: f32,
    segs: &[(usize, usize)],
    base: Vec3,
    folds: f32,
    layers: Vec<crate::texture::PaintLayer>,
) -> Part {
    let mut m = crate::mesh::loft(stations, 28, arc, 180.0, |_| Vec3::ONE);
    crate::skinning::smooth_bind(&mut m, &segs_of(ctx.rig, segs), 2.2);
    Part {
        mesh: m,
        material: Material {
            roughness: 0.85,
            double_sided: true,
            texture: garment_tex(base, ctx.seed, folds, layers),
            ..Default::default()
        },
    }
}

fn segs_of(rig: &Rig, pairs: &[(usize, usize)]) -> Vec<crate::skinning::BoneSeg> {
    segs(rig, pairs)
}

fn outfit_parts(ctx: &GarmentCtx, style: &str) -> Vec<Part> {
    use crate::mesh::LoftStation;
    use crate::texture::PaintLayer;
    let (h, bulk) = (ctx.h, ctx.bulk);
    let w = ctx.w;
    let jw = |i: usize| ctx.rig.world[i];
    let gold = srgb(216, 176, 88);
    let trim = w.accent * 0.75;
    let ground = lerp(w.shirt, srgb(232, 221, 196), 0.55); // cream-shifted
    let rich = lerp(w.shirt, w.accent, 0.35) * 0.9;
    let st = |y: f32, rx: f32, rz: f32| LoftStation {
        center: Vec3::new(0.0, y, 0.0),
        rx: rx * bulk,
        rz: rz * bulk,
    };
    let body_segs: &[(usize, usize)] = &[
        (HIPS, SPINE),
        (SPINE, CHEST),
        (CHEST, NECK),
        (THIGH_L, SHIN_L),
        (THIGH_R, SHIN_R),
        (SHIN_L, FOOT_L),
        (SHIN_R, FOOT_R),
    ];
    let mut parts = Vec::new();

    // under-robe: closed, hem -> chest; radii must clear the elliptical
    // torso lathe (x = r * 1.28) or it pokes through
    let hem_y = if style == "tunic" { h * 0.26 } else { h * 0.055 };
    let hem_rx = if style == "tunic" { h * 0.15 } else { h * 0.165 };
    let under = [
        st(hem_y, hem_rx, hem_rx * 0.85),
        st(h * 0.33, h * 0.138, h * 0.115),
        st(jw(HIPS).y, h * 0.128, h * 0.102),
        st(jw(SPINE).y + h * 0.02, h * 0.120, h * 0.096),
        st(jw(CHEST).y + h * 0.05, h * 0.138, h * 0.100),
    ];
    parts.push(garment(
        ctx,
        &under,
        360.0,
        body_segs,
        ground,
        0.7,
        hem_band(&ctx.motif, trim, gold, ctx.orn),
    ));

    if style == "robe" || style == "coat" {
        // open outer coat: mid-shin -> neck, front opening
        let coat = [
            st(h * 0.16, h * 0.195, h * 0.16),
            st(h * 0.30, h * 0.158, h * 0.13),
            st(jw(HIPS).y, h * 0.140, h * 0.112),
            st(jw(SPINE).y + h * 0.02, h * 0.132, h * 0.106),
            st(jw(CHEST).y + h * 0.04, h * 0.150, h * 0.110),
            st(jw(NECK).y + h * 0.005, h * 0.090, h * 0.074),
        ];
        let mut layers = hem_band(&ctx.motif, trim, gold, ctx.orn);
        layers.push(PaintLayer::UBand {
            u: 0.025,
            width: 0.05,
            color: crate::palette::to_hex(trim),
        });
        layers.push(PaintLayer::UBand {
            u: 0.975,
            width: 0.05,
            color: crate::palette::to_hex(trim),
        });
        if ctx.orn > 0.55 {
            // small, low-contrast stamps read as brocade, not wallpaper
            layers.push(PaintLayer::MotifGrid {
                motif: ctx.motif.clone(),
                color: crate::palette::to_hex(lerp(rich, trim, 0.45)),
                scale: 3.2,
                v_min: 0.34,
                v_max: 0.68,
            });
        }
        let coat_segs: &[(usize, usize)] = &[
            (HIPS, SPINE),
            (SPINE, CHEST),
            (CHEST, NECK),
            (THIGH_L, SHIN_L),
            (THIGH_R, SHIN_R),
        ];
        parts.push(garment(ctx, &coat, 295.0, coat_segs, rich, 1.0, layers));

        // wide hanging sleeves over the arms
        for (aj, fj, hj) in [(ARM_L, FOREARM_L, HAND_L), (ARM_R, FOREARM_R, HAND_R)] {
            let ax = jw(aj).x;
            let wrist_y = jw(hj).y + h * 0.01;
            // stations track the actual joint positions — the A-pose rig
            // angles the arm outward, so a straight x=ax axis leaves the
            // elbow/deltoid poking through the cloth
            let stations = [
                LoftStation {
                    center: Vec3::new(jw(hj).x, wrist_y, jw(hj).z),
                    rx: h * 0.062 * bulk,
                    rz: h * 0.058 * bulk,
                },
                LoftStation {
                    center: Vec3::new(jw(fj).x, jw(fj).y, jw(fj).z),
                    rx: h * 0.052 * bulk,
                    rz: h * 0.050 * bulk,
                },
                LoftStation {
                    // swallow the shoulder ball completely
                    center: Vec3::new(ax, jw(aj).y + h * 0.048, 0.0),
                    rx: h * 0.062 * bulk,
                    rz: h * 0.060 * bulk,
                },
                // converge to close the shoulder opening (kept low so the
                // tip doesn't poke above the shoulder line from behind)
                LoftStation {
                    center: Vec3::new(ax * 0.94, jw(aj).y + h * 0.058, 0.0),
                    rx: h * 0.012,
                    rz: h * 0.012,
                },
            ];
            let mut m = crate::mesh::loft(&stations, 16, 360.0, 0.0, |_| Vec3::ONE);
            crate::skinning::smooth_bind(
                &mut m,
                &segs_of(ctx.rig, &[(aj, fj), (fj, hj)]),
                2.5,
            );
            parts.push(Part {
                mesh: m,
                material: Material {
                    roughness: 0.85,
                    double_sided: true,
                    texture: garment_tex(
                        rich,
                        ctx.seed ^ aj as u64,
                        0.6,
                        hem_band(&ctx.motif, trim, gold, ctx.orn * 0.8),
                    ),
                    ..Default::default()
                },
            });
        }

        // sash at the waist + hanging front tail
        let sash_y = jw(HIPS).y + h * 0.055;
        let sash = [
            st(sash_y - h * 0.028, h * 0.146, h * 0.118),
            st(sash_y + h * 0.028, h * 0.140, h * 0.114),
        ];
        parts.push(garment(
            ctx,
            &sash,
            360.0,
            &[(HIPS, SPINE)],
            w.accent * 0.8,
            0.3,
            vec![PaintLayer::Stripes {
                count: 3,
                width: 0.18,
                color: crate::palette::to_hex(gold),
                axis: Some("v".into()),
            }],
        ));
        let tail = [
            st(h * 0.24, h * 0.16, h * 0.138),
            st(sash_y - h * 0.02, h * 0.144, h * 0.117),
        ];
        parts.push(garment(
            ctx,
            &tail,
            42.0,
            &[(HIPS, SPINE), (THIGH_L, SHIN_L), (THIGH_R, SHIN_R)],
            w.accent * 0.8,
            0.4,
            hem_band(&ctx.motif, trim * 0.9, gold, ctx.orn),
        ));

        // mantle collar draped over the shoulders
        let mantle = [
            st(jw(CHEST).y + h * 0.035, h * 0.160, h * 0.122),
            st(jw(NECK).y + h * 0.025, h * 0.054, h * 0.050),
        ];
        parts.push(garment(
            ctx,
            &mantle,
            360.0,
            &[(CHEST, NECK), (NECK, HEAD)],
            ground * 0.96,
            0.35,
            hem_band(&ctx.motif, trim, gold, ctx.orn),
        ));
    } else if style == "tunic" {
        // belted knee tunic over the under-robe... under-robe IS the tunic:
        // shorten look with a belt sash only
        let sash_y = jw(HIPS).y + h * 0.045;
        let sash = [
            st(sash_y - h * 0.022, h * 0.132, h * 0.106),
            st(sash_y + h * 0.022, h * 0.128, h * 0.104),
        ];
        parts.push(garment(
            ctx,
            &sash,
            360.0,
            &[(HIPS, SPINE)],
            w.boots * 0.8,
            0.2,
            Vec::new(),
        ));
    }

    parts
}

/// Accessory props: bead necklace + gem pendant, belt knot with hanging
/// ribbons, or a staff in the right hand.
fn accessory(name: &str, rig: &Rig, h: f32, w: &Wardrobe, pal: &Palette) -> Option<Mesh> {
    let jw = |i: usize| rig.world[i];
    let gold = srgb(216, 176, 88);
    let mut m = Mesh::new();
    match name {
        "necklace" => {
            // beads draped across the chest in a shallow V
            let c = jw(CHEST) + Vec3::new(0.0, h * 0.055, h * 0.125);
            let beads = 15;
            for i in 0..beads {
                let t = i as f32 / (beads - 1) as f32; // 0..1 left→right
                let x = (t - 0.5) * h * 0.11;
                let sag = (1.0 - (2.0 * t - 1.0).powi(2)) * h * 0.05;
                let mut bead = icosphere(h * 0.0075, 1, gold);
                bead.translate(c + Vec3::new(x, -sag, -x.abs() * 0.25));
                m.merge(&bead);
            }
            // pendant gem at the lowest point
            let mut gem = icosphere(h * 0.016, 1, pal.accent * 1.3);
            for v in gem.positions.iter_mut() {
                v.y *= 1.4;
            }
            gem.recompute_smooth_normals();
            gem.translate(c + Vec3::new(0.0, -h * 0.065, 0.0));
            m.merge(&gem);
            m.bind_all_to_joint(CHEST as u16);
        }
        "belt_knot" => {
            let at = jw(HIPS) + Vec3::new(h * 0.02, h * 0.05, h * 0.115);
            let knot = crate::subdiv::subdivide_n(
                &cuboid(at, Vec3::new(h * 0.022, h * 0.018, h * 0.014), gold * 0.9),
                1,
                true,
            );
            m.merge(&knot);
            // two hanging ribbon tails
            for sx in [-1.0f32, 1.0] {
                let guide: Vec<Vec3> = (0..5)
                    .map(|i| {
                        let t = i as f32 / 4.0;
                        at + Vec3::new(
                            sx * h * 0.012 + sx * t * h * 0.008,
                            -t * h * 0.16,
                            -t * t * h * 0.02,
                        )
                    })
                    .collect();
                m.merge(&ribbon(&guide, h * 0.022, h * 0.014, w.accent * 0.75, at));
            }
            m.bind_all_to_joint(HIPS as u16);
        }
        "staff" => {
            let hand = jw(HAND_R);
            let base = Vec3::new(hand.x - h * 0.01, 0.0, hand.z + h * 0.01);
            let shaft = tube(
                &[
                    (base, h * 0.014),
                    (base + Vec3::Y * h * 0.62, h * 0.012),
                    (base + Vec3::Y * h * 1.02, h * 0.010),
                ],
                8,
                |_| pal.trunk * 0.85,
            );
            m.merge(&crate::subdiv::subdivide(&shaft, false));
            let mut orb = icosphere(h * 0.032, 2, pal.accent * 1.4);
            orb.translate(base + Vec3::Y * h * 1.06);
            m.merge(&orb);
            m.bind_all_to_joint(HAND_R as u16);
        }
        _ => return None,
    }
    Some(m)
}

fn keys(n: usize, dur: f32) -> Vec<f32> {
    (0..=n).map(|i| i as f32 / n as f32 * dur).collect()
}

fn rot_channel(joint: usize, times: &[f32], f: impl Fn(f32) -> Quat) -> Channel {
    let dur = *times.last().unwrap();
    Channel {
        joint,
        times: times.to_vec(),
        data: ChannelData::Rotation(times.iter().map(|&t| f(t / dur)).collect()),
    }
}

fn idle_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 2.4;
    let t = keys(16, dur);
    let tau = core::f32::consts::TAU;
    let mut channels = vec![
        rot_channel(CHEST, &t, move |p| Quat::from_rotation_x(0.035 * (p * tau).sin())),
        rot_channel(HEAD, &t, move |p| Quat::from_rotation_y(0.06 * (p * tau + 1.2).sin())),
        rot_channel(ARM_L, &t, move |p| Quat::from_rotation_z(0.05 + 0.03 * (p * tau).sin())),
        rot_channel(ARM_R, &t, move |p| Quat::from_rotation_z(-0.05 - 0.03 * (p * tau).sin())),
    ];
    // breathing bob on hips
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    channels.push(Channel {
        joint: HIPS,
        times: t.clone(),
        data: ChannelData::Translation(
            t.iter()
                .map(|&tt| hips_bind + Vec3::Y * h * 0.004 * (tt / dur * tau).sin())
                .collect(),
        ),
    });
    AnimationClip { name: "idle".into(), channels }
}

fn trans_channel(joint: usize, times: &[f32], bind: Vec3, f: impl Fn(f32) -> Vec3) -> Channel {
    let dur = *times.last().unwrap();
    Channel {
        joint,
        times: times.to_vec(),
        data: ChannelData::Translation(times.iter().map(|&t| bind + f(t / dur)).collect()),
    }
}

/// Smooth one-shot envelope: eases 0→1 over [a, b] and holds.
fn env(p: f32, a: f32, b: f32) -> f32 {
    let t = ((p - a) / (b - a).max(1e-4)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn run_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 0.65;
    let t = keys(16, dur);
    let tau = core::f32::consts::TAU;
    let swing = 1.05;
    let mut channels = vec![
        rot_channel(THIGH_L, &t, move |p| Quat::from_rotation_x(swing * (p * tau).sin() + 0.15)),
        rot_channel(THIGH_R, &t, move |p| Quat::from_rotation_x(-swing * (p * tau).sin() + 0.15)),
        rot_channel(SHIN_L, &t, move |p| {
            Quat::from_rotation_x(-(1.6 * (p * tau + 0.7).sin()).max(0.0) - 0.15)
        }),
        rot_channel(SHIN_R, &t, move |p| {
            Quat::from_rotation_x(-(1.6 * (p * tau + 0.7 + tau / 2.0).sin()).max(0.0) - 0.15)
        }),
        rot_channel(FOOT_L, &t, move |p| Quat::from_rotation_x(0.35 * (p * tau + 1.2).sin())),
        rot_channel(FOOT_R, &t, move |p| Quat::from_rotation_x(-0.35 * (p * tau + 1.2).sin())),
        // arms pump bent at ~90°
        rot_channel(ARM_L, &t, move |p| Quat::from_rotation_x(-0.9 * (p * tau).sin())),
        rot_channel(ARM_R, &t, move |p| Quat::from_rotation_x(0.9 * (p * tau).sin())),
        rot_channel(FOREARM_L, &t, move |_| Quat::from_rotation_x(-1.5)),
        rot_channel(FOREARM_R, &t, move |_| Quat::from_rotation_x(-1.5)),
        // forward lean + counter-rotation
        rot_channel(CHEST, &t, move |p| {
            Quat::from_rotation_x(0.22) * Quat::from_rotation_y(0.14 * (p * tau).sin())
        }),
        rot_channel(HIPS, &t, move |p| Quat::from_rotation_y(-0.10 * (p * tau).sin())),
    ];
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    channels.push(trans_channel(HIPS, &t, hips_bind, move |p| {
        Vec3::Y * h * 0.035 * ((p * tau * 2.0).sin().abs() + 0.2)
    }));
    AnimationClip { name: "run".into(), channels }
}

fn attack_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 0.9;
    let t = keys(20, dur);
    // wind-up (0..0.35), strike (0.35..0.55), recover (0.55..1)
    let strike = move |p: f32| env(p, 0.35, 0.5) - env(p, 0.6, 1.0);
    let windup = move |p: f32| env(p, 0.0, 0.3) - env(p, 0.35, 0.55);
    let mut channels = vec![
        // right arm: raise overhead then chop down-forward
        rot_channel(ARM_R, &t, move |p| {
            let lift = windup(p) * 2.4 + strike(p) * -0.4;
            Quat::from_rotation_x(-lift.max(-0.4)) * Quat::from_rotation_z(-0.25 * windup(p))
        }),
        rot_channel(FOREARM_R, &t, move |p| {
            Quat::from_rotation_x(-0.6 * windup(p) - 0.25 + 0.55 * strike(p))
        }),
        // left arm guards across the body
        rot_channel(ARM_L, &t, move |p| {
            Quat::from_rotation_x(-0.5 * env(p, 0.0, 0.3) + 0.3 * strike(p))
                * Quat::from_rotation_z(0.35)
        }),
        rot_channel(FOREARM_L, &t, move |_| Quat::from_rotation_x(-0.9)),
        // torso winds and whips
        rot_channel(CHEST, &t, move |p| {
            Quat::from_rotation_y(-0.5 * windup(p) + 0.55 * strike(p))
                * Quat::from_rotation_x(0.25 * strike(p))
        }),
        rot_channel(HIPS, &t, move |p| Quat::from_rotation_y(-0.2 * windup(p) + 0.3 * strike(p))),
        rot_channel(HEAD, &t, move |p| Quat::from_rotation_x(-0.15 * windup(p))),
        // lunge stance
        rot_channel(THIGH_L, &t, move |p| Quat::from_rotation_x(0.5 * strike(p))),
        rot_channel(THIGH_R, &t, move |p| Quat::from_rotation_x(-0.35 * strike(p))),
        rot_channel(SHIN_L, &t, move |p| Quat::from_rotation_x(-0.55 * strike(p))),
    ];
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    channels.push(trans_channel(HIPS, &t, hips_bind, move |p| {
        Vec3::new(0.0, -h * 0.05 * strike(p), h * 0.06 * strike(p))
    }));
    AnimationClip { name: "attack".into(), channels }
}

fn sit_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 1.2;
    let t = keys(16, dur);
    let s = move |p: f32| env(p, 0.1, 0.7);
    let mut channels = vec![
        rot_channel(THIGH_L, &t, move |p| Quat::from_rotation_x(1.5 * s(p))),
        rot_channel(THIGH_R, &t, move |p| Quat::from_rotation_x(1.5 * s(p))),
        rot_channel(SHIN_L, &t, move |p| Quat::from_rotation_x(-1.5 * s(p))),
        rot_channel(SHIN_R, &t, move |p| Quat::from_rotation_x(-1.5 * s(p))),
        rot_channel(CHEST, &t, move |p| Quat::from_rotation_x(0.12 * s(p))),
        // hands rest toward knees
        rot_channel(ARM_L, &t, move |p| Quat::from_rotation_x(-0.55 * s(p))),
        rot_channel(ARM_R, &t, move |p| Quat::from_rotation_x(-0.55 * s(p))),
        rot_channel(FOREARM_L, &t, move |p| Quat::from_rotation_x(-0.35 * s(p))),
        rot_channel(FOREARM_R, &t, move |p| Quat::from_rotation_x(-0.35 * s(p))),
    ];
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    // drop by the thigh length onto an imaginary stool
    channels.push(trans_channel(HIPS, &t, hips_bind, move |p| {
        Vec3::new(0.0, -h * 0.2 * s(p), -h * 0.02 * s(p))
    }));
    AnimationClip { name: "sit".into(), channels }
}

fn wave_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 1.6;
    let t = keys(20, dur);
    let tau = core::f32::consts::TAU;
    let up = move |p: f32| env(p, 0.0, 0.25) - env(p, 0.85, 1.0);
    let mut channels = vec![
        // raise the right arm out and up, forearm waves
        rot_channel(ARM_R, &t, move |p| Quat::from_rotation_z(-2.4 * up(p))),
        rot_channel(FOREARM_R, &t, move |p| {
            Quat::from_rotation_z((-0.5 + 0.45 * (p * tau * 2.0).sin()) * up(p))
        }),
        rot_channel(HAND_R, &t, move |p| {
            Quat::from_rotation_z(0.3 * (p * tau * 2.0).sin() * up(p))
        }),
        rot_channel(HEAD, &t, move |p| Quat::from_rotation_z(0.12 * up(p))),
        rot_channel(CHEST, &t, move |p| Quat::from_rotation_z(0.05 * up(p))),
        rot_channel(ARM_L, &t, move |p| Quat::from_rotation_z(0.06 + 0.02 * (p * tau).sin())),
    ];
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    channels.push(trans_channel(HIPS, &t, hips_bind, move |p| {
        Vec3::Y * h * 0.004 * (p * tau).sin()
    }));
    AnimationClip { name: "wave".into(), channels }
}

fn death_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 1.3;
    let t = keys(20, dur);
    // stagger (0..0.3), buckle (0.3..0.6), collapse backward (0.5..0.95)
    let stagger = move |p: f32| env(p, 0.0, 0.25);
    let buckle = move |p: f32| env(p, 0.3, 0.6);
    let fall = move |p: f32| env(p, 0.5, 0.95);
    let mut channels = vec![
        rot_channel(CHEST, &t, move |p| {
            Quat::from_rotation_x(-0.3 * stagger(p) + 0.15 * fall(p))
                * Quat::from_rotation_y(0.15 * stagger(p))
        }),
        rot_channel(HEAD, &t, move |p| Quat::from_rotation_x(-0.35 * stagger(p) + 0.9 * fall(p))),
        rot_channel(ARM_L, &t, move |p| Quat::from_rotation_z(0.5 * stagger(p) * (1.0 - fall(p)))),
        rot_channel(ARM_R, &t, move |p| {
            Quat::from_rotation_z(-0.6 * stagger(p) * (1.0 - fall(p)))
        }),
        rot_channel(THIGH_L, &t, move |p| Quat::from_rotation_x(1.1 * buckle(p) - 1.0 * fall(p))),
        rot_channel(THIGH_R, &t, move |p| Quat::from_rotation_x(0.9 * buckle(p) - 0.8 * fall(p))),
        rot_channel(SHIN_L, &t, move |p| Quat::from_rotation_x(-1.3 * buckle(p) + 0.9 * fall(p))),
        rot_channel(SHIN_R, &t, move |p| Quat::from_rotation_x(-1.1 * buckle(p) + 0.8 * fall(p))),
    ];
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    channels.push(Channel {
        joint: HIPS,
        times: t.clone(),
        data: ChannelData::Rotation(
            t.iter()
                .map(|&tt| {
                    let p = tt / dur;
                    Quat::from_rotation_x(-1.45 * fall(p))
                })
                .collect(),
        ),
    });
    channels.push(trans_channel(HIPS, &t, hips_bind, move |p| {
        Vec3::new(
            0.0,
            -h * 0.09 * buckle(p) - (hips_bind.y - h * 0.09) * 0.82 * fall(p),
            -h * 0.10 * fall(p),
        )
    }));
    AnimationClip { name: "death".into(), channels }
}

fn dance_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 1.6;
    let t = keys(24, dur);
    let tau = core::f32::consts::TAU;
    let mut channels = vec![
        // alternating arm raises
        rot_channel(ARM_L, &t, move |p| {
            Quat::from_rotation_z(1.4 + 0.9 * (p * tau * 2.0).sin())
        }),
        rot_channel(ARM_R, &t, move |p| {
            Quat::from_rotation_z(-1.4 + 0.9 * (p * tau * 2.0).sin())
        }),
        rot_channel(FOREARM_L, &t, move |p| {
            Quat::from_rotation_z(0.5 * (p * tau * 2.0 + 0.8).sin())
        }),
        rot_channel(FOREARM_R, &t, move |p| {
            Quat::from_rotation_z(0.5 * (p * tau * 2.0 + 0.8).sin())
        }),
        // hip + chest groove
        rot_channel(HIPS, &t, move |p| Quat::from_rotation_z(0.14 * (p * tau * 2.0).sin())),
        rot_channel(CHEST, &t, move |p| {
            Quat::from_rotation_z(-0.12 * (p * tau * 2.0).sin())
                * Quat::from_rotation_y(0.1 * (p * tau).sin())
        }),
        rot_channel(HEAD, &t, move |p| Quat::from_rotation_z(0.1 * (p * tau * 2.0 + 1.0).sin())),
        // bouncing knees
        rot_channel(THIGH_L, &t, move |p| {
            Quat::from_rotation_x(0.3 * ((p * tau * 2.0).sin().abs()))
        }),
        rot_channel(THIGH_R, &t, move |p| {
            Quat::from_rotation_x(0.3 * ((p * tau * 2.0 + tau / 2.0).sin().abs()))
        }),
        rot_channel(SHIN_L, &t, move |p| {
            Quat::from_rotation_x(-0.5 * ((p * tau * 2.0).sin().abs()))
        }),
        rot_channel(SHIN_R, &t, move |p| {
            Quat::from_rotation_x(-0.5 * ((p * tau * 2.0 + tau / 2.0).sin().abs()))
        }),
    ];
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    channels.push(trans_channel(HIPS, &t, hips_bind, move |p| {
        Vec3::new(
            h * 0.02 * (p * tau).sin(),
            -h * 0.03 * ((p * tau * 2.0).sin().abs()),
            0.0,
        )
    }));
    AnimationClip { name: "dance".into(), channels }
}

fn walk_clip(rig: &Rig, h: f32) -> AnimationClip {
    let dur = 1.0;
    let t = keys(16, dur);
    let tau = core::f32::consts::TAU;
    let swing = 0.62;
    let mut channels = vec![
        rot_channel(THIGH_L, &t, move |p| Quat::from_rotation_x(swing * (p * tau).sin())),
        rot_channel(THIGH_R, &t, move |p| Quat::from_rotation_x(-swing * (p * tau).sin())),
        // shins bend only when the leg is behind
        rot_channel(SHIN_L, &t, move |p| {
            Quat::from_rotation_x(-(0.9 * (p * tau + 0.6).sin()).max(0.0) - 0.05)
        }),
        rot_channel(SHIN_R, &t, move |p| {
            Quat::from_rotation_x(-(0.9 * (p * tau + 0.6 + tau / 2.0).sin()).max(0.0) - 0.05)
        }),
        rot_channel(FOOT_L, &t, move |p| Quat::from_rotation_x(0.25 * (p * tau + 1.2).sin())),
        rot_channel(FOOT_R, &t, move |p| Quat::from_rotation_x(-0.25 * (p * tau + 1.2).sin())),
        rot_channel(ARM_L, &t, move |p| Quat::from_rotation_x(-0.45 * (p * tau).sin())),
        rot_channel(ARM_R, &t, move |p| Quat::from_rotation_x(0.45 * (p * tau).sin())),
        rot_channel(FOREARM_L, &t, move |p| {
            Quat::from_rotation_x(-0.3 - 0.15 * (p * tau).sin())
        }),
        rot_channel(FOREARM_R, &t, move |p| {
            Quat::from_rotation_x(-0.3 + 0.15 * (p * tau).sin())
        }),
        rot_channel(CHEST, &t, move |p| Quat::from_rotation_y(0.08 * (p * tau).sin())),
        rot_channel(HIPS, &t, move |p| Quat::from_rotation_y(-0.06 * (p * tau).sin())),
    ];
    let hips_bind = rig.skeleton.joints[HIPS].translation;
    channels.push(Channel {
        joint: HIPS,
        times: t.clone(),
        data: ChannelData::Translation(
            t.iter()
                .map(|&tt| hips_bind + Vec3::Y * h * 0.018 * ((tt / dur * tau * 2.0).sin().abs()))
                .collect(),
        ),
    });
    AnimationClip { name: "walk".into(), channels }
}

