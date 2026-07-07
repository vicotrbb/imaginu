//! Boss rig planning: dispatches on [`BossArchetype`] to build a
//! [`MonsterRig`] (reusing the shared monster body/skin pipeline) plus the
//! boss-specific weak-point / destructible-part metadata extracted from the
//! named joints. This is the template archetype (hydra); Tasks 7-10 add the
//! remaining archetypes following the same shape.

use core::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::{Mat4, Quat, Vec3};

use crate::generators::monster::rig::{
    Gait, GaitDesc, MonsterRig, PrimTint, RigBuilder, add_joint, push_cone, push_flat,
};
use crate::mesh::{Mesh, cuboid, lathe};
use crate::recipe::{BossArchetype, BossParams};

use super::meta::{ColliderJson, PartMeta, WeakPointMeta};

/// A planned boss: the underlying [`MonsterRig`] plus the weak points and
/// destructible parts extracted from its named joints, plus an optional
/// extra mesh (currently only the lich's CSG throne) that isn't part of the
/// rig's SDF primitive field and must be merged into the body mesh directly
/// (see `generate`).
pub struct BossRig {
    pub rig: MonsterRig,
    pub weak_points: Vec<WeakPointMeta>,
    pub parts: Vec<PartMeta>,
    pub extra_mesh: Option<Mesh>,
}

