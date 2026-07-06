//! Monster generator — a generalization of the character body pipeline past
//! the fixed humanoid: a data-driven [`rig::MonsterRig`] (joints +
//! fold-order-ranked SDF primitives + gait descriptor) fed to one shared
//! organic pass (smooth-min compose -> surface-net mesh -> family-restricted
//! skin -> procedural clips).

mod body;
mod rig;

use crate::gltf::{Asset, Material, Part};
use crate::palette::Palette;
use crate::recipe::MonsterParams;

pub fn generate(p: &MonsterParams, pal: &Palette) -> Asset {
    let r = rig::build_rig(p);
    let mesh = body::build_body(&r, p, pal);
    let phys = body::fit_collider(&r, p);
    Asset::static_mesh(
        "monster",
        vec![Part {
            mesh,
            material: Material {
                roughness: 0.75,
                ..Default::default()
            },
        }],
        Some(phys),
    )
}
