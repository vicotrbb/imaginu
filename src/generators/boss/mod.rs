//! Boss generator — a composite, multi-part, multi-phase encounter creature
//! built by escalating the monster rig/body/skin/anim pipeline. See
//! docs/superpowers/specs/2026-07-06-phase7-bosses-design.md.

use crate::gltf::{Asset, Material, Part};
use crate::palette::Palette;
use crate::recipe::BossParams;

/// STUB (Task 2): returns a trivial single-sphere asset so recipe dispatch is
/// testable before archetype geometry lands (Task 4+). Replaced in Task 4.
pub fn generate(_p: &BossParams, pal: &Palette) -> Asset {
    let mesh = crate::mesh::icosphere(0.5, 1, pal.accent);
    Asset {
        name: "boss".into(),
        parts: vec![Part {
            mesh,
            material: Material {
                emissive: pal.accent * 0.3,
                ..Default::default()
            },
        }],
        skeleton: None,
        animations: Vec::new(),
        physics: None,
        lods: Vec::new(),
        instanced: Vec::new(),
    }
}