/// Dispatch on archetype. `Hydra`, `Colossus`, and `Lich` have dedicated
/// plans; the remaining archetypes fall back to `plan_hydra` so dispatch
/// stays total until Tasks 9-10 land.
pub fn build_boss_rig(p: &BossParams) -> BossRig {
    match p.archetype {
        BossArchetype::Hydra => plan_hydra(p),
        BossArchetype::Colossus => plan_colossus(p),
        BossArchetype::Lich => plan_lich(p),
        BossArchetype::SwarmQueen => plan_swarm_queen(p),
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
        extra_mesh: None,
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

    // trunk (rank 0/1): a HULKING, hunched-forward stone torso — a broad
    // blocky barrel that leans out over the viewer, with a heavy head sunk
    // DOWN into the shoulders (almost no neck). The upper body steps forward
    // in +Z (the hunch) so the golem looms rather than standing at attention.
    let hips = r.joint(None, "hips", v(0.0, 1.02 * s, 0.0));
    let spine = r.joint(Some(hips), "spine", v(0.0, 1.28 * s, 0.03 * s));
    let chest = r.joint(Some(spine), "chest", v(0.0, 1.56 * s, 0.09 * s));
    // short, thick neck: the heavy blocky head sits sunk between the shoulder
    // slabs but its crown still clears them (the head must READ as a distinct
    // craggy skull, not vanish into the shoulder mass) — thrust forward over
    // the chest so the golem looms.
    let neck = r.joint(Some(chest), "neck", v(0.0, 1.72 * s, 0.16 * s));
    let head = r.joint(Some(neck), "head", v(0.0, 1.9 * s, 0.24 * s));
    // NOTE: the trunk radii are kept DELIBERATELY modest relative to the
    // arm/leg socket offsets below (`bk` only thickens a LITTLE beyond
    // `bulk`, 0.1 not 0.3 per armor point): an oversized flat shoulder plate
    // or barrel torso swallows the limbs into one featureless blob (the
    // "engulfment" failure — visually a web even though skinning is clean),
    // and separately widened limb sockets keep the trunk/limb SKIN families
    // geometrically separated (the union-find "web" failure). The BROAD,
    // heavy shoulder read instead comes from the big rank-4 pauldron stone
    // slabs pushed after `finish`, which sit ON TOP of (not inside) the
    // silhouette and skin rigidly to the torso.
    let bk = bulk * (1.0 + 0.1 * armor);
    // low, tight k across the trunk = a harder, more faceted rock read (less
    // soft molten blob) — the smin blend radius is what rounds junctions, so
    // shrinking it keeps craggy stone edges.
    r.ellip(hips, chest, 0.25 * s * bk, 0.05 * s, 0, 0.08 * s); // broad barrel
    r.cone(hips, chest, 0.23 * s * bk, 0.26 * s * bk, 0, 0.08 * s);
    r.flat(
        chest,
        chest,
        v(0.4 * s * bk, 0.2 * s, 0.26 * s),
        0,
        0.06 * s,
    ); // broad blocky shoulders — half-width (0.40) still < the arm socket
    // offset (0.52*s) so the arms read as attached limbs, not absorbed mass
    r.cone(chest, neck, 0.16 * s, 0.15 * s, 1, 0.04 * s); // thick stump neck
    r.cone(neck, head, 0.15 * s, 0.16 * s, 1, 0.04 * s);
    // heavy BLOCKY head (an anisotropic slab, not a round ball) with a jutting
    // craggy brow, thrust forward and clearing the shoulder slabs.
    r.flat(head, head, v(0.24 * s, 0.24 * s, 0.28 * s), 1, 0.035 * s);
    parts.push(PartMeta {
        name: "head".into(),
        joint: "head".into(),
        destructible: true,
    });

    // legs (rank 2, thick planted stone PILLARS in a wide braced stance —
    // grounding, not named parts. Blocky feet anchor the mass.).
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        let th = r.joint(Some(hips), "thigh", v(side * 0.26 * s, 0.86 * s, 0.0));
        let sn = r.joint(Some(th), "shin", v(side * 0.27 * s, 0.44 * s, 0.02 * s));
        let ft = r.joint(Some(sn), "foot", v(side * 0.28 * s, 0.05 * s, 0.18 * s));
        r.cone(th, sn, 0.22 * s * bulk, 0.19 * s, 2, 0.02 * s);
        r.cone(sn, ft, 0.19 * s, 0.14 * s, 2, 0.025 * s);
        r.flat(ft, ft, v(0.17 * s, 0.09 * s, 0.24 * s), 2, 0.02 * s); // blocky foot
        legs.push(vec![th, sn, ft]);
    }

    // arms (rank 2, each its OWN family since left/right never share a
    // rank>=2 primitive) — heavy blocky segmented limbs hanging forward and
    // low in a hunched, ape-like set, ending in BIG stone fists near the
    // knees. Named `arm.l`/`arm.r`, destructible.
    for (side, name) in [(-1.0f32, "arm.l"), (1.0f32, "arm.r")] {
        let sh = r.joint(
            Some(chest),
            &format!("{name}_upper"),
            v(side * 0.52 * s, 1.5 * s, 0.06 * s),
        );
        let el = r.joint(
            Some(sh),
            &format!("{name}_fore"),
            v(side * 0.62 * s, 1.08 * s, 0.18 * s),
        );
        let hn = r.joint(
            Some(el),
            &format!("{name}_hand"),
            v(side * 0.66 * s, 0.72 * s, 0.24 * s),
        );
        r.cone(sh, el, 0.2 * s * bulk, 0.18 * s * bulk, 2, 0.025 * s);
        r.cone(el, hn, 0.18 * s, 0.16 * s, 2, 0.03 * s);
        r.flat(hn, hn, v(0.2 * s, 0.19 * s, 0.2 * s), 2, 0.025 * s); // BIG blocky fist
        parts.push(PartMeta {
            name: name.into(),
            joint: format!("{name}_fore"),
            destructible: true,
        });
    }

    // chest core joint (rank 0/1-free: added to the builder now purely so it
    // exists as a real skeleton joint for the weak point; its glowing prim is
    // pushed AFTER `finish` with an explicit high fold rank + Eye tint so it
    // never folds into the rank-0/1 trunk band). Positioned so the glow
    // sphere pokes clear of the chest's forward stone surface — an EXPOSED,
    // large, central molten core, not one buried inside the torso.
    let chest_r2 = 0.26 * s * bk;
    let core = r.joint(
        Some(chest),
        "core",
        v(0.0, 1.52 * s, 0.09 * s + 0.78 * chest_r2),
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

    // The glowing exposed core (own rank 3 family): a LARGE, central molten
    // sphere in the chest cavity, tinted Eye so it paints full accent albedo
    // and reads as the boss's brightest element against the dark stone body.
    let core_r = (0.24 + 0.05 * plates) * s;
    push_flat(
        &mut rig,
        core,
        core,
        v(core_r, core_r, core_r),
        3,
        0.05 * s,
        PrimTint::Eye,
    );

    // Escalated armor: big ANGULAR stone pauldron slabs capping the shoulders
    // (rank 4, Body tint = dark rock, NOT pale bone) that broaden the
    // silhouette into a hulking wedge, plus a heavy stone brow-plate arching
    // over the core. These are the "phase 1 covered by plates" reading.
    add_armor_plates(&mut rig, chest, core, s, 0.6 + 0.4 * armor.max(plates));
    add_horn_crown(&mut rig, head, s, horns);

    rig.bounds = crate::generators::monster::rig::compute_bounds(&rig);

    BossRig {
        rig,
        weak_points,
        parts,
        extra_mesh: None,
    }
}

/// Big ANGULAR stone pauldron slabs capping the shoulders, plus a heavy
/// stone brow-plate arching over the core — `intensity` (0..1, from
/// `armor`/`plates` knobs) scales their size. Body tint (dark rock, not pale
/// bone) so they read as part of the stone golem, and a small tight `k` keeps
/// them slab-edged rather than melting into the torso. Own rank-4 family per
/// plate (children of `chest`) with a rank-1 trunk-anchor spoke so they skin
/// rigidly to the torso and never web to the trunk, the arms, or each other.
fn add_armor_plates(rig: &mut MonsterRig, chest: usize, core: usize, s: f32, intensity: f32) {
    let v = Vec3::new;
    let cp = rig.joint_world(chest);
    let g = 0.16 + 0.08 * intensity; // base slab half-extent
    for side in [-1.0f32, 1.0] {
        // slab sits at shoulder level (below the head crown) and OUTBOARD of
        // the shoulder, overhanging the arm socket to broaden the silhouette
        // into a hulking wedge.
        let pb = add_joint(
            rig,
            chest,
            "pauldron",
            cp + v(side * 0.5 * s, 0.1 * s, 0.02 * s),
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
        // a WIDE, deep, shallow slab (thin on Y) = a flat stone shoulder
        // plate; a second slab canted outboard-and-down breaks the round
        // silhouette into an angular, faceted two-plate pauldron.
        push_flat(
            rig,
            pb,
            pb,
            v(g * 1.5 * s, g * 0.6 * s, g * 1.25 * s),
            4,
            0.025 * s,
            PrimTint::Body,
        );
        let po = add_joint(
            rig,
            pb,
            "pauldron_out",
            cp + v(side * 0.72 * s, -0.04 * s, 0.0),
        );
        // trunk-anchor `po` too (it sits right beside the arm socket; without
        // an anchor it would form a stray single-joint limb family and web).
        push_cone(
            rig,
            chest,
            po,
            0.02 * s,
            0.02 * s,
            1,
            0.02 * s,
            PrimTint::Body,
        );
        push_flat(
            rig,
            po,
            po,
            v(g * 0.75 * s, g * 0.5 * s, g * 1.0 * s),
            4,
            0.025 * s,
            PrimTint::Body,
        );
    }
    // a heavy stone brow-plate arching just above the core — reinforces the
    // "core set in an armored chest cavity" read without occluding the glow.
    let core_p = rig.joint_world(core);
    let bp = add_joint(
        rig,
        chest,
        "chestplate",
        core_p + v(0.0, 0.24 * s, -0.02 * s),
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
        v((0.34 + 0.1 * intensity) * s, 0.1 * s, 0.08 * s),
        4,
        0.03 * s,
        PrimTint::Body,
    );
}

/// A pair of short, thick, back-swept craggy stone horns low on the brow —
/// `horns` (0..1) scales their length. Deliberately NOT tall upright antennae
/// (which read comical on a golem): low Y-rise, strong rearward sweep, stubby.
/// Own rank-5 family, children of `head`.
fn add_horn_crown(rig: &mut MonsterRig, head: usize, s: f32, horns: f32) {
    if horns <= 0.0 {
        return;
    }
    let v = Vec3::new;
    let hp = rig.joint_world(head);
    let len = 0.06 + 0.1 * horns;
    for side in [-1.0f32, 1.0] {
        let hb = add_joint(
            rig,
            head,
            "horn",
            hp + v(side * 0.14 * s, 0.12 * s, 0.06 * s),
        );
        let ht = add_joint(
            rig,
            hb,
            "horn_tip",
            hp + v(side * 0.18 * s, (0.12 + 0.5 * len) * s, (0.06 - len) * s),
        );
        // Body tint (dark rock), NOT bone/pale: a golem's horns are craggy
        // stone protrusions, not keratin — a pale nub reads as an odd bright
        // dot on the dark head.
        push_cone(
            rig,
            hb,
            ht,
            0.06 * s,
            0.015 * s,
            5,
            0.01 * s,
            PrimTint::Body,
        );
    }
}

/// Lich / overlord (a GALLERY HERO — palette necrotic, never element-switched):
/// a GAUNT robed caster on a slender biped skeleton (thin torso/limbs, a
/// sweeping robe hem hiding the legs, a narrow skull), topped with a bony
/// CROWN and glowing eye sockets. `core` (rank-3 own family, Eye tint) is a
/// small glowing green phylactery set into the chest — the weak point. A
/// hovering ring of 3 `implement.N` orbs/blades (each its own rank-6 family,
/// a `pivot` co-located with `core` -> a tip offset by the orbit radius) spin
/// slowly in `idle` (see `boss::anim::add_implement_orbit`). A static, CSG
/// carved stone `throne` (built from real closed-solid booleans, NOT an SDF
/// primitive — see `build_throne_mesh`) sits behind the lich as a pedestal;
/// it is merged into the body mesh in `generate`, and a small enclosing
/// rank-7 anchor primitive here gives `skin_body`'s nearest-primitive
/// classifier something rigid to bind those merged vertices to. Bounds are
/// captured BEFORE the throne anchor is added so the whole-body collider and
/// the marching-cubes sample box stay tight to the caster, excluding the
/// throne (per the brief).
fn plan_lich(p: &BossParams) -> BossRig {
    let s = p.size.clamp(0.4, 8.0);
    let crown_k = p.crown.clamp(0.0, 1.0);
    let regalia = p.regalia.clamp(0.0, 1.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let mut parts = Vec::new();
    let mut weak_points = Vec::new();

    // GAUNT ROBED SORCERER-KING. An UPRIGHT, slender column of a body —
    // narrow shoulders, a narrow waist — wrapped in a long necrotic robe: a
    // distinct A-line SKIRT that stays narrow down the upper body and FLARES
    // only near the ground (two stacked cones = a concave flare), plus
    // vertical robe-fold ridges. Deliberately NOT the smooth teardrop /
    // bowling-pin a single hip->floor taper produces.
    let hips = r.joint(None, "hips", v(0.0, 0.98 * s, 0.0));
    let waist = r.joint(Some(hips), "waist", v(0.0, 1.18 * s, 0.0));
    let chest = r.joint(Some(waist), "chest", v(0.0, 1.48 * s, 0.02 * s));
    let neck = r.joint(Some(chest), "neck", v(0.0, 1.66 * s, 0.05 * s));
    let head = r.joint(Some(neck), "head", v(0.0, 1.82 * s, 0.07 * s));
    let skirt_mid = r.joint(Some(hips), "skirt_mid", v(0.0, 0.5 * s, 0.02 * s));
    let hem = r.joint(Some(skirt_mid), "hem", v(0.0, 0.02 * s, 0.04 * s));
    // upper body: a NARROW vertical column (waist -> chest), tight `k` so it
    // reads as a slim robed torso, not a barrel.
    r.cone(waist, chest, 0.1 * s, 0.115 * s, 0, 0.045 * s);
    r.ellip(waist, chest, 0.09 * s, 0.02 * s, 0, 0.045 * s);
    // skirt: narrow from the hips to mid-thigh, a GENTLE taper below (a small
    // bottom cap, NOT a big round-cone sphere — that sphere cap was the
    // teardrop bulb), then a WIDE, FLAT hem ellipsoid (thin on Y) that flares
    // the skirt into an A-line bell with a flat cloth hem, not a bowling pin.
    r.cone(hips, skirt_mid, 0.11 * s, 0.14 * s, 0, 0.05 * s);
    r.cone(skirt_mid, hem, 0.14 * s, 0.16 * s, 0, 0.05 * s);
    r.flat(hem, hem, v(0.31 * s, 0.11 * s, 0.29 * s), 0, 0.06 * s);
    // narrow bony shoulders (thin on Y so they read as clavicles, not a slab)
    r.flat(chest, chest, v(0.2 * s, 0.075 * s, 0.11 * s), 0, 0.045 * s);
    r.cone(chest, neck, 0.05 * s, 0.048 * s, 1, 0.03 * s); // thin neck
    r.cone(neck, head, 0.048 * s, 0.06 * s, 1, 0.03 * s);
    parts.push(PartMeta {
        name: "head".into(),
        joint: "head".into(),
        destructible: true,
    });

    // legs (rank 2, thin — hidden under the flared robe skirt but present so
    // the walk gait actually swings limbs under the cloth).
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        let th = r.joint(Some(hips), "thigh", v(side * 0.08 * s, 0.62 * s, 0.0));
        let sn = r.joint(Some(th), "shin", v(side * 0.08 * s, 0.32 * s, 0.02 * s));
        let ft = r.joint(Some(sn), "foot", v(side * 0.09 * s, 0.03 * s, 0.1 * s));
        r.cone(th, sn, 0.055 * s, 0.04 * s, 2, 0.018 * s);
        r.cone(sn, ft, 0.04 * s, 0.03 * s, 2, 0.02 * s);
        legs.push(vec![th, sn, ft]);
    }

    // TWO real gaunt arms, ASYMMETRIC (each its own rank-2 family). The LEFT
    // is RAISED in a casting/summoning gesture — an open bony hand lifted
    // toward the orbiting implements; the RIGHT hangs LOWERED at the hip.
    // Both wear a wide draped SLEEVE (an ellipsoid bulge at the forearm) so
    // they read as long robed arms, not skeletal sticks.
    {
        // LEFT — raised, open hand toward the implements
        let sh = r.joint(Some(chest), "upperarm_l", v(-0.19 * s, 1.46 * s, 0.03 * s));
        let el = r.joint(Some(sh), "forearm_l", v(-0.35 * s, 1.62 * s, 0.16 * s));
        let hn = r.joint(Some(el), "hand_l", v(-0.42 * s, 1.86 * s, 0.3 * s));
        r.cone(sh, el, 0.05 * s, 0.042 * s, 2, 0.02 * s);
        r.cone(el, hn, 0.042 * s, 0.03 * s, 2, 0.018 * s);
        r.ellip(el, el, 0.085 * s, 0.03 * s, 2, 0.025 * s); // wide draped sleeve
        r.ellip(hn, hn, 0.05 * s, 0.0, 2, 0.014 * s); // open bony hand
        parts.push(PartMeta {
            name: "arm.l".into(),
            joint: "forearm_l".into(),
            destructible: true,
        });
    }
    {
        // RIGHT — lowered, draped at the hip
        let sh = r.joint(Some(chest), "upperarm_r", v(0.19 * s, 1.44 * s, 0.03 * s));
        let el = r.joint(Some(sh), "forearm_r", v(0.27 * s, 1.08 * s, 0.1 * s));
        let hn = r.joint(Some(el), "hand_r", v(0.29 * s, 0.76 * s, 0.16 * s));
        r.cone(sh, el, 0.05 * s, 0.042 * s, 2, 0.02 * s);
        r.cone(el, hn, 0.042 * s, 0.03 * s, 2, 0.018 * s);
        r.ellip(el, el, 0.09 * s, 0.04 * s, 2, 0.025 * s); // wide draped sleeve
        r.ellip(hn, hn, 0.05 * s, 0.0, 2, 0.014 * s); // bony hand
        parts.push(PartMeta {
            name: "arm.r".into(),
            joint: "forearm_r".into(),
            destructible: true,
        });
    }

    let gait = GaitDesc {
        legs,
        spine: vec![hips, waist, chest, neck, head],
        wings: Vec::new(),
        tail: Vec::new(),
        head: Some(head),
        style: Gait::Walk,
    };
    let mut rig = r.finish(gait);

    add_lich_skull(&mut rig, head, s);
    add_robe_folds(&mut rig, hips, s);

    // chest phylactery (own rank-3 family, Eye tint = full accent glow): a
    // small green orb set INTO the chest, popping against the dark robe —
    // the weak point.
    let chest_wp = rig.joint_world(chest);
    let core = add_joint(
        &mut rig,
        chest,
        "core",
        chest_wp + v(0.0, 0.02 * s, 0.11 * s),
    );
    let core_r = 0.075 * s;
    push_flat(
        &mut rig,
        core,
        core,
        v(core_r, core_r, core_r),
        3,
        0.03 * s,
        PrimTint::Eye,
    );

    add_lich_eyes(&mut rig, head, s);
    add_crown(&mut rig, head, s, crown_k, regalia);

    // floating implements: a hovering ring of 2-3 orbs/blades around the
    // phylactery, each its OWN rank-6 family — a `pivot` joint co-located
    // with `core` -> an `implement.N` tip offset by the orbit radius, so
    // rotating `pivot` in `idle` spins the tip around `core` with no
    // geometry spanning the gap (no visible rod poking through the body).
    let core_wp = rig.joint_world(core);
    for i in 0..3usize {
        let phase = i as f32 / 3.0 * TAU;
        let radius = (0.5 + 0.04 * i as f32) * s;
        let y = 0.12 * s * (i as f32 - 1.0);
        let pivot = add_joint(&mut rig, core, "implement_pivot", core_wp);
        let tip = add_joint(
            &mut rig,
            pivot,
            &format!("implement.{}", i + 1),
            core_wp + v(radius * phase.cos(), y, radius * phase.sin()),
        );
        if i % 2 == 0 {
            // a floating glowing orb
            let orb_r = 0.075 * s;
            push_flat(
                &mut rig,
                tip,
                tip,
                v(orb_r, orb_r, orb_r),
                6,
                0.02 * s,
                PrimTint::Eye,
            );
        } else {
            // a floating blade: a thin flat anisotropic sliver
            push_flat(
                &mut rig,
                tip,
                tip,
                v(0.02 * s, 0.16 * s, 0.05 * s),
                6,
                0.015 * s,
                PrimTint::Horn,
            );
        }
        parts.push(PartMeta {
            name: format!("implement.{}", i + 1),
            joint: format!("implement.{}", i + 1),
            destructible: true,
        });
    }

    if p.weak_points {
        weak_points.push(WeakPointMeta {
            name: "weak_point.phylactery".into(),
            joint: "core".into(),
            collider: ColliderJson::Sphere {
                radius: core_r * 1.4,
            },
            offset: [0.0, 0.0, 0.0],
            destructible: true,
            phase: 2,
        });
    }

    // Capture bounds NOW — body + crown + eyes + implements, the real
    // silhouette — BEFORE the throne anchor below, so `fit_collider` and
    // `build_body`'s marching-cubes sample box stay tight to the caster and
    // never balloon to include the throne behind it.
    rig.bounds = crate::generators::monster::rig::compute_bounds(&rig);

    // static throne joint, seated DIRECTLY BEHIND the lich on the SAME
    // central axis (x = 0), pulled in close so the seat overlaps the caster's
    // robe (the lich floats in FRONT of the throne, partly occluding the
    // seat) — a single enthroned silhouette, not "a lich AND a chair". It is
    // scaled UP so its carved backrest looms taller than the crown. The
    // throne never animates. An anchor primitive on the BACKREST slab (own
    // rank-7 family, kept thin-in-Z entirely behind the caster so it captures
    // no robe vertex) gives `skin_body`'s nearest-primitive classifier a
    // rigid static family to bind the merged CSG throne mesh's back verts to;
    // the seat's front verts fall to the (also static) trunk.
    let throne_pos = v(0.0, 0.62 * s, -0.24 * s);
    let throne = add_joint(&mut rig, hips, "throne", throne_pos);
    let throne_back = add_joint(
        &mut rig,
        throne,
        "throne_back",
        throne_pos + v(0.0, 0.9 * s, -0.32 * s),
    );
    push_flat(
        &mut rig,
        throne_back,
        throne_back,
        v(0.42 * s, 1.0 * s, 0.16 * s),
        7,
        0.02 * s,
        PrimTint::Body,
    );
    parts.push(PartMeta {
        name: "throne".into(),
        joint: "throne".into(),
        destructible: true,
    });

    BossRig {
        rig,
        weak_points,
        parts,
        extra_mesh: Some(build_throne_mesh(s, throne_pos)),
    }
}

/// Two small glowing green eye sockets sunk into the gaunt skull, each an
/// Eye-tinted rank-4 family.
fn add_lich_eyes(rig: &mut MonsterRig, head: usize, s: f32) {
    let v = Vec3::new;
    let hp = rig.joint_world(head);
    for side in [-1.0f32, 1.0] {
        let eb = add_joint(
            rig,
            head,
            "eye",
            hp + v(side * 0.045 * s, 0.01 * s, 0.07 * s),
        );
        push_flat(
            rig,
            eb,
            eb,
            v(0.018 * s, 0.018 * s, 0.018 * s),
            4,
            0.006 * s,
            PrimTint::Eye,
        );
    }
}

/// A fan of thin bone-toned crown spikes across the brow (`crown` scales
/// their height) plus a thin circlet band wrapping the skull (`regalia`
/// scales its thickness) — the "regalia" read. Own rank-5 family, children
/// of `head`.
fn add_crown(rig: &mut MonsterRig, head: usize, s: f32, crown: f32, regalia: f32) {
    let v = Vec3::new;
    let hp = rig.joint_world(head);
    let n = 5usize;
    let h = (0.1 + 0.16 * crown) * s;
    for i in 0..n {
        let f = (i as f32 / (n - 1) as f32 - 0.5) * 2.0; // -1..1 fan across the brow
        let base = hp + v(f * 0.085 * s, 0.09 * s, 0.03 * s - 0.02 * f.abs() * s);
        let b0 = add_joint(rig, head, "crown_spike", base);
        let b1 = add_joint(
            rig,
            b0,
            "crown_spike_tip",
            base + v(f * 0.02 * s, h, -0.02 * s),
        );
        push_cone(
            rig,
            b0,
            b1,
            0.026 * s,
            0.007 * s,
            5,
            0.014 * s,
            PrimTint::Horn,
        );
    }
    if regalia > 0.0 {
        let band_r = (0.028 + 0.014 * regalia) * s;
        push_flat(
            rig,
            head,
            head,
            v(0.11 * s, band_r, 0.11 * s),
            5,
            0.012 * s,
            PrimTint::Horn,
        );
    }
}

/// A gaunt, skeletal undead SKULL beneath the crown (rank-1 trunk band, so
/// it rides rigidly with the `head` joint): a narrow tall cranium, a heavy
/// BROW shelf overhanging the (glowing) eye sockets so they read as sunken,
/// two sharp cheekbones, and a tapered pointed JAW — a skull, not a smooth
/// ball. All `PrimTint::Body` (dark) so only the eyes/crown glow.
fn add_lich_skull(rig: &mut MonsterRig, head: usize, s: f32) {
    let v = Vec3::new;
    let hp = rig.joint_world(head);
    // cranium: a narrow, slightly tall skull dome
    push_flat(
        rig,
        head,
        head,
        v(0.095 * s, 0.11 * s, 0.1 * s),
        1,
        0.025 * s,
        PrimTint::Body,
    );
    // heavy brow shelf overhanging the eyes (sunken, menacing sockets)
    let brow = add_joint(rig, head, "brow", hp + v(0.0, 0.035 * s, 0.07 * s));
    push_flat(
        rig,
        brow,
        brow,
        v(0.1 * s, 0.028 * s, 0.045 * s),
        1,
        0.012 * s,
        PrimTint::Body,
    );
    // sharp cheekbones
    for side in [-1.0f32, 1.0] {
        let ch = add_joint(
            rig,
            head,
            "cheek",
            hp + v(side * 0.062 * s, -0.02 * s, 0.06 * s),
        );
        push_flat(
            rig,
            ch,
            ch,
            v(0.028 * s, 0.03 * s, 0.03 * s),
            1,
            0.01 * s,
            PrimTint::Body,
        );
    }
    // tapered pointed jaw
    let chin = add_joint(rig, head, "chin", hp + v(0.0, -0.14 * s, 0.05 * s));
    push_cone(
        rig,
        head,
        chin,
        0.075 * s,
        0.028 * s,
        1,
        0.02 * s,
        PrimTint::Body,
    );
}

/// Vertical robe-fold ridges running down the flared skirt (rank-0 trunk
/// band, children of `hips`), wrapping the FRONT and SIDES (the back is left
/// clear where the throne sits). Each is a thin proud `Body` cone that
/// breaks the skirt's smooth surface into legible cloth folds.
fn add_robe_folds(rig: &mut MonsterRig, hips: usize, s: f32) {
    let v = Vec3::new;
    let hipw = rig.joint_world(hips);
    let n = 7usize;
    for i in 0..n {
        let frac = i as f32 / (n - 1) as f32;
        let theta = (frac - 0.5) * PI * 1.35; // -121°..121° from +Z (skip the back)
        let (top_y, r_top) = (0.5 * s, 0.15 * s);
        let (bot_y, r_bot) = (0.06 * s, 0.31 * s);
        let top = hipw + v(theta.sin() * r_top, top_y - hipw.y, theta.cos() * r_top);
        let bot = hipw + v(theta.sin() * r_bot, bot_y - hipw.y, theta.cos() * r_bot);
        let jt = add_joint(rig, hips, "robe_fold", top);
        let jb = add_joint(rig, jt, "robe_fold_tip", bot);
        push_cone(
            rig,
            jt,
            jb,
            0.02 * s,
            0.03 * s,
            0,
            0.015 * s,
            PrimTint::Body,
        );
    }
}

/// Build the lich's throne as a literal, CSG-carved closed-solid mesh (NOT
/// an SDF primitive): a seat slab on four stubby feet, a tall backrest with
/// a carved gothic arch window, and armrests with finial spikes. `pos` is
/// the throne joint's world position; the returned mesh is already
/// translated there. The arch cutter is a CLOSED solid — the lathe profile
/// touches the axis (`r=0`) at both ends, so revolving it yields a capped,
/// watertight capsule, not an open tube; an open cutter would silently fail
/// to carve (see the CSG closed-solids rule in the task brief).
fn build_throne_mesh(s: f32, pos: Vec3) -> Mesh {
    let v = Vec3::new;
    let stone = crate::palette::srgb(19, 24, 18); // near-black necrotic basalt
    let trim = crate::palette::srgb(46, 52, 40); // faint lighter trim, same dark hue
    let w = 0.4 * s; // seat/back half-width
    let mut m = Mesh::new();
    // wide seat slab (built around y = 0 at the seat, then translated by pos)
    m.merge(&cuboid(v(0.0, 0.0, 0.0), v(w, 0.08 * s, 0.34 * s), stone));
    // heavy tapered pedestal base grounding the throne
    m.merge(&cuboid(
        v(0.0, -0.34 * s, -0.02 * s),
        v(w * 0.9, 0.26 * s, 0.3 * s),
        stone,
    ));
    m.merge(&cuboid(
        v(0.0, -0.6 * s, -0.02 * s),
        v(w * 1.05, 0.06 * s, 0.36 * s),
        stone,
    ));
    // TALL backrest slab that looms JUST ABOVE the crown (not dwarfing the
    // caster), with a carved gothic arch window. Backrest spans y ~0.1..1.6s
    // -> world top ~2.24s (crown tips sit ~2.2s), and the finials add the
    // looming apex above that.
    let mut back = cuboid(v(0.0, 0.85 * s, -0.3 * s), v(w, 0.75 * s, 0.07 * s), stone);
    // gothic arch cutter: a capped cylinder along Z (profile touches the axis
    // r=0 at BOTH ends -> a CLOSED solid; an open tube would silently fail to
    // carve). Sized to punch a tall window through the thin backrest slab,
    // centered behind the caster's upper body so the glow reads against it.
    let mut bore = lathe(
        &[
            (0.0, -0.4 * s),
            (0.22 * s, -0.4 * s),
            (0.22 * s, 0.42 * s),
            (0.0, 0.5 * s),
        ],
        12,
        |_, _| stone,
    );
    bore.transform(Mat4::from_rotation_translation(
        Quat::from_rotation_x(FRAC_PI_2),
        v(0.0, 0.85 * s, -0.3 * s),
    ));
    back = crate::csg::subtract(&back, &bore);
    m.merge(&back);
    // vertical pilaster ridges framing the backrest edges (grandeur)
    for side in [-1.0f32, 1.0] {
        m.merge(&cuboid(
            v(side * (w - 0.03 * s), 0.85 * s, -0.28 * s),
            v(0.05 * s, 0.75 * s, 0.08 * s),
            trim,
        ));
    }
    // armrests + front supports + finial spikes
    for side in [-1.0f32, 1.0] {
        m.merge(&cuboid(
            v(side * w, 0.28 * s, 0.02 * s),
            v(0.06 * s, 0.06 * s, 0.3 * s),
            trim,
        ));
        m.merge(&cuboid(
            v(side * w, 0.08 * s, 0.3 * s),
            v(0.06 * s, 0.2 * s, 0.06 * s),
            stone,
        ));
        // corner finial spikes crowning the backrest
        m.merge(&cuboid(
            v(side * (w - 0.04 * s), 1.7 * s, -0.3 * s),
            v(0.05 * s, 0.12 * s, 0.05 * s),
            trim,
        ));
    }
    // a taller central crowning finial so the throne reads as a looming apex
    m.merge(&cuboid(
        v(0.0, 1.78 * s, -0.3 * s),
        v(0.06 * s, 0.18 * s, 0.06 * s),
        trim,
    ));
    m.translate(pos);
    m
}

/// Swarm-queen: a HUGE insectoid brood-mother built on the `insectoid` body
/// plan's shape (thorax/head/abdomen + 6 legs) scaled large, with the
/// abdomen swollen into a massive bulbous mass studded with glowing
/// `brood_sac.N` weak points — the money detail. Thorax + head + abdomen are
/// the rank 0/1 trunk; each of the 6 legs is its own rank-2 family, planted
/// WIDE so six legs on one broad thorax never web together; the brood sacs
/// are fold-ranked LAST (rank 5) as their own families (each its own
/// base->tip joint pair, Eye tint) so every sac pops as a distinct glowing
/// pustule against the dark carapace, never fusing to the abdomen or to each
/// other. `abdomen` and `head` are named destructible parts; each
/// `weak_point.brood_sac.i` is a destructible weak point.
fn plan_swarm_queen(p: &BossParams) -> BossRig {
    let s = p.size.clamp(0.4, 8.0);
    let spikes = p.spikes.clamp(0.0, 1.0);
    let eyes_n = p.eyes.clamp(2, 16) as usize;
    let menace = p.menace.clamp(0.0, 1.0);
    let v = Vec3::new;
    let mut r = RigBuilder::new();
    let mut parts = Vec::new();
    let mut weak_points = Vec::new();

    // trunk (rank 0/1): a broad thorax core, a fanged head thrust forward,
    // and a MASSIVE bulbous abdomen trailing behind and sagging low (a
    // brood-mother's egg-heavy abdomen drags, not held aloft like a spider's).
    let thorax = r.joint(None, "thorax", v(0.0, 0.95 * s, 0.15 * s));
    let head = r.joint(Some(thorax), "head", v(0.0, 1.05 * s, 0.85 * s));
    let abdomen = r.joint(Some(thorax), "abdomen", v(0.0, 0.75 * s, -1.15 * s));
    let abtip = r.joint(Some(abdomen), "abtip", v(0.0, 0.5 * s, -2.15 * s));
    r.ellip(thorax, thorax, 0.42 * s, 0.05 * s, 0, 0.14 * s); // broad thorax
    r.ellip(head, head, 0.28 * s, 0.0, 1, 0.1 * s); // head
    r.cone(thorax, head, 0.24 * s, 0.22 * s, 0, 0.09 * s);
    // massive bulbous abdomen: a wide connecting cone + two big overlapping
    // ellipsoids so it swells then tapers to a blunt tip, not a smooth
    // teardrop cone.
    r.cone(thorax, abdomen, 0.3 * s, 0.58 * s, 0, 0.16 * s);
    r.ellip(abdomen, abdomen, 0.66 * s, 0.0, 0, 0.2 * s);
    r.ellip(abdomen, abtip, 0.58 * s, 0.16 * s, 0, 0.18 * s);
    parts.push(PartMeta {
        name: "abdomen".into(),
        joint: "abdomen".into(),
        destructible: true,
    });
    parts.push(PartMeta {
        name: "head".into(),
        joint: "head".into(),
        destructible: true,
    });

    // 6 legs off the thorax (own rank-2 family each) — the web risk with six
    // legs sharing one broad thorax, so bases spread wide and feet splay
    // further still. STURDY and planted WIDE in a bracing, grounded stance:
    // the three leg pairs fan front / mid / rear around the thorax (a wide
    // spread of z anchors, not bunched at the front) and the feet plant far
    // out to the sides so a heavy brood-mother grounds her mass, not a
    // dangling bug.
    let mut legs = Vec::new();
    for side in [-1.0f32, 1.0] {
        for kk in 0..3 {
            // front(+0.6) / mid(0.0) / rear(-0.7) anchors: a wide fan around
            // the thorax so the six legs brace the body from three stations.
            let zc = 0.6 * s - kk as f32 * 0.65 * s;
            // splay the front pair forward and the rear pair back at the foot
            // so the stance reads spread front-to-back, not a tight bunch.
            let foot_z = zc + (1.0 - kk as f32) * 0.45 * s;
            let root = r.joint(Some(thorax), "coxa", v(side * 0.42 * s, 0.9 * s, zc));
            let knee = r.joint(
                Some(root),
                "knee",
                v(side * 1.05 * s, 1.15 * s, zc + 0.12 * s),
            );
            let foot = r.joint(Some(knee), "foot", v(side * 1.6 * s, 0.0, foot_z));
            // thicker segments = sturdier bracing limbs, not thin sticks.
            r.cone(root, knee, 0.17 * s, 0.13 * s, 2, 0.05 * s);
            r.cone(knee, foot, 0.13 * s, 0.06 * s, 2, 0.05 * s);
            legs.push(vec![root, knee, foot]);
        }
    }

    let gait = GaitDesc {
        legs,
        spine: vec![head, thorax, abdomen, abtip],
        wings: Vec::new(),
        tail: Vec::new(),
        head: Some(head),
        style: Gait::Crawl,
    };
    let mut rig = r.finish(gait);

    add_swarm_head_features(&mut rig, head, s, eyes_n);
    add_carapace_spikes(&mut rig, thorax, abdomen, s, spikes);

    let nsacs = (5.0 + 3.0 * menace).round().clamp(5.0, 8.0) as usize;
    add_brood_sacs(
        &mut rig,
        abdomen,
        abtip,
        s,
        nsacs,
        &mut parts,
        &mut weak_points,
        p.weak_points,
    );

    rig.bounds = crate::generators::monster::rig::compute_bounds(&rig);

    BossRig {
        rig,
        weak_points,
        parts,
        extra_mesh: None,
    }
}

/// A fanged, many-eyed head: two curved mandibles jutting forward-down
/// (rank-3 `Horn` family) and a cluster of `eyes_n` small glowing eyes
/// (rank-4 `Eye`, each its own family) staggered across the face — the
/// "compound eye cluster" read of a huge insectoid.
fn add_swarm_head_features(rig: &mut MonsterRig, head: usize, s: f32, eyes_n: usize) {
    let v = Vec3::new;
    let hp = rig.joint_world(head);
    for side in [-1.0f32, 1.0] {
        let mb = add_joint(
            rig,
            head,
            "mandible",
            hp + v(side * 0.14 * s, -0.06 * s, 0.22 * s),
        );
        let mt = add_joint(
            rig,
            mb,
            "mandible_tip",
            hp + v(side * 0.26 * s, -0.16 * s, 0.44 * s),
        );
        push_cone(
            rig,
            mb,
            mt,
            0.05 * s,
            0.012 * s,
            3,
            0.02 * s,
            PrimTint::Horn,
        );
    }
    let n = eyes_n.max(2);
    for i in 0..n {
        let f = (i as f32 / (n - 1).max(1) as f32 - 0.5) * 2.0; // -1..1 across the face
        let row = (i % 2) as f32; // stagger two rows for a compound-eye read
        let eb = add_joint(
            rig,
            head,
            "eye",
            hp + v(
                f * 0.2 * s,
                0.05 * s - row * 0.09 * s,
                0.24 * s + row * 0.02 * s,
            ),
        );
        push_flat(
            rig,
            eb,
            eb,
            v(0.03 * s, 0.03 * s, 0.03 * s),
            4,
            0.008 * s,
            PrimTint::Eye,
        );
    }
}

/// A crest of bony carapace spikes running along the thorax->abdomen
/// dorsal line — `spikes` (0..1) scales count/height. Own rank-3 `Horn`
/// family per spike, children of `thorax`.
fn add_carapace_spikes(rig: &mut MonsterRig, thorax: usize, abdomen: usize, s: f32, spikes: f32) {
    let v = Vec3::new;
    let n = (3.0 + 4.0 * spikes).round().max(2.0) as usize;
    let tp = rig.joint_world(thorax);
    let ap = rig.joint_world(abdomen);
    for i in 0..n {
        let f = i as f32 / (n - 1).max(1) as f32;
        let base_pos = tp.lerp(ap, f) + v(0.0, (0.4 - 0.12 * f) * s, 0.0);
        let h = (0.14 + 0.14 * spikes) * s * (1.0 - 0.3 * f);
        let b0 = add_joint(rig, thorax, "carapace_spike", base_pos);
        let b1 = add_joint(
            rig,
            b0,
            "carapace_spike_tip",
            base_pos + v(0.0, h, -0.05 * s),
        );
        push_cone(
            rig,
            b0,
            b1,
            0.05 * s,
            0.008 * s,
            3,
            0.015 * s,
            PrimTint::Horn,
        );
    }
}

/// The money detail: `n` big glowing brood sacs STUDDED across the whole
/// bulbous abdomen — a bloated egg-sac cluster. Sacs are distributed ALONG
/// the abdomen's long axis (front girth -> rear tip) and spiralled around
/// its girth (top / sides / rear, spread by the golden angle so no two
/// cluster) so several pods read from every viewing angle. Each pod is a
/// sizable `Eye`-tinted ellipsoid whose center sits just OUTSIDE the
/// carapace surface so it visibly PROTRUDES/bulges (not a flat embedded
/// speckle), with a tiny blend `k` so it stays a distinct pustule. Each sac
/// is a `sac_base -> brood_sac.i` joint pair added fresh (never sharing a
/// joint with any other sac), fold-ranked LAST (rank 5), so `skin_body`'s
/// union-find gives every sac its OWN family — they glow, pop, and stay
/// web-free = the weak points. Named `brood_sac.i` destructible parts +
/// `weak_point.brood_sac.i` weak points (when enabled). The abdomen itself
/// stays dark (`Body` tint) so the sacs pop.
#[allow(clippy::too_many_arguments)]
fn add_brood_sacs(
    rig: &mut MonsterRig,
    abdomen: usize,
    abtip: usize,
    s: f32,
    n: usize,
    parts: &mut Vec<PartMeta>,
    weak_points: &mut Vec<WeakPointMeta>,
    emit_weak_points: bool,
) {
    let v = Vec3::new;
    let ap = rig.joint_world(abdomen);
    let tp = rig.joint_world(abtip);
    // long axis of the abdomen mass + an orthonormal girth basis around it.
    let axis = (tp - ap).normalize_or_zero();
    let side = axis.cross(Vec3::Y).normalize_or_zero();
    let up = side.cross(axis).normalize_or_zero();
    let golden = 2.399_963_2; // golden angle (rad): even, non-clustering spiral
    // Sample the CURRENT composed field (abdomen/thorax/legs/head, no sacs yet)
    // so each pod is planted on the ACTUAL blended carapace surface rather than
    // a hand-estimated radius that the big smooth-min abdomen would swallow.
    let field = crate::generators::monster::body::organic_field(rig);
    for i in 0..n {
        // march front->rear along the abdomen (t: 0.12..0.82 keeps sacs on the
        // fat body, off the thorax junction and off the very tail point).
        let t = 0.12 + 0.7 * (i as f32 / (n - 1).max(1) as f32);
        let anchor = ap.lerp(tp, t);
        // spiral the girth angle so sacs stud top / sides / rear evenly.
        let theta = i as f32 * golden;
        let dir = (side * theta.cos() + up * theta.sin()).normalize_or_zero();
        // ray-march outward from the (interior) anchor along `dir` to find the
        // carapace surface (field crosses 0), so the pod plants exactly on the
        // real bulging body wherever the girth actually is.
        let mut d_surf = 0.1 * s;
        let step = 0.03 * s;
        while d_surf < 2.5 * s && field(anchor + dir * d_surf) < 0.0 {
            d_surf += step;
        }
        // pod size tapers slightly toward the narrower rear.
        let r1 = (0.22 - 0.06 * t) * s;
        // base sunk just INSIDE the surface (so it fuses, no floating gap);
        // the ellipsoid CENTER lands ~half a radius OUTSIDE the surface so the
        // pod clearly PROTRUDES as a bulging pustule, not an embedded speckle.
        let base_pos = anchor + dir * (d_surf - 0.35 * r1);
        let tip_pos = anchor + dir * (d_surf + 1.25 * r1);
        let name = format!("brood_sac.{}", i + 1);
        let base = add_joint(rig, abdomen, "sac_base", base_pos);
        let tip = add_joint(rig, base, &name, tip_pos);
        push_flat(
            rig,
            base,
            tip,
            v(r1, r1, r1 * 1.15),
            5,
            0.012 * s,
            PrimTint::Eye,
        );
        parts.push(PartMeta {
            name: name.clone(),
            joint: name.clone(),
            destructible: true,
        });
        if emit_weak_points {
            weak_points.push(WeakPointMeta {
                name: format!("weak_point.brood_sac.{}", i + 1),
                joint: name,
                collider: ColliderJson::Sphere { radius: r1 * 1.3 },
                offset: [0.0, 0.0, 0.0],
                destructible: true,
                phase: 2,
            });
        }
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
        let p = boss(r#"{"kind":"boss","archetype":"colossus","element":"volcanic"}"#);
        let br = build_boss_rig(&p);
        let pal = crate::palette::by_name("volcanic");
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

    #[test]
    fn lich_rig_is_wellformed() {
        let br = build_boss_rig(&boss(r#"{"kind":"boss","archetype":"lich"}"#));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(br.rig.prims.iter().any(|d| d.fold_rank == 0), "has a core");
        assert!(
            br.rig.skeleton.joints.iter().any(|j| j.name == "core"),
            "has a core joint"
        );
        assert!(br.parts.iter().any(|p| p.name == "head"), "has head part");
        assert!(
            br.parts.iter().any(|p| p.name == "throne"),
            "has a throne part"
        );
        assert!(
            br.rig.skeleton.joints.iter().any(|j| j.name == "throne"),
            "has a throne joint"
        );
        assert!(
            br.parts
                .iter()
                .filter(|p| p.name.starts_with("implement."))
                .count()
                >= 2,
            "has floating implement parts"
        );
        assert!(
            br.weak_points
                .iter()
                .any(|w| w.name == "weak_point.phylactery" && w.joint == "core" && w.destructible)
        );
        assert!(
            br.extra_mesh.is_some(),
            "throne is a merged CSG mesh, not an SDF primitive"
        );
        let throne = br.extra_mesh.as_ref().unwrap();
        assert!(
            throne.triangle_count() > 12,
            "throne carve adds geometry: {}",
            throne.triangle_count()
        );
        throne.validate().expect("throne mesh valid");
    }

    #[test]
    fn lich_hostile_input_cannot_panic() {
        let br = build_boss_rig(&boss(
            r#"{"kind":"boss","archetype":"lich","size":1e30,"phases":999,"eyes":999999,"crown":1e30,"regalia":-1e30}"#,
        ));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(n < 2000, "joint count bounded: {n}");
    }

    #[test]
    fn lich_skinning_has_no_webs() {
        let p = boss(r#"{"kind":"boss","archetype":"lich","element":"necrotic"}"#);
        let br = build_boss_rig(&p);
        let pal = crate::palette::by_name("necrotic");
        let mut mesh = super::super::body::build_body(&br.rig, p.size, p.detail, p.seed, 0.0, &pal);
        if let Some(extra) = &br.extra_mesh {
            mesh.merge(extra);
        }
        mesh.validate()
            .expect("bind mesh valid (no degenerate/zero-area tris)");
        crate::generators::monster::skin_body(&mut mesh, &br.rig);
        assert!(mesh.is_skinned(), "boss mesh must be skinned");

        // synthetic clip: swing both arms, bend one leg, and spin the
        // implement pivots — the junctions this probe targets.
        let skel = &br.rig.skeleton;
        let mut channels = Vec::new();
        for (i, j) in skel.joints.iter().enumerate() {
            if j.name.contains("upperarm") || j.name.contains("forearm") || j.name == "thigh" {
                channels.push(crate::gltf::Channel {
                    joint: i,
                    times: vec![0.0, 1.0],
                    data: crate::gltf::ChannelData::Rotation(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_x(0.6),
                    ]),
                });
            }
            if j.name == "implement_pivot" {
                channels.push(crate::gltf::Channel {
                    joint: i,
                    times: vec![0.0, 1.0],
                    data: crate::gltf::ChannelData::Rotation(vec![
                        Quat::IDENTITY,
                        Quat::from_rotation_y(1.2),
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
        assert!(moved, "limb/implement pose should deform the mesh");

        let stretch = max_edge_stretch(&mesh, &posed);
        assert!(
            stretch < 8.0,
            "edge stretch {stretch} indicates a skinning web between limbs/implements/torso"
        );

        // the throne must stay put (rigid, unanimated family): its BACKREST
        // vertices should not move under a pose that only targets
        // arms/legs/implements. Sample the region well behind the caster
        // (z < -0.42*size) — the throne backrest lives there and NO animated
        // lich part (arms/legs/orbiting implements all sit at z > -0.4*size)
        // reaches it, so any motion there is a throne skinning web.
        let sz = p.size.clamp(0.4, 8.0);
        let mut throne_moved = 0usize;
        let mut throne_checked = 0usize;
        for (a, b) in mesh.positions.iter().zip(&posed.positions) {
            if a.z < -0.42 * sz {
                throne_checked += 1;
                if a.distance(*b) > 0.01 {
                    throne_moved += 1;
                }
            }
        }
        assert!(
            throne_checked > 0,
            "probe should sample throne backrest vertices"
        );
        assert_eq!(
            throne_moved, 0,
            "throne must stay rigid/static under a pose"
        );
    }

    #[test]
    fn swarm_queen_rig_is_wellformed() {
        let br = build_boss_rig(&boss(r#"{"kind":"boss","archetype":"swarm_queen"}"#));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(br.rig.prims.iter().any(|d| d.fold_rank == 0), "has a core");
        assert!(
            br.rig.skeleton.joints.iter().any(|j| j.name == "abdomen"),
            "has an abdomen joint"
        );
        assert!(
            br.parts
                .iter()
                .any(|p| p.name == "abdomen" && p.destructible),
            "has destructible abdomen part"
        );
        assert!(br.parts.iter().any(|p| p.name == "head"), "has head part");
        assert!(
            br.parts
                .iter()
                .filter(|p| p.name.starts_with("brood_sac."))
                .count()
                >= 3,
            "has at least 3 brood_sac parts"
        );
        assert!(
            br.weak_points
                .iter()
                .filter(|w| w.name.starts_with("weak_point.brood_sac.") && w.destructible)
                .count()
                >= 3,
            "has at least 3 brood_sac weak points"
        );
    }

    #[test]
    fn swarm_queen_hostile_input_cannot_panic() {
        let br = build_boss_rig(&boss(
            r#"{"kind":"boss","archetype":"swarm_queen","size":1e30,"phases":999,"eyes":999999,"spikes":1e30,"menace":1e30}"#,
        ));
        let n = br.rig.skeleton.joints.len();
        assert!(br.rig.prims.iter().all(|d| d.joint_a < n && d.joint_b < n));
        assert!(n < 2000, "joint count bounded: {n}");
    }

    #[test]
    fn swarm_queen_skinning_has_no_webs() {
        let p = boss(r#"{"kind":"boss","archetype":"swarm_queen","element":"fungal"}"#);
        let br = build_boss_rig(&p);
        let pal = crate::palette::by_name("fungal");
        let mut mesh = super::super::body::build_body(&br.rig, p.size, p.detail, p.seed, 0.0, &pal);
        mesh.validate()
            .expect("bind mesh valid (no degenerate/zero-area tris)");
        crate::generators::monster::skin_body(&mut mesh, &br.rig);
        assert!(mesh.is_skinned(), "boss mesh must be skinned");

        // synthetic clip: swing the legs (knee bend) so the many-leg junction
        // this probe targets actually deforms relative to the thorax.
        let skel = &br.rig.skeleton;
        let mut channels = Vec::new();
        for (i, j) in skel.joints.iter().enumerate() {
            if j.name == "coxa" || j.name == "knee" {
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
        assert!(moved, "leg bend pose should deform the mesh");

        let stretch = max_edge_stretch(&mesh, &posed);
        assert!(
            stretch < 8.0,
            "edge stretch {stretch} indicates a skinning web between legs/thorax/brood sacs"
        );
    }
}
