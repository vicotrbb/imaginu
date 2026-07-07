//! Boss generator — a composite, multi-part, multi-phase encounter creature
//! built by escalating the monster rig/body/skin/anim pipeline. See
//! docs/superpowers/specs/2026-07-06-phase7-bosses-design.md.

use crate::generators::monster::body;
use crate::gltf::{Asset, Material, Part};
use crate::palette::Palette;
use crate::recipe::BossParams;

mod anim;
pub mod meta;
mod preset;
mod rig;

use meta::BossMeta;

/// Approximate collider shape per archetype: the colossus is a stout biped,
/// so it fits `BipedBrute` (a tighter capsule); the rest still fall back to
/// `Serpent` (a low sprawling capsule) until Tasks 8-10 pick their own plans.
fn collider_plan(a: crate::recipe::BossArchetype) -> crate::recipe::BodyPlan {
    match a {
        crate::recipe::BossArchetype::Colossus | crate::recipe::BossArchetype::Lich => {
            crate::recipe::BodyPlan::BipedBrute
        }
        _ => crate::recipe::BodyPlan::Serpent,
    }
}

/// Real boss pipeline: archetype preset -> rig plan -> shared organic
/// body/skin pass -> collider fit -> (clips in Task 6) -> `BossMeta`
/// assembly (weak points, destructible parts, arena sizing).
pub fn generate(p: &BossParams, pal: &Palette) -> Asset {
    let mut owned = p.clone();
    preset::apply_archetype_preset(&mut owned);
    let p = &owned;

    let br = rig::build_boss_rig(p);
    // Eyes must glow regardless of the emissive knob: floor the emission
    // whenever the rig carries eye primitives (same rule as monster generate),
    // so the default sentinel emissive still lights up an infernal hydra.
    // A LOW floor (0.12) is deliberate: this value both (a) sets the fraction
    // of body vertices painted full accent (`build_body`) and (b) scales the
    // flat material emissive the renderer adds everywhere. A high floor washes
    // the whole body bright accent and flattens its shading (candy/coral); a
    // low floor keeps the body dark with sparse glowing crack accents and lets
    // the rounded form read — dark scales + glowing lava eyes, high value
    // contrast (matching how fire_wyrm reads dark-with-glow).
    let eye_glow = br
        .rig
        .prims
        .iter()
        .any(|d| d.tint == crate::generators::monster::rig::PrimTint::Eye);
    let e = p.emissive.clamp(0.0, 1.0);
    // `emissive` here drives BOTH (a) the fraction of BODY vertices painted
    // full accent albedo in `build_body` (`body_accent`) and (b) the UNIFORM
    // material emissive the renderer adds to every surface. For most bosses
    // these are one value. The colossus DECOUPLES them: a dark stone golem
    // must read dark with the glow CONCENTRATED at the molten core (an
    // Eye-tinted prim that always paints full accent regardless of these
    // knobs) and in sparse crevice cracks — NOT measled bright accent all
    // over (the green-on-green / washed-out failure). So its body gets only a
    // few percent accent speckle and a LOW uniform emissive floor that lets
    // the core+eye albedo pop without washing the rock.
    // The lich is the OTHER gallery hero and is necrotic-only (never
    // element-switched), so it hits the exact same green-on-green
    // full-bright failure the colossus did: it gets the SAME dark-base
    // treatment — a near-black robed body with the glow concentrated at the
    // phylactery core, the eyes, the crown, and the floating implements
    // (all pushed as Eye/Horn-tinted prims that paint full accent regardless
    // of these knobs), NOT measled bright-green speckle over the whole robe.
    let colossus = matches!(p.archetype, crate::recipe::BossArchetype::Colossus);
    let lich = matches!(p.archetype, crate::recipe::BossArchetype::Lich);
    let (body_accent, emissive) = if colossus {
        (
            (e * 0.12).clamp(0.03, 0.07),
            if eye_glow { 0.11 } else { 0.0 },
        )
    } else if lich {
        // Unlike the colossus's brown body / orange glow (different hue
        // families, so a uniform low emissive floor still reads "brown"),
        // the lich's body and glow are BOTH green — any uniform
        // whole-surface emissive additive reads as "the whole robe is
        // glowing", not "a dark robe with a few bright accents". So the
        // uniform floor here is kept almost off; the phylactery/eyes/crown
        // circlet/implements already paint FULL accent unconditionally via
        // `PrimTint::Eye`/`Horn` (see `plan_lich`), so the "glow" pop comes
        // from ALBEDO CONTRAST against the (separately darkened, see
        // `body_pal` below) near-black robe, not from an emissive wash.
        (
            (e * 0.04).clamp(0.008, 0.02),
            if eye_glow { 0.02 } else { 0.0 },
        )
    } else {
        (e, e.max(if eye_glow { 0.12 } else { 0.0 }))
    };
    // Necrotic green reads visually "loud"/glowing even at a moderate raw
    // value (it carries far more perceived luminance than an equally-dark
    // stone brown, which is why the colossus's brown-on-orange decoupling
    // above wasn't enough here on its own — measured renders still washed
    // bright green). So the lich additionally darkens the BASE body
    // palette fed to `build_body` (terrain/foliage/trunk/rock, i.e.
    // everything but `accent`) before meshing, so the dark robe reads
    // genuinely near-black instead of "toxic green wash" — the glow stays
    // exactly as bright since `accent` (the phylactery/eyes/crown/implement
    // color) is untouched.
    let body_pal = if lich {
        let mut dp = *pal;
        let dim = 0.2;
        for t in &mut dp.terrain {
            *t *= dim;
        }
        for f in &mut dp.foliage {
            *f *= dim;
        }
        dp.trunk *= dim;
        for r in &mut dp.rock {
            *r *= dim;
        }
        dp
    } else {
        *pal
    };
    let mut mesh = body::build_body(&br.rig, p.size, p.detail, p.seed, body_accent, &body_pal);
    // The lich's throne is a literal CSG-carved mesh, not an SDF primitive
    // (see `rig::build_throne_mesh`); merge it into the body mesh BEFORE
    // skinning so `skin_body`'s nearest-primitive classifier binds its
    // vertices to the throne's own rank-7 anchor family (rigid, static).
    if let Some(extra) = &br.extra_mesh {
        mesh.merge(extra);
    }
    crate::generators::monster::skin_body(&mut mesh, &br.rig);
    mesh.validate().expect("boss mesh invalid");
    // Whole-body collider reuses the monster fit; boss body plan approximated
    // by the closest monster BodyPlan for the collider shape. Serpent (a
    // capsule along the long axis) is the right approximation for the hydra's
    // low sprawling torso + reared necks; the colossus is a biped, so it maps
    // to BipedBrute. Tasks 8-10 pick plans for the remaining archetypes.
    let plan = collider_plan(p.archetype);
    let phys = body::fit_collider(&br.rig, p.size, plan);

    let animations = if p.animate {
        anim::build_boss_clips(&br.rig, p)
    } else {
        Vec::new()
    };

    let mut bm = BossMeta::new(
        format!("{:?}", p.archetype).to_lowercase(),
        format!("{:?}", p.element).to_lowercase(),
    );
    bm.weak_points = br.weak_points;
    bm.parts = br.parts;
    bm.arena.recommended_radius = (p.size * 2.7).max(4.0);
    bm.phases = anim::build_phase_meta(p, &bm.weak_points);

    Asset {
        name: "boss".into(),
        parts: vec![Part {
            mesh,
            material: Material {
                roughness: 0.7,
                emissive: pal.accent * emissive * 0.6,
                ..Default::default()
            },
        }],
        skeleton: Some(br.rig.skeleton),
        animations,
        physics: Some(phys),
        boss: Some(bm),
        lods: Vec::new(),
        instanced: Vec::new(),
    }
}
