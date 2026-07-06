//! Dungeon generator — a themed, navigable underground layout. Mirrors the
//! `world` pattern: a pure-of-seed `DungeonModel` (rooms/corridors/doors/
//! spawns) drives a geometry pass (rooms/corridors + CSG doorways) and a
//! dressing pass (props + emissive torches). `generate` emits ONE merged GLB
//! (the render / small-dungeon path); `manifest::write_dir` emits a per-room
//! directory + `manifest.json` (the streaming path, wired by the CLI later).

mod dress;
mod geom;
mod layout;
pub mod manifest;
mod model;

use glam::Vec3;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::Mesh;
use crate::palette::Palette;
use crate::recipe::DungeonParams;

use model::{Door, DungeonModel, Room};

/// Emissive torch glow (matches `dress::TORCH_GLOW`).
const TORCH_GLOW: Vec3 = Vec3::new(1.0, 0.55, 0.18);

/// Single-GLB build path: the whole dungeon (rooms + corridors + carved
/// doorways + dressing) merged into one asset with a trimesh collider.
pub fn generate(p: &DungeonParams, pal: &Palette) -> Result<Asset, String> {
    let model = DungeonModel::new(p, pal)?;
    Ok(merged_asset(&model))
}

/// Build the two prop meshes for a room: (opaque, emissive). Emissive props
/// (torches) go into a separate part so they render as lighting cues.
fn dress_meshes(model: &DungeonModel, room: &Room) -> (Mesh, Mesh) {
    let (mut opaque, mut emissive) = (Mesh::new(), Mesh::new());
    for pp in dress::dress_room(
        room,
        model.p.theme,
        model.p.density,
        model.p.seed,
        &model.pal,
    ) {
        match pp.emissive {
            Some(_) => emissive.merge(&pp.mesh),
            None => opaque.merge(&pp.mesh),
        }
    }
    (opaque, emissive)
}

/// A single room's carved shell (used by both the merged asset and the
/// per-room manifest output).
fn carved_room(model: &DungeonModel, room: &Room) -> Mesh {
    let walls = geom::room_mesh(room, model.p.theme, &model.pal, model.p.detail);
    let doors: Vec<Door> = model
        .doors
        .iter()
        .filter(|d| d.room == room.id)
        .cloned()
        .collect();
    geom::carve_doorways(walls, &doors)
}

fn assemble(opaque: Mesh, emissive: Mesh) -> Asset {
    let mut parts = vec![Part {
        mesh: opaque,
        material: Material {
            roughness: 0.92,
            ..Default::default()
        },
    }];
    if !emissive.positions.is_empty() {
        parts.push(Part {
            mesh: emissive,
            material: Material {
                roughness: 0.4,
                emissive: TORCH_GLOW * 2.4,
                ..Default::default()
            },
        });
    }
    Asset::static_mesh(
        "dungeon",
        parts,
        Some(Physics {
            collider: Collider::TriMesh,
            mass: 0.0,
            friction: 0.9,
            restitution: 0.0,
        }),
    )
}

fn merged_asset(model: &DungeonModel) -> Asset {
    let mut opaque = Mesh::new();
    let mut emissive = Mesh::new();
    for room in &model.rooms {
        opaque.merge(&carved_room(model, room));
        let (o, e) = dress_meshes(model, room);
        opaque.merge(&o);
        emissive.merge(&e);
    }
    for c in &model.corridors {
        opaque.merge(&geom::corridor_mesh(
            c,
            model.p.theme,
            &model.pal,
            model.p.detail,
        ));
    }
    assemble(opaque, emissive)
}

/// Self-contained asset for one room (its carved shell + dressing + the
/// corridors it owns), for the per-room directory output.
pub(crate) fn room_asset(model: &DungeonModel, room: &Room) -> Asset {
    let mut opaque = carved_room(model, room);
    // own the corridors whose lower-id endpoint is this room, so each corridor
    // is written exactly once across the room set
    for c in &model.corridors {
        if c.a.min(c.b) == room.id {
            opaque.merge(&geom::corridor_mesh(
                c,
                model.p.theme,
                &model.pal,
                model.p.detail,
            ));
        }
    }
    let (o, e) = dress_meshes(model, room);
    opaque.merge(&o);
    assemble(opaque, e)
}
