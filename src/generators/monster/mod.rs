//! Monster generator — a generalization of the character body pipeline past
//! the fixed humanoid: a data-driven [`rig::MonsterRig`] (joints +
//! fold-order-ranked SDF primitives + gait descriptor) fed to one shared
//! organic pass (smooth-min compose -> surface-net mesh -> family-restricted
//! skin -> procedural clips).

use glam::Vec3;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::palette::Palette;
use crate::recipe::MonsterParams;
use crate::sdf;

pub fn generate(p: &MonsterParams, pal: &Palette) -> Asset {
    // TEMP stub (replaced in M2-M8): a single ellipsoid body with a capsule
    // collider so recipe wiring + `Asset::validate` pass end-to-end.
    body_stub(p, pal)
}

fn body_stub(p: &MonsterParams, pal: &Palette) -> Asset {
    let s = p.size.clamp(0.2, 4.0);
    let detail = p.detail.clamp(0.5, 2.0);
    let r = Vec3::new(0.8 * s, 0.6 * s, 1.1 * s);
    let c = Vec3::new(0.0, 0.6 * s, 0.0);
    let pad = Vec3::splat(0.2 * s);
    let body = pal.terrain[2];
    let mesh = sdf::mesh_field(
        c - r - pad,
        c + r + pad,
        (0.08 * s / detail).max(0.02),
        &|q| sdf::sd_ellipsoid(q, c, r),
        &|_| body,
    );
    Asset::static_mesh(
        "monster",
        vec![Part {
            mesh,
            material: Material {
                roughness: 0.7,
                ..Default::default()
            },
        }],
        Some(Physics {
            collider: Collider::Capsule {
                radius: r.x,
                height: r.y * 2.0,
            },
            mass: 60.0 * s * s * s,
            friction: 0.6,
            restitution: 0.2,
        }),
    )
}
