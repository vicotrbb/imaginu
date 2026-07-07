//! Boss generator — a composite, multi-part, multi-phase encounter creature
//! built by escalating the monster rig/body/skin/anim pipeline. See
//! docs/superpowers/specs/2026-07-06-phase7-bosses-design.md.

use crate::generators::monster::body;
use crate::gltf::{Asset, Material, Part};
use crate::palette::Palette;
use crate::recipe::BossParams;

pub mod meta;
mod preset;
mod rig;

use meta::BossMeta;

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
    let eye_glow = br
        .rig
        .prims
        .iter()
        .any(|d| d.tint == crate::generators::monster::rig::PrimTint::Eye);
    let emissive = p
        .emissive
        .clamp(0.0, 1.0)
        .max(if eye_glow { 0.3 } else { 0.0 });
    let mut mesh = body::build_body(&br.rig, p.size, p.detail, p.seed, emissive, pal);
    crate::generators::monster::skin_body(&mut mesh, &br.rig);
    mesh.validate().expect("boss mesh invalid");
    // Whole-body collider reuses the monster fit; boss body plan approximated
    // by the closest monster BodyPlan for the collider shape. Serpent (a
    // capsule along the long axis) is the right approximation for the hydra's
    // low sprawling torso + reared necks; Tasks 7-10 pick per-archetype plans.
    let phys = body::fit_collider(&br.rig, p.size, crate::recipe::BodyPlan::Serpent);

    // Clip driver lands in Task 6; no procedural clips yet.
    let animations = Vec::new();

    let mut bm = BossMeta::new(
        format!("{:?}", p.archetype).to_lowercase(),
        format!("{:?}", p.element).to_lowercase(),
    );
    bm.weak_points = br.weak_points;
    bm.parts = br.parts;
    bm.arena.recommended_radius = (p.size * 2.7).max(4.0);
    // phases filled in Task 6 (clip-linked); leave empty for now.

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
