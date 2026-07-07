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
        crate::recipe::BossArchetype::SwarmQueen => crate::recipe::BodyPlan::Insectoid,
        crate::recipe::BossArchetype::DragonLord | crate::recipe::BossArchetype::Hydra => {
            crate::recipe::BodyPlan::Serpent
        }
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
    // The swarm-queen is fungal-element (a brood-mother's palette) and hits
    // the same over-saturation failure necrotic did on the lich: fungal's
    // teal/green accent reads loud even at a moderate value, so a uniform
    // whole-body emissive wash would flatten the huge carapace into a green
    // glow blob instead of a dark insectoid with bright brood sacs. Same
    // dark-base treatment: near-black carapace, glow concentrated at the
    // brood sacs + eyes (`PrimTint::Eye`, already unconditional full accent).
    let swarm_queen = matches!(p.archetype, crate::recipe::BossArchetype::SwarmQueen);
    // The dragon-lord's arctic palette hits the same same-hue problem as the
    // lich's necrotic green: the body (pale icy blues) and the accent glow
    // (a bright cyan) share a hue family, so a uniform emissive wash reads as
    // "the whole dragon is glowing pale blue" and washes out flat, instead of
    // "a dark-scaled dragon with a glowing frost heart/eyes". Same dark-base
    // treatment as the lich/swarm-queen: glow concentrated at the heart +
    // eyes (`PrimTint::Eye`, already unconditional full accent), body base
    // darkened (see `body_pal`) so those glow points POP against contrast.
    let dragon_lord = matches!(p.archetype, crate::recipe::BossArchetype::DragonLord);
    // The hydra is the gallery's marquee boss and is infernal-only (never
    // element-switched), which hits the exact same same-hue problem as the
    // dragon-lord's icy blues: the infernal base palette's warm oranges/reds
    // and the orange accent glow share a hue family, so the ORIGINAL
    // undecoupled `(e, e.max(...))` fallback below painted roughly half the
    // body vertices full bright accent — a uniform candy-orange hydra instead
    // of a dark charcoal-basalt beast with glowing lava eyes/cracks. Same
    // dark-base treatment as the other three gallery bosses: glow
    // concentrated at the eyes (`PrimTint::Eye`, already unconditional full
    // accent) and sparse crack speckle, body base darkened (see `body_pal`)
    // so those glow points POP against contrast.
    let hydra = matches!(p.archetype, crate::recipe::BossArchetype::Hydra);
    let (body_accent, emissive) = if colossus {
        (
            (e * 0.12).clamp(0.03, 0.07),
            if eye_glow { 0.11 } else { 0.0 },
        )
    } else if lich || dragon_lord {
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
    } else if swarm_queen || hydra {
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
    let body_pal = if lich || swarm_queen || dragon_lord || hydra {
        let mut dp = *pal;
        // The dragon-lord gets a MILDER dim than the lich/swarm-queen/hydra's
        // near-black 0.2: the brief calls for a darker SLATE-BLUE body (icy
        // scales, not a void), just dark enough that the glowing frost heart
        // and eyes clearly pop. The hydra keeps the near-black 0.2 dim: it
        // needs a dark charcoal-basalt body (not a mere dim orange) so the
        // glowing lava eyes/cracks read as clearly separate from the base.
        let dim = if dragon_lord { 0.3 } else { 0.2 };
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

    // Serialize the archetype/element via serde so the metadata strings match
    // the recipe's snake_case keys exactly (e.g. "swarm_queen", not the
    // Debug-derived "SwarmQueen"/"swarmqueen") for clean round-trips.
    let mut bm = BossMeta::new(
        serde_json::to_value(p.archetype)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        serde_json::to_value(p.element)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
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
