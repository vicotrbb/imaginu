//! Dungeon generator — a themed, navigable underground layout. Mirrors the
//! `world` pattern: a pure-of-seed `DungeonModel` (rooms/corridors/doors/
//! spawns) drives a geometry pass (rooms/corridors + CSG doorways) and a
//! dressing pass (props + emissive torches). `generate` emits ONE merged GLB
//! (the render / small-dungeon path); `manifest::write_dir` emits a per-room
//! directory + `manifest.json` (the streaming path, wired by the CLI later).

mod cavern;
mod dress;
mod geom;
mod layout;
pub mod manifest;
pub mod model;

use glam::Vec3;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::Mesh;
use crate::palette::Palette;
use crate::recipe::DungeonParams;

use model::{Corridor, Door, DungeonModel, Room};

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
/// per-room manifest output). `include_ceiling` is false for the top-down
/// overview so the interior is visible from above.
fn carved_room(model: &DungeonModel, room: &Room, include_ceiling: bool) -> Mesh {
    let walls = geom::room_mesh(
        room,
        model.p.theme,
        &model.pal,
        model.p.detail,
        include_ceiling,
    );
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

fn build_merged(model: &DungeonModel, include_ceiling: bool) -> Asset {
    let mut opaque = Mesh::new();
    let mut emissive = Mesh::new();
    let cavern = matches!(model.p.theme, crate::recipe::DungeonTheme::Cavern);
    // Cavern renders as one organic SDF void (rooms + corridors fused); the
    // other themes keep the boxy carved shells.
    if cavern {
        opaque.merge(&cavern::cavern_mesh(
            &model.rooms,
            &model.corridors,
            &model.pal,
            model.p.detail,
            include_ceiling,
        ));
    }
    for room in &model.rooms {
        if !cavern {
            opaque.merge(&carved_room(model, room, include_ceiling));
        }
        let (o, e) = dress_meshes(model, room);
        opaque.merge(&o);
        emissive.merge(&e);
    }
    if !cavern {
        for c in &model.corridors {
            opaque.merge(&geom::corridor_mesh(
                c,
                model.p.theme,
                &model.pal,
                model.p.detail,
                include_ceiling,
            ));
        }
    }
    assemble(opaque, emissive)
}

/// The whole dungeon merged into one asset, WITH ceilings (the render /
/// small-dungeon path). Byte-stable — the single-GLB output must not drift.
pub fn merged_asset(model: &DungeonModel) -> Asset {
    build_merged(model, true)
}

/// The whole dungeon merged WITHOUT ceilings, for a near-top-down beauty shot:
/// floor, walls, dressing props and glowing torches read from above. Not part
/// of any streamed output — render-only.
pub fn overview_asset(model: &DungeonModel) -> Asset {
    build_merged(model, false)
}

/// Self-contained asset for one room (its carved shell + dressing + the
/// corridors it owns), for the per-room directory output.
pub(crate) fn room_asset(model: &DungeonModel, room: &Room) -> Asset {
    let cavern = matches!(model.p.theme, crate::recipe::DungeonTheme::Cavern);
    // corridors this room owns (lower-id endpoint), so each is written once.
    let owned: Vec<Corridor> = model
        .corridors
        .iter()
        .filter(|c| c.a.min(c.b) == room.id)
        .cloned()
        .collect();
    let mut opaque = if cavern {
        cavern::cavern_mesh(
            std::slice::from_ref(room),
            &owned,
            &model.pal,
            model.p.detail,
            true,
        )
    } else {
        let mut m = carved_room(model, room, true);
        for c in &owned {
            m.merge(&geom::corridor_mesh(
                c,
                model.p.theme,
                &model.pal,
                model.p.detail,
                true,
            ));
        }
        m
    };
    let (o, e) = dress_meshes(model, room);
    opaque.merge(&o);
    assemble(opaque, e)
}

/// Generate the boss asset for a dungeon's inline `boss` recipe field,
/// uniform-scaled to fit the boss room's footprint (with margin) and
/// translated so it stands centered on the room floor at the boss spawn
/// point (already integer-snapped by the layout — the seam law). Mesh
/// vertices, skeleton joint translations, the physics collider, and the
/// boss metadata's spatial fields (weak-point offsets/colliders, arena
/// radius) all scale by the same factor so everything stays consistent.
pub(crate) fn place_boss(bp: &crate::recipe::BossParams, room: &Room, spawn_pos: Vec3) -> Asset {
    let pal_name = crate::recipe::element_palette(bp.element);
    let pal = crate::palette::by_name(pal_name);
    let mut asset = super::boss::generate(bp, &pal);

    // Current (unscaled) world-space AABB of the boss mesh.
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for part in &asset.parts {
        for p in &part.mesh.positions {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    if !lo.is_finite() || !hi.is_finite() {
        // Degenerate mesh (shouldn't happen — generate() validates non-empty)
        // — leave the asset unplaced rather than divide by zero.
        return asset;
    }
    let extent = hi - lo;
    let room_w = (room.max.x - room.min.x).max(1.0);
    let room_d = (room.max.z - room.min.z).max(1.0);
    // Fit within a margin of the tighter room dimension so the boss never
    // clips the walls, regardless of how the archetype's silhouette is
    // oriented (a sprawling serpent vs. a stout biped).
    const MARGIN: f32 = 0.7;
    let footprint = extent.x.max(extent.z).max(1e-4);
    let scale = (room_w.min(room_d) * MARGIN / footprint).max(0.05);

    let center_xz = Vec3::new((lo.x + hi.x) * 0.5, 0.0, (lo.z + hi.z) * 0.5);
    let base_y = lo.y;
    // Seam law: the boss stands on the integer-meter room floor, centered
    // on the (already integer-snapped) spawn point.
    let target = Vec3::new(spawn_pos.x.round(), room.min.y.round(), spawn_pos.z.round());
    let translate = Vec3::new(
        target.x - center_xz.x * scale,
        target.y - base_y * scale,
        target.z - center_xz.z * scale,
    );

    for part in &mut asset.parts {
        for p in &mut part.mesh.positions {
            *p = *p * scale + translate;
        }
    }
    if let Some(skel) = &mut asset.skeleton {
        for j in &mut skel.joints {
            j.translation = if j.parent.is_none() {
                j.translation * scale + translate
            } else {
                j.translation * scale
            };
        }
    }
    if let Some(phys) = &mut asset.physics {
        match &mut phys.collider {
            Collider::Box { half_extents } => *half_extents *= scale,
            Collider::Sphere { radius } => *radius *= scale,
            Collider::Capsule { radius, height } => {
                *radius *= scale;
                *height *= scale;
            }
            Collider::TriMesh | Collider::Heightfield => {}
        }
    }
    if let Some(bm) = &mut asset.boss {
        bm.arena.recommended_radius *= scale;
        for wp in &mut bm.weak_points {
            for c in wp.offset.iter_mut() {
                *c *= scale;
            }
            use crate::generators::boss::meta::ColliderJson;
            match &mut wp.collider {
                ColliderJson::Sphere { radius } => *radius *= scale,
                ColliderJson::Box { half_extents } => {
                    for h in half_extents.iter_mut() {
                        *h *= scale;
                    }
                }
                ColliderJson::Capsule { radius, height } => {
                    *radius *= scale;
                    *height *= scale;
                }
            }
        }
    }
    asset
}
