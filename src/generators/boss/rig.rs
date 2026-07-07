//! Boss rig planning: dispatches on [`BossArchetype`] to build a
//! [`MonsterRig`] (reusing the shared monster body/skin pipeline) plus the
//! boss-specific weak-point / destructible-part metadata extracted from the
//! named joints. This is the template archetype (hydra); Tasks 7-10 add the
//! remaining archetypes following the same shape.

use glam::Vec3;

use crate::generators::monster::rig::{
    Gait, GaitDesc, MonsterRig, PrimTint, RigBuilder, add_joint, push_cone, push_flat,
};
use crate::recipe::{BossArchetype, BossParams};

use super::meta::{ColliderJson, PartMeta, WeakPointMeta};

/// A planned boss: the underlying [`MonsterRig`] plus the weak points and
/// destructible parts extracted from its named joints.
pub struct BossRig {
    pub rig: MonsterRig,
    pub weak_points: Vec<WeakPointMeta>,
    pub parts: Vec<PartMeta>,
}

/// Dispatch on archetype. Only `Hydra` has a dedicated plan so far; the
/// others fall back to it so dispatch stays total until Tasks 7-10 land.
pub fn build_boss_rig(p: &BossParams) -> BossRig {
    match p.archetype {
        BossArchetype::Hydra => plan_hydra(p),
        BossArchetype::Colossus => plan_hydra(p),
        BossArchetype::Lich => plan_hydra(p),
        BossArchetype::SwarmQueen => plan_hydra(p),
        BossArchetype::DragonLord => plan_hydra(p),
    }
}

