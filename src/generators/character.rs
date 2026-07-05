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
    joints.push((ARM_L, Some(CHEST), "upperarm_l", Vec3::new(sw, h * 0.075, 0.0)));
    joints.push((FOREARM_L, Some(ARM_L), "forearm_l", Vec3::new(0.0, -h * 0.15, 0.0)));
    joints.push((HAND_L, Some(FOREARM_L), "hand_l", Vec3::new(0.0, -h * 0.13, 0.0)));
    joints.push((ARM_R, Some(CHEST), "upperarm_r", Vec3::new(-sw, h * 0.075, 0.0)));
    joints.push((FOREARM_R, Some(ARM_R), "forearm_r", Vec3::new(0.0, -h * 0.15, 0.0)));
    joints.push((HAND_R, Some(FOREARM_R), "hand_r", Vec3::new(0.0, -h * 0.13, 0.0)));
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

fn wardrobe(r: &mut Rand, pal: &Palette, class: CharacterClass) -> Wardrobe {
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
    let skin = skins[r.gen_range(0..skins.len())];
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

/// Limb segment: tapered tube from joint toward child (unbound — the body
/// core is smooth-skinned as a whole).
fn limb(from: Vec3, to: Vec3, r0: f32, r1: f32, color: Vec3) -> Mesh {
    let m = tube(&[(from, r0), (from + (to - from) * 0.98, r1)], 7, |_| color);
    to_flat_shaded(&m)
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
    let sw = h * 0.135 * bulk;
    let rig = build_rig(h, sw);
    let w = wardrobe(&mut r, pal, p.class);
    let jw = |i: usize| rig.world[i];

    let mut body = Mesh::new();
    // smooth-skinned torso core (limbs are bound per-region below)
    let mut core = Mesh::new();

    // pelvis: rigid to HIPS — smooth weights here let opposing thighs tear
    // the crotch open on wide strides
    let mut pelvis = to_flat_shaded(&cuboid(
        jw(HIPS) + Vec3::new(0.0, -h * 0.015, 0.0),
        Vec3::new(h * 0.095 * bulk, h * 0.05, h * 0.062 * bulk),
        w.pants,
    ));
    pelvis.bind_all_to_joint(HIPS as u16);
    body.merge(&pelvis);

    // torso: lower (spine) + upper (chest, broader)
    core.merge(&to_flat_shaded(&cuboid(
        jw(SPINE) + Vec3::new(0.0, h * 0.05, 0.0),
        Vec3::new(h * 0.09 * bulk, h * 0.068, h * 0.06 * bulk),
        w.shirt * 0.92,
    )));
    core.merge(&to_flat_shaded(&cuboid(
        jw(CHEST) + Vec3::new(0.0, h * 0.042, 0.0),
        Vec3::new(sw * 0.9, h * 0.066, h * 0.066 * bulk),
        w.shirt,
    )));
    // belt
    let mut belt = to_flat_shaded(&cuboid(
        jw(HIPS) + Vec3::new(0.0, h * 0.025, 0.0),
        Vec3::new(h * 0.098 * bulk, h * 0.014, h * 0.066 * bulk),
        w.boots * 0.7,
    ));
    belt.bind_all_to_joint(HIPS as u16);
    body.merge(&belt);
    // buckle
    let mut buckle = to_flat_shaded(&cuboid(
        jw(HIPS) + Vec3::new(0.0, h * 0.025, h * 0.066 * bulk),
        Vec3::new(h * 0.015, h * 0.015, h * 0.006),
        srgb(212, 175, 55),
    ));
    buckle.bind_all_to_joint(HIPS as u16);
    body.merge(&buckle);

    // head + face
    let head_r = h * 0.075;
    let mut head = icosphere(head_r, 2, w.skin);
    for v in head.positions.iter_mut() {
        v.x *= 0.92;
        v.y *= 1.12;
    }
    head.recompute_smooth_normals();
    let mut head = to_flat_shaded(&head);
    head.translate(jw(HEAD) + Vec3::new(0.0, head_r * 0.9, 0.0));
    head.bind_all_to_joint(HEAD as u16);
    body.merge(&head);
    // hair cap
    let mut hair = icosphere(head_r * 1.06, 2, w.hair);
    for v in hair.positions.iter_mut() {
        v.x *= 0.94;
        v.y *= 1.05;
        // carve away the face: pull front-lower verts back
        if v.z > head_r * 0.25 && v.y < head_r * 0.45 {
            v.z = head_r * 0.25;
        }
        if v.y < -head_r * 0.3 {
            v.y = -head_r * 0.3;
        }
    }
    hair.recompute_smooth_normals();
    let mut hair = to_flat_shaded(&hair);
    hair.translate(jw(HEAD) + Vec3::new(0.0, head_r * 1.02, -head_r * 0.06));
    hair.bind_all_to_joint(HEAD as u16);
    body.merge(&hair);
    // eyes
    for sx in [-1.0f32, 1.0] {
        let mut eye = cuboid(
            jw(HEAD) + Vec3::new(sx * head_r * 0.34, head_r * 0.95, head_r * 0.82),
            Vec3::new(head_r * 0.12, head_r * 0.16, head_r * 0.05),
            srgb(30, 26, 26),
        );
        eye.bind_all_to_joint(HEAD as u16);
        body.merge(&eye);
    }
    // neck
    core.merge(&to_flat_shaded(&tube(
        &[(jw(NECK) - Vec3::Y * h * 0.01, h * 0.028), (jw(NECK) + Vec3::Y * h * 0.035, h * 0.026)],
        6,
        |_| w.skin,
    )));

    // arms
    let arm_r0 = h * 0.036 * bulk;
    let forearm_col = match p.class {
        CharacterClass::Warrior | CharacterClass::Rogue => w.boots * 1.15,
        _ => w.skin,
    };
    for (aj, fj, hj) in [(ARM_L, FOREARM_L, HAND_L), (ARM_R, FOREARM_R, HAND_R)] {
        let mut arm = Mesh::new();
        arm.merge(&limb(jw(aj), jw(fj), arm_r0, arm_r0 * 0.8, w.shirt));
        arm.merge(&limb(jw(fj), jw(hj), arm_r0 * 0.78, arm_r0 * 0.62, forearm_col));
        crate::skinning::smooth_bind(&mut arm, &segs(&rig, &[(aj, fj), (fj, hj)]), 4.0);
        body.merge(&arm);
        let mut hand = to_flat_shaded(&icosphere(arm_r0 * 0.85, 1, w.skin));
        hand.translate(jw(hj) - Vec3::Y * arm_r0 * 0.1);
        hand.bind_all_to_joint(hj as u16);
        body.merge(&hand);
        // shoulder pad: armor plate for warriors, subtle sleeve puff otherwise
        let pad_r = match p.class {
            CharacterClass::Warrior => arm_r0 * 1.6,
            _ => arm_r0 * 1.2,
        };
        let mut pad = icosphere(pad_r, 1, match p.class {
            CharacterClass::Warrior => w.shirt * 0.7,
            _ => w.shirt * 1.05,
        });
        for v in pad.positions.iter_mut() {
            if v.y < 0.0 {
                v.y *= 0.3;
            }
        }
        pad.recompute_smooth_normals();
        let mut pad = to_flat_shaded(&pad);
        let side = jw(aj).x.signum();
        pad.translate(jw(aj) + Vec3::new(side * arm_r0 * 0.35, arm_r0 * 0.55, 0.0));
        pad.bind_all_to_joint(aj as u16);
        body.merge(&pad);
    }

    // legs
    let leg_r = h * 0.05 * bulk;
    for (tj, sj, fj) in [(THIGH_L, SHIN_L, FOOT_L), (THIGH_R, SHIN_R, FOOT_R)] {
        let mut leg = Mesh::new();
        leg.merge(&limb(jw(tj), jw(sj), leg_r, leg_r * 0.82, w.pants));
        leg.merge(&limb(jw(sj), jw(fj), leg_r * 0.8, leg_r * 0.6, w.boots));
        crate::skinning::smooth_bind(&mut leg, &segs(&rig, &[(tj, sj), (sj, fj)]), 4.0);
        body.merge(&leg);
        let mut foot = to_flat_shaded(&cuboid(
            jw(fj) + Vec3::new(0.0, -h * 0.006, h * 0.032),
            Vec3::new(leg_r * 0.9, h * 0.022, h * 0.07),
            w.boots,
        ));
        foot.bind_all_to_joint(fj as u16);
        body.merge(&foot);
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
            // flowing robe skirt from the hips
            let mut robe = to_flat_shaded(&lathe(
                &[
                    (h * 0.145 * bulk, -h * 0.30),
                    (h * 0.125 * bulk, -h * 0.18),
                    (h * 0.10 * bulk, -h * 0.05),
                    (h * 0.095 * bulk, h * 0.03),
                ],
                10,
                |ri, _| if ri == 0 { w.accent * 0.7 } else { w.shirt * 0.9 },
            ));
            robe.translate(jw(HIPS));
            robe.bind_all_to_joint(HIPS as u16);
            body.merge(&robe);
        }
        CharacterClass::Warrior => {
            // wrap-around cuirass with an accent trim band
            let mut plate = to_flat_shaded(&cuboid(
                jw(CHEST) + Vec3::new(0.0, h * 0.042, 0.0),
                Vec3::new(sw * 0.96, h * 0.058, h * 0.072 * bulk),
                w.shirt * 0.65,
            ));
            plate.bind_all_to_joint(CHEST as u16);
            body.merge(&plate);
            let mut trim = to_flat_shaded(&cuboid(
                jw(CHEST) + Vec3::new(0.0, h * 0.085, 0.0),
                Vec3::new(sw * 0.98, h * 0.012, h * 0.074 * bulk),
                w.accent * 0.85,
            ));
            trim.bind_all_to_joint(CHEST as u16);
            body.merge(&trim);
        }
        CharacterClass::Rogue => {
            // hood
            let mut hood = icosphere(head_r * 1.18, 1, w.shirt * 0.8);
            for v in hood.positions.iter_mut() {
                if v.z > head_r * 0.4 {
                    v.z = head_r * 0.4;
                }
            }
            hood.recompute_smooth_normals();
            let mut hood = to_flat_shaded(&hood);
            hood.translate(jw(HEAD) + Vec3::new(0.0, head_r * 1.05, -head_r * 0.15));
            hood.bind_all_to_joint(HEAD as u16);
            body.merge(&hood);
        }
        CharacterClass::Villager => {}
    }

    // smooth-skin the torso core and fold it into the body
    crate::skinning::smooth_bind(
        &mut core,
        &segs(&rig, &[(SPINE, CHEST), (CHEST, NECK), (NECK, HEAD)]),
        4.0,
    );
    body.merge(&core);

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

    Asset {
        name: "character".into(),
        parts: vec![Part {
            mesh: body,
            material: Material { roughness: 0.85, ..Default::default() },
        }],
        skeleton: Some(rig.skeleton),
        animations,
        physics: Some(Physics {
            collider: Collider::Capsule { radius: h * 0.16, height: h },
            mass: 70.0 * h / 1.7,
            friction: 0.4,
            restitution: 0.0,
        }),
    }
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
