//! Dungeon generator — a themed, navigable underground layout. Mirrors the
//! `world` pattern: a pure-of-seed `DungeonModel` (rooms/corridors/doors/spawns,
//! added in D2) drives a geometry pass (D3), emitting a single GLB for a
//! one-room dungeon or a directory + `manifest.json` for multi-room (D4).

use glam::Vec3;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, cuboid};
use crate::palette::Palette;
use crate::recipe::DungeonParams;

/// Single-GLB build path (used for small/1-room dungeons; multi-room writes a
/// manifest directory via the `dungeon` subcommand). Returns `Result` because
/// palette/layout validation and (for the directory form) IO can fail.
pub fn generate(p: &DungeonParams, pal: &Palette) -> Result<Asset, String> {
    // TEMP stub (replaced in D2-D5): one themed room box (floor + 4 walls +
    // ceiling) with a trimesh collider, so recipe wiring + validate pass.
    Ok(room_stub(p, pal))
}

fn room_stub(_p: &DungeonParams, pal: &Palette) -> Asset {
    let floor_col = pal.terrain[1];
    let wall_col = pal.terrain[3];
    let (w, d, h) = (6.0f32, 6.0f32, 3.0f32);
    let t = 0.4f32; // slab/wall thickness
    let mut m = Mesh::new();
    // floor
    m.merge(&cuboid(
        Vec3::new(0.0, t * 0.5, 0.0),
        Vec3::new(w * 0.5, t * 0.5, d * 0.5),
        floor_col,
    ));
    // ceiling
    m.merge(&cuboid(
        Vec3::new(0.0, h - t * 0.5, 0.0),
        Vec3::new(w * 0.5, t * 0.5, d * 0.5),
        wall_col,
    ));
    // 4 walls
    for (cx, cz, hx, hz) in [
        (0.0, -d * 0.5, w * 0.5, t * 0.5),
        (0.0, d * 0.5, w * 0.5, t * 0.5),
        (-w * 0.5, 0.0, t * 0.5, d * 0.5),
        (w * 0.5, 0.0, t * 0.5, d * 0.5),
    ] {
        m.merge(&cuboid(
            Vec3::new(cx, h * 0.5, cz),
            Vec3::new(hx, h * 0.5, hz),
            wall_col,
        ));
    }
    Asset::static_mesh(
        "dungeon",
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.9,
                ..Default::default()
            },
        }],
        Some(Physics {
            collider: Collider::TriMesh,
            mass: 0.0,
            friction: 0.9,
            restitution: 0.0,
        }),
    )
}