/// Hydra: a heavy coiled torso + haunches + a thick tapering tail, and
/// `nheads` LONG sinuous necks rearing up in a menacing cobra-hood fan, each
/// ending in a real wedge/skull head (elongated cranium + underslung maw + two
/// glowing infernal eyes + back-swept horns). A row of dorsal spikes runs the
/// spine and up each neck. The `core` joint (rank 0) is the exposed weak point
/// where the necks converge; each `neck{i}_head` is a targetable head part.
fn plan_hydra(p: &BossParams) -> BossRig {
    let s = p.size.clamp(0.4, 8.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let mut parts = Vec::new();
    let mut weak_points = Vec::new();

    // core torso (rank 0): a low, HEAVY elongated coil along Z (hips at the
    // rear -> shoulders up front), not a round ball. `core` is a raised chest
    // hump on the shoulders where the necks converge and the weak point sits.
    let hips = r.joint(None, "hips", v(0.0, 0.5 * s, -0.8 * s));
    let shoulders = r.joint(Some(hips), "shoulders", v(0.0, 0.6 * s, 0.45 * s));
    let core = r.joint(Some(shoulders), "core", v(0.0, 1.0 * s, 0.6 * s));
    let tail1 = r.joint(Some(hips), "tail1", v(0.0, 0.36 * s, -1.5 * s));
    let tail2 = r.joint(Some(tail1), "tail2", v(0.0, 0.22 * s, -2.2 * s));
    let tail3 = r.joint(Some(tail2), "tail3", v(0.0, 0.1 * s, -2.9 * s));
    r.ellip(hips, shoulders, 0.5 * s, 0.12 * s, 0, 0.18 * s);
    r.ellip(shoulders, core, 0.32 * s, 0.06 * s, 0, 0.12 * s);
    r.cone(hips, tail1, 0.34 * s, 0.18 * s, 0, 0.08 * s);
    r.cone(tail1, tail2, 0.18 * s, 0.08 * s, 0, 0.06 * s);
    r.cone(tail2, tail3, 0.08 * s, 0.02 * s, 0, 0.05 * s);

    // haunches (rank 2): four thick legs planted WIDE of the torso so they
    // read below its silhouette and ground the creature in a coiled crouch.
    for (dx, dz, name) in [
        (0.46, 0.2, "leg_fl"),
        (-0.46, 0.2, "leg_fr"),
        (0.46, -0.95, "leg_bl"),
        (-0.46, -0.95, "leg_br"),
    ] {
        let up = r.joint(
            Some(hips),
            &format!("{name}_up"),
            v(dx * s, 0.42 * s, dz * s),
        );
        let ft = r.joint(Some(up), &format!("{name}_ft"), v(dx * s, 0.0, dz * s));
        r.cone(up, ft, 0.17 * s, 0.1 * s, 2, 0.05 * s);
    }

    // nheads LONG serpentine necks, each a 4-joint S-curve rearing up + forward
    // in a wide cobra-hood fan. Bases spread enough that adjacent neck radii
    // never sum past their separation (the webbing failure the first pass hit);
    // a height stagger (center neck tallest) reads as a fan, not parallel tubes.
    // fold_rank 2 (NOT the rank-1 trunk band): each neck is its OWN skinning
    // family bound to its own chain, so necks rear independently and never fuse.
    let nheads = 5usize;
    let mut head_joints = Vec::new();
    for i in 0..nheads {
        let f = (i as f32 / (nheads - 1) as f32 - 0.5) * 2.0; // -1..1 fan
        let rise = 1.0 - 0.22 * f.abs(); // center necks rear highest
        // A hydra REARS its necks UP, not out: each neck is a tall S rising
        // well above the torso (like plan_serpent's reared front third, x5).
        // Horizontal spread is kept NARROW (heads sit high above the body, not
        // splayed out to the sides at torso height); the S leans back low then
        // sweeps up + forward so the heads crown forward at the top.
        let b = r.joint(
            Some(core),
            &format!("neck{i}_0"),
            v(f * 0.45 * s, 1.25 * s, 0.5 * s),
        );
        let m1 = r.joint(
            Some(b),
            &format!("neck{i}_1"),
            v(f * 0.62 * s, (2.05 * rise) * s, 0.32 * s),
        );
        let m2 = r.joint(
            Some(m1),
            &format!("neck{i}_2"),
            v(f * 0.82 * s, (2.85 * rise) * s, 0.66 * s),
        );
        let head = r.joint(
            Some(m2),
            &format!("neck{i}_head"),
            v(f * 0.95 * s, (3.25 * rise) * s, 1.02 * s),
        );
        r.cone(b, m1, 0.15 * s, 0.12 * s, 2, 0.05 * s);
        r.cone(m1, m2, 0.12 * s, 0.1 * s, 2, 0.045 * s);
        r.cone(m2, head, 0.1 * s, 0.11 * s, 2, 0.045 * s);
        head_joints.push(head);
        parts.push(PartMeta {
            name: format!("head.{}", i + 1),
            joint: format!("neck{i}_head"),
            destructible: true,
        });
    }

    if p.weak_points {
        weak_points.push(WeakPointMeta {
            name: "weak_point.core".into(),
            joint: "core".into(),
            collider: ColliderJson::Sphere { radius: 0.45 * s },
            offset: [0.0, 0.0, 0.0],
            destructible: true,
            phase: 2,
        });
    }

    let gait = GaitDesc {
        legs: Vec::new(),
        spine: vec![hips, shoulders, core],
        wings: Vec::new(),
        tail: vec![tail1, tail2, tail3],
        head: None, // multi-headed: no single roar head
        style: Gait::Slither,
    };
    let mut rig = r.finish(gait);

    // Head detail + dorsal spikes are pushed onto the FINISHED rig with the
    // promoted monster helpers so they carry PrimTint::Eye/Horn (glow/bone) —
    // the RigBuilder only makes Body prims. They fold at ranks strictly ABOVE
    // the neck rank (3=skull/maw, 4=eyes, 5=horns/spikes) so each stays crisp
    // and, being children of the neck-head joints, joins that neck's skinning
    // family (never webs across heads). This mirrors the monster knob system.
    for &head in &head_joints {
        add_head_features(&mut rig, head, s);
    }
    add_dorsal_spikes(&mut rig, s, &[hips, shoulders, core], &head_joints);

    rig.bounds = crate::generators::monster::rig::compute_bounds(&rig);

    BossRig {
        rig,
        weak_points,
        parts,
    }
}

/// Build a real predatory head on the neck-tip joint `head`: an elongated
/// cranium wedge tapering to a snout, an underslung lower jaw (a maw), two
/// small glowing eyes on the sides, and a pair of back-swept horns.
fn add_head_features(rig: &mut MonsterRig, head: usize, s: f32) {
    let v = Vec3::new;
    let hp = rig.joint_world(head);

    // cranium wedge: broad at the back (the neck-tip), tapering forward-down to
    // a snout. An explicit-radii ellipsoid gives a defined skull block.
    let snout = add_joint(rig, head, "snout", hp + v(0.0, -0.06 * s, 0.36 * s));
    push_flat(
        rig,
        head,
        snout,
        v(0.16 * s, 0.15 * s, 0.32 * s),
        3,
        0.05 * s,
        PrimTint::Body,
    );
    // underslung lower jaw = a maw jutting forward-down under the cranium,
    // dropped enough to leave a defined mouth line between it and the snout.
    let jaw = add_joint(rig, head, "jaw", hp + v(0.0, -0.19 * s, 0.3 * s));
    push_cone(
        rig,
        head,
        jaw,
        0.12 * s,
        0.05 * s,
        3,
        0.035 * s,
        PrimTint::Body,
    );

    // two BIG glowing infernal eyes set forward on the snout — the head's most
    // readable feature, so make them prominent (they read as the eyes against
    // the dark hide). Own rank-4 family, crisp.
    for side in [-1.0f32, 1.0] {
        let eb = add_joint(
            rig,
            head,
            "eye",
            hp + v(side * 0.12 * s, 0.05 * s, 0.18 * s),
        );
        let eo = add_joint(
            rig,
            eb,
            "eye_out",
            hp + v(side * 0.14 * s, 0.05 * s, 0.21 * s),
        );
        push_cone(
            rig,
            eb,
            eo,
            0.075 * s,
            0.05 * s,
            4,
            0.006 * s,
            PrimTint::Eye,
        );
    }

    // small, tight, strongly BACK-SWEPT horns hugging the crown (not tall
    // upright rabbit-ears): low Y gain, mostly rearward Z so they streak back.
    for side in [-1.0f32, 1.0] {
        let hb = add_joint(
            rig,
            head,
            "horn",
            hp + v(side * 0.08 * s, 0.08 * s, -0.02 * s),
        );
        let hm = add_joint(
            rig,
            hb,
            "horn_mid",
            hp + v(side * 0.1 * s, 0.14 * s, -0.16 * s),
        );
        let ht = add_joint(
            rig,
            hm,
            "horn_tip",
            hp + v(side * 0.1 * s, 0.16 * s, -0.32 * s),
        );
        push_cone(
            rig,
            hb,
            hm,
            0.038 * s,
            0.022 * s,
            5,
            0.012 * s,
            PrimTint::Horn,
        );
        push_cone(
            rig,
            hm,
            ht,
            0.022 * s,
            0.006 * s,
            5,
            0.01 * s,
            PrimTint::Horn,
        );
    }
}

/// A crest of bony dorsal spikes marching up the spine (`back` joints, from
/// hips to core) and a smaller crest up each neck to its head. Each spike is a
/// tiny rank-5 `Horn` family rooted on the joint it grows from.
fn add_dorsal_spikes(rig: &mut MonsterRig, s: f32, back: &[usize], heads: &[usize]) {
    let v = Vec3::new;
    // torso crest: a spike near each back joint, tallest over the shoulders.
    for (i, &j) in back.iter().enumerate() {
        let jp = rig.joint_world(j);
        let h = (0.22 - 0.03 * i as f32) * s;
        let b0 = add_joint(rig, j, "spike", jp + v(0.0, 0.24 * s, 0.0));
        let b1 = add_joint(rig, b0, "spike_tip", jp + v(0.0, 0.24 * s + h, -0.08 * s));
        push_cone(
            rig,
            b0,
            b1,
            0.05 * s,
            0.006 * s,
            5,
            0.012 * s,
            PrimTint::Horn,
        );
    }
    // a small spike on the crown-back of each head, continuing the crest line.
    for &head in heads {
        let hp = rig.joint_world(head);
        let b0 = add_joint(rig, head, "neck_spike", hp + v(0.0, 0.16 * s, -0.06 * s));
        let b1 = add_joint(rig, b0, "neck_spike_tip", hp + v(0.0, 0.28 * s, -0.14 * s));
        push_cone(
            rig,
            b0,
            b1,
            0.035 * s,
            0.005 * s,
            5,
            0.01 * s,
            PrimTint::Horn,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Mesh;
    use glam::{Mat4, Quat};

    fn boss(json: &str) -> BossParams {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn hydra_rig_is_wellformed() {
        let br = build_boss_rig(&boss(r#"{"kind":"boss","archetype":"hydra"}"#));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(br.rig.prims.iter().any(|d| d.fold_rank == 0), "has a core");
        // hydra has multiple heads -> multiple head.N parts + a core weak point
        assert!(
            br.parts
                .iter()
                .filter(|p| p.name.starts_with("head."))
                .count()
                >= 3
        );
        assert!(br.weak_points.iter().any(|w| w.name == "weak_point.core"));
    }

    #[test]
    fn hydra_hostile_input_cannot_panic() {
        let br = build_boss_rig(&boss(
            r#"{"kind":"boss","archetype":"hydra","size":1e30,"phases":999,"eyes":999999,"horns":1e30}"#,
        ));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(n < 2000, "joint count bounded: {n}");
    }

    /// Largest per-edge length ratio (posed / bind) — a stretched-triangle
    /// probe for skinning webs, mirroring
    /// `crate::generators::monster::tests::max_edge_stretch`. Since the boss
    /// clip driver doesn't exist yet (Task 6), the pose here is a synthetic
    /// clip bending every neck forward, not a real walk/attack clip.
    fn max_edge_stretch(bind: &Mesh, posed: &Mesh) -> f32 {
        let mut m = 0.0f32;
        for tri in bind.indices.chunks(3) {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let bl = bind.positions[a as usize].distance(bind.positions[b as usize]);
                let pl = posed.positions[a as usize].distance(posed.positions[b as usize]);
                if bl > 1e-5 {
                    m = m.max(pl / bl);
                }
            }
        }
        m
    }

    #[test]
    fn hydra_skinning_has_no_webs() {
        let p = boss(r#"{"kind":"boss","archetype":"hydra","element":"infernal"}"#);
        let br = build_boss_rig(&p);
        let pal = crate::palette::by_name("infernal");
        let mut mesh = super::super::body::build_body(&br.rig, p.size, p.detail, p.seed, 0.0, &pal);
        mesh.validate()
            .expect("bind mesh valid (no degenerate/zero-area tris)");
        crate::generators::monster::skin_body(&mut mesh, &br.rig);
        assert!(mesh.is_skinned(), "boss mesh must be skinned");

        // synthetic clip: bend every neck joint (base+mid) forward by ~35deg
        // so the multi-neck junctions actually move relative to the torso —
        // the web risk this probe targets.
        let skel = &br.rig.skeleton;
        let mut channels = Vec::new();
        for (i, j) in skel.joints.iter().enumerate() {
            if j.name.ends_with("_0") || j.name.ends_with("_1") {
                channels.push(crate::gltf::Channel {
                    joint: i,
                    times: vec![0.0, 1.0],
                    data: crate::gltf::ChannelData::Rotation(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_x(0.6),
                    ]),
                });
            }
        }
        let clip = crate::gltf::AnimationClip {
            name: "probe".into(),
            channels,
        };
        let globals = crate::anim::pose_at(skel, &clip, 1.0);
        let ibms: Vec<Mat4> = (0..skel.joints.len())
            .map(|i| skel.global(i).inverse())
            .collect();
        let posed = crate::anim::skin_mesh(&mesh, &globals, &ibms);

        // the pose must actually move vertices (sanity: not a no-op probe)
        let moved = mesh
            .positions
            .iter()
            .zip(&posed.positions)
            .any(|(a, b)| a.distance(*b) > 0.01);
        assert!(moved, "neck bend pose should deform the mesh");

        let stretch = max_edge_stretch(&mesh, &posed);
        assert!(
            stretch < 8.0,
            "edge stretch {stretch} indicates a skinning web between necks/torso"
        );
    }
}
