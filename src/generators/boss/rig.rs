//! Boss rig planning: dispatches on [`BossArchetype`] to build a
//! [`MonsterRig`] (reusing the shared monster body/skin pipeline) plus the
//! boss-specific weak-point / destructible-part metadata extracted from the
//! named joints. This is the template archetype (hydra); Tasks 7-10 add the
//! remaining archetypes following the same shape.

use glam::Vec3;

use crate::generators::monster::rig::{Gait, GaitDesc, MonsterRig, RigBuilder};
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

/// Hydra: broad low torso + haunches + tail, and `nheads` reared serpentine
/// necks fanning forward, each ending in its own head. The `core` joint
/// (rank 0) sits at the torso center and is the exposed weak point between
/// the necks. Each `neck{i}_head` joint is a targetable, destructible part.
fn plan_hydra(p: &BossParams) -> BossRig {
    let s = p.size.clamp(0.4, 8.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let mut parts = Vec::new();
    let mut weak_points = Vec::new();

    // core torso (rank 0): a low, ELONGATED body along Z (hips at the rear ->
    // shoulders up front), not a round ball — a sphere left no room below for
    // legs or behind for the tail to read past the torso silhouette. `core`
    // is a small raised chest lump on top of the shoulders where the necks
    // sprout and the exposed weak point sits.
    let hips = r.joint(None, "hips", v(0.0, 0.5 * s, -0.75 * s));
    let shoulders = r.joint(Some(hips), "shoulders", v(0.0, 0.58 * s, 0.4 * s));
    let core = r.joint(Some(shoulders), "core", v(0.0, 0.92 * s, 0.55 * s));
    let tail1 = r.joint(Some(hips), "tail1", v(0.0, 0.35 * s, -1.3 * s));
    let tail2 = r.joint(Some(tail1), "tail2", v(0.0, 0.18 * s, -1.9 * s));
    r.ellip(hips, shoulders, 0.42 * s, 0.1 * s, 0, 0.16 * s);
    r.ellip(shoulders, core, 0.28 * s, 0.05 * s, 0, 0.1 * s);
    r.cone(hips, tail1, 0.3 * s, 0.14 * s, 0, 0.07 * s);
    r.cone(tail1, tail2, 0.14 * s, 0.04 * s, 0, 0.05 * s);

    // haunches (rank 2): four thick legs planted WIDE of the (now slimmer)
    // torso so they read below its silhouette instead of being swallowed by
    // it, grounding the creature.
    for (dx, dz, name) in [
        (0.42, 0.15, "leg_fl"),
        (-0.42, 0.15, "leg_fr"),
        (0.42, -0.85, "leg_bl"),
        (-0.42, -0.85, "leg_br"),
    ] {
        let up = r.joint(
            Some(hips),
            &format!("{name}_up"),
            v(dx * s, 0.42 * s, dz * s),
        );
        let ft = r.joint(Some(up), &format!("{name}_ft"), v(dx * s, 0.0, dz * s));
        r.cone(up, ft, 0.15 * s, 0.09 * s, 2, 0.05 * s);
    }

    // nheads reared serpentine necks fanning forward-up from the core, wide
    // enough apart (base/mid/head each fan out MORE than the last) that
    // adjacent neck radii never sum past their separation — the exact
    // webbing failure the first pass hit. A height stagger (center neck
    // tallest) reads as a fan instead of a row of parallel tubes.
    let nheads = 5usize;
    for i in 0..nheads {
        let f = (i as f32 / (nheads - 1) as f32 - 0.5) * 2.0; // -1..1 fan
        let rise = 1.0 - 0.35 * f.abs(); // center neck rears highest
        let base = r.joint(
            Some(core),
            &format!("neck{i}_0"),
            v(f * 1.0 * s, (0.92 + 0.1 * rise) * s, 0.7 * s),
        );
        let mid = r.joint(
            Some(base),
            &format!("neck{i}_1"),
            v(f * 1.7 * s, (1.5 * rise) * s, 1.15 * s),
        );
        let head = r.joint(
            Some(mid),
            &format!("neck{i}_head"),
            v(f * 2.2 * s, (1.95 * rise) * s, 1.6 * s),
        );
        // fold_rank 2 (not 1/trunk): each neck must be its OWN skinning
        // family, bound to its own joint chain — a multi-neck hydra where
        // every neck rode the rigid trunk (spine-bound) couldn't rear
        // independently, and grouping them all under one rank risks the
        // union-find family split going wrong at the shared `core` root.
        r.cone(base, mid, 0.13 * s, 0.1 * s, 2, 0.045 * s);
        r.cone(mid, head, 0.1 * s, 0.1 * s, 2, 0.045 * s);
        r.ellip(head, head, 0.19 * s, 0.02 * s, 2, 0.05 * s);
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
            collider: ColliderJson::Sphere { radius: 0.4 * s },
            offset: [0.0, 0.0, 0.0],
            destructible: true,
            phase: 2,
        });
    }

    let gait = GaitDesc {
        legs: Vec::new(),
        spine: vec![hips, shoulders, core],
        wings: Vec::new(),
        tail: vec![tail1, tail2],
        head: None, // multi-headed: no single roar head
        style: Gait::Slither,
    };
    let rig = r.finish(gait);
    BossRig {
        rig,
        weak_points,
        parts,
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
