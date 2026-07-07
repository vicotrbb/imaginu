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

/// Dispatch on archetype. `Hydra` and `Colossus` have dedicated plans; the
/// remaining archetypes fall back to `plan_hydra` so dispatch stays total
/// until Tasks 8-10 land.
pub fn build_boss_rig(p: &BossParams) -> BossRig {
    match p.archetype {
        BossArchetype::Hydra => plan_hydra(p),
        BossArchetype::Colossus => plan_colossus(p),
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

/// Colossus: a massive humanoid stone golem built on the `biped_brute`
/// layout scaled large — a heavy hunched trunk (hips/spine/chest/neck/head)
/// with thick pillar legs, two destructible `arm.l`/`arm.r` limbs, and a
/// glowing exposed `core` set into the chest cavity. Trunk is rank 0/1, legs
/// and arms are rank-2 families (arms never fuse to each other or the torso
/// since union-find only merges joints sharing a rank>=2 primitive), and the
/// core + shoulder/chest armor plates fold LAST as their own high-rank
/// families so they read crisp against the stone body (mirrors the hydra's
/// neck-rank-2 / head-detail-rank-3..5 pattern).
fn plan_colossus(p: &BossParams) -> BossRig {
    let s = p.size.clamp(0.4, 8.0);
    let bulk = 1.0 + 0.5 * p.menace.clamp(0.0, 1.0);
    let armor = p.armor.clamp(0.0, 1.0);
    let plates = p.plates.clamp(0.0, 1.0);
    let horns = p.horns.clamp(0.0, 1.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let mut parts = Vec::new();
    let mut weak_points = Vec::new();

    // trunk (rank 0/1): a TALL, narrow-ish stone torso (a towering golem
    // reads tall-and-heavy, not wide-and-round) — biped_brute's layout
    // stretched up, with long legs underneath doing most of the height.
    let hips = r.joint(None, "hips", v(0.0, 1.05 * s, 0.0));
    let spine = r.joint(Some(hips), "spine", v(0.0, 1.30 * s, 0.0));
    let chest = r.joint(Some(spine), "chest", v(0.0, 1.62 * s, -0.05 * s));
    let neck = r.joint(Some(chest), "neck", v(0.0, 1.86 * s, 0.04 * s));
    let head = r.joint(Some(neck), "head", v(0.0, 2.08 * s, 0.10 * s));
    // NOTE: the trunk radii are kept DELIBERATELY modest relative to the
    // arm/leg socket offsets below (`bk` only thickens a LITTLE beyond
    // `bulk`, 0.1 not 0.3 per armor point): an oversized flat shoulder plate
    // or barrel torso swallows the limbs into one featureless blob (the
    // "engulfment" failure — visually a web even though skinning is clean),
    // and separately widened limb sockets keep the trunk/limb SKIN families
    // geometrically separated (the union-find "web" failure). "Heavy armor"
    // instead reads through the dedicated pauldron/chestplate primitives
    // pushed after `finish`, which sit ON TOP of (not inside) the silhouette.
    let bk = bulk * (1.0 + 0.1 * armor);
    r.ellip(hips, chest, 0.22 * s * bk, 0.06 * s, 0, 0.14 * s);
    r.cone(hips, chest, 0.19 * s * bk, 0.24 * s * bk, 0, 0.14 * s);
    r.flat(
        chest,
        chest,
        v(0.32 * s * bk, 0.22 * s, 0.24 * s),
        0,
        0.1 * s,
    ); // shoulders — narrower than the arm socket offset (0.46*s) so the
    // arms read as attached limbs, not absorbed mass
    r.cone(chest, neck, 0.14 * s, 0.11 * s, 1, 0.07 * s);
    r.cone(neck, head, 0.10 * s, 0.14 * s, 1, 0.06 * s);
    r.ellip(head, head, 0.16 * s, 0.0, 1, 0.05 * s);
    parts.push(PartMeta {
        name: "head".into(),
        joint: "head".into(),
        destructible: true,
    });

    // legs (rank 2, LONG thick planted pillars — grounding, not named parts,
    // and most of the "towering" height reads through their length).
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        let th = r.joint(Some(hips), "thigh", v(side * 0.22 * s, 0.95 * s, 0.0));
        let sn = r.joint(Some(th), "shin", v(side * 0.23 * s, 0.5 * s, 0.02 * s));
        let ft = r.joint(Some(sn), "foot", v(side * 0.24 * s, 0.04 * s, 0.15 * s));
        r.cone(th, sn, 0.17 * s * bulk, 0.13 * s, 2, 0.02 * s);
        r.cone(sn, ft, 0.13 * s, 0.09 * s, 2, 0.03 * s);
        legs.push(vec![th, sn, ft]);
    }

    // arms (rank 2, each its OWN family since left/right never share a
    // rank>=2 primitive) — chunky segmented limbs, named `arm.l`/`arm.r`,
    // destructible.
    for (side, name) in [(-1.0f32, "arm.l"), (1.0f32, "arm.r")] {
        let sh = r.joint(
            Some(chest),
            &format!("{name}_upper"),
            v(side * 0.46 * s, 1.58 * s, 0.0),
        );
        let el = r.joint(
            Some(sh),
            &format!("{name}_fore"),
            v(side * 0.66 * s, 1.28 * s, 0.10 * s),
        );
        let hn = r.joint(
            Some(el),
            &format!("{name}_hand"),
            v(side * 0.76 * s, 0.94 * s, 0.16 * s),
        );
        r.cone(sh, el, 0.17 * s * bulk, 0.135 * s * bulk, 2, 0.03 * s);
        r.cone(el, hn, 0.135 * s, 0.11 * s, 2, 0.035 * s);
        r.ellip(hn, hn, 0.13 * s, 0.0, 2, 0.03 * s); // huge stone fist
        parts.push(PartMeta {
            name: name.into(),
            joint: format!("{name}_fore"),
            destructible: true,
        });
    }

    // chest core joint (rank 0/1-free: added to the builder now purely so it
    // exists as a real skeleton joint for the weak point; its glowing prim is
    // pushed AFTER `finish` with an explicit high fold rank + Eye tint so it
    // never folds into the rank-0/1 trunk band). Positioned at 0.88x the
    // chest cone's own forward radius so the glow sphere pokes clear of the
    // stone surface — an EXPOSED core in a chest cavity, not one buried
    // inside the torso where it would never be visible.
    let chest_r2 = 0.24 * s * bk;
    let core = r.joint(
        Some(chest),
        "core",
        v(0.0, 1.60 * s, -0.05 * s + 0.88 * chest_r2),
    );
    // trunk anchor: a tiny near-invisible rank-1 spoke from chest to core so
    // `core` is classified `is_trunk` (rigid to the spine chain) — the
    // GLOWING sphere pushed after `finish` is still its own higher-rank
    // family visually, but the joint's skin binding rides with the torso
    // instead of racing the nearest-limb-family classifier against the arms
    // (which sit right next to it and DO move independently under a pose).
    r.cone(chest, core, 0.02 * s, 0.02 * s, 1, 0.02 * s);

    if p.weak_points {
        weak_points.push(WeakPointMeta {
            name: "weak_point.core".into(),
            joint: "core".into(),
            collider: ColliderJson::Sphere {
                radius: (0.22 + 0.06 * plates) * s,
            },
            offset: [0.0, 0.0, 0.0],
            destructible: true,
            phase: 2,
        });
    }

    let gait = GaitDesc {
        legs,
        spine: vec![hips, spine, chest, neck, head],
        wings: Vec::new(),
        tail: Vec::new(),
        head: Some(head),
        style: Gait::Walk,
    };
    let mut rig = r.finish(gait);

    // The glowing exposed core (own rank 3 family): a bright sphere set into
    // the chest cavity, tinted Eye so the boss-level emissive floor (0.12)
    // lights it regardless of the `emissive` knob.
    push_flat(
        &mut rig,
        core,
        core,
        v(
            (0.20 + 0.05 * plates) * s,
            (0.20 + 0.05 * plates) * s,
            (0.20 + 0.05 * plates) * s,
        ),
        3,
        0.06 * s,
        PrimTint::Eye,
    );

    // Escalated armor: heavy shoulder pauldrons flanking the core (rank 4,
    // Horn tint = stone plating, NOT glowing) — these are the "phase 1
    // covered by plates" reading over the core, and a small back-plate ridge.
    add_armor_plates(&mut rig, chest, core, s, 0.5 + 0.5 * armor.max(plates));
    add_horn_crown(&mut rig, head, s, horns);

    rig.bounds = crate::generators::monster::rig::compute_bounds(&rig);

    BossRig {
        rig,
        weak_points,
        parts,
    }
}

/// Heavy stone shoulder pauldrons flanking the chest core, plus a small
/// spine-ridge plate — `intensity` (0..1, from `armor`/`plates` knobs) scales
/// their size. Own rank-4 family per plate (children of `chest`), so they
/// never web to the trunk or to each other.
fn add_armor_plates(rig: &mut MonsterRig, chest: usize, core: usize, s: f32, intensity: f32) {
    let v = Vec3::new;
    let cp = rig.joint_world(chest);
    let k = 0.12 + 0.09 * intensity;
    for side in [-1.0f32, 1.0] {
        let pb = add_joint(
            rig,
            chest,
            "pauldron",
            cp + v(side * 0.44 * s, 0.14 * s, 0.0),
        );
        // trunk anchor (see `core`'s spoke above): keeps the pauldron rigid
        // to the torso instead of racing the nearby arm for nearest-family.
        push_cone(
            rig,
            chest,
            pb,
            0.02 * s,
            0.02 * s,
            1,
            0.02 * s,
            PrimTint::Body,
        );
        push_flat(
            rig,
            pb,
            pb,
            v(k * s, k * 0.7 * s, k * 0.9 * s),
            4,
            0.05 * s,
            PrimTint::Horn,
        );
    }
    // a small brow-plate ridge just above the core, reinforcing the "phase 1
    // covered by plates" silhouette without occluding the glow entirely.
    let core_p = rig.joint_world(core);
    let bp = add_joint(
        rig,
        chest,
        "chestplate",
        core_p + v(0.0, 0.16 * s, -0.03 * s),
    );
    push_cone(
        rig,
        chest,
        bp,
        0.02 * s,
        0.02 * s,
        1,
        0.02 * s,
        PrimTint::Body,
    );
    push_flat(
        rig,
        bp,
        bp,
        v((0.30 + 0.1 * intensity) * s, 0.08 * s, 0.05 * s),
        4,
        0.05 * s,
        PrimTint::Horn,
    );
}

/// A small ring of stubby stone horns around the head crown — `horns` (0..1)
/// scales count/size. Own rank-5 family, children of `head`.
fn add_horn_crown(rig: &mut MonsterRig, head: usize, s: f32, horns: f32) {
    if horns <= 0.0 {
        return;
    }
    let v = Vec3::new;
    let hp = rig.joint_world(head);
    let n = 2 + (horns * 2.0).round() as usize; // 2..4
    for i in 0..n {
        let f = (i as f32 / n.max(1) as f32) * std::f32::consts::TAU;
        let dir = v(f.cos(), 0.0, f.sin());
        let hb = add_joint(
            rig,
            head,
            "horn",
            hp + v(0.0, 0.18 * s, 0.0) + dir * 0.05 * s,
        );
        let ht = add_joint(
            rig,
            hb,
            "horn_tip",
            hp + v(0.0, (0.18 + 0.16 * horns) * s, 0.0) + dir * 0.09 * s,
        );
        push_cone(
            rig,
            hb,
            ht,
            0.035 * s * horns.max(0.3),
            0.008 * s,
            5,
            0.012 * s,
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
    fn colossus_rig_is_wellformed() {
        let br = build_boss_rig(&boss(r#"{"kind":"boss","archetype":"colossus"}"#));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(br.rig.prims.iter().any(|d| d.fold_rank == 0), "has a core");
        assert!(
            br.parts.iter().any(|p| p.name == "arm.l" && p.destructible),
            "has destructible arm.l"
        );
        assert!(
            br.parts.iter().any(|p| p.name == "arm.r" && p.destructible),
            "has destructible arm.r"
        );
        assert!(br.parts.iter().any(|p| p.name == "head"), "has head part");
        assert!(
            br.weak_points
                .iter()
                .any(|w| w.name == "weak_point.core" && w.joint == "core" && w.destructible)
        );
        assert!(
            br.rig.skeleton.joints.iter().any(|j| j.name == "core"),
            "has a core joint"
        );
    }

    #[test]
    fn colossus_hostile_input_cannot_panic() {
        let br = build_boss_rig(&boss(
            r#"{"kind":"boss","archetype":"colossus","size":1e30,"phases":999,"eyes":999999,"horns":1e30,"plates":1e30,"armor":1e30}"#,
        ));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(n < 2000, "joint count bounded: {n}");
    }

    #[test]
    fn colossus_skinning_has_no_webs() {
        let p = boss(r#"{"kind":"boss","archetype":"colossus","element":"necrotic"}"#);
        let br = build_boss_rig(&p);
        let pal = crate::palette::by_name("necrotic");
        let mut mesh = super::super::body::build_body(&br.rig, p.size, p.detail, p.seed, 0.0, &pal);
        mesh.validate()
            .expect("bind mesh valid (no degenerate/zero-area tris)");
        crate::generators::monster::skin_body(&mut mesh, &br.rig);
        assert!(mesh.is_skinned(), "boss mesh must be skinned");

        // synthetic clip: swing both arms + bend one leg so the destructible
        // limb junctions actually move relative to the torso — the web risk
        // this probe targets.
        let skel = &br.rig.skeleton;
        let mut channels = Vec::new();
        for (i, j) in skel.joints.iter().enumerate() {
            if j.name.contains("upper") || j.name.contains("fore") || j.name == "thigh" {
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

        let moved = mesh
            .positions
            .iter()
            .zip(&posed.positions)
            .any(|(a, b)| a.distance(*b) > 0.01);
        assert!(moved, "limb bend pose should deform the mesh");

        let stretch = max_edge_stretch(&mesh, &posed);
        assert!(
            stretch < 8.0,
            "edge stretch {stretch} indicates a skinning web between arms/core/torso"
        );
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
