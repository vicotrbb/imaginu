//! Geometry pass: rooms (floor slab + extruded walls + ceiling), CSG-carved
//! doorways, and corridor shells. Doorway cutters are CLOSED solid boxes that
//! poke through the wall on both faces — an open profile would let the BSP
//! subtract silently fail.

use glam::Vec3;

use crate::csg;
use crate::mesh::{Mesh, cuboid};
use crate::palette::{Palette, lerp};
use crate::recipe::DungeonTheme;

use super::model::{Corridor, Door, Room};

/// Slab / wall thickness in meters.
const T: f32 = 0.4;
/// Carved doorway opening (width, height).
const DOOR_W: f32 = 2.2;
const DOOR_H: f32 = 2.6;
/// Corridor interior width.
const CORR_W: f32 = 3.0;

/// Themed wall albedo pulled from the palette.
pub fn wall_color(theme: DungeonTheme, pal: &Palette) -> Vec3 {
    match theme {
        DungeonTheme::Crypt => pal.rock[1],
        DungeonTheme::Cavern => pal.rock[0],
        DungeonTheme::Sewer => lerp(pal.rock[1], pal.foliage[2], 0.35),
        DungeonTheme::Mine => lerp(pal.rock[1], pal.trunk, 0.4),
        DungeonTheme::Temple => lerp(pal.rock[0], pal.terrain[3], 0.35),
        DungeonTheme::Fortress => pal.rock[0],
    }
}

fn floor_color(pal: &Palette) -> Vec3 {
    pal.terrain[1]
}

/// A single room's solid shell: floor slab, four extruded walls, and (when
/// `include_ceiling`) a ceiling slab. The ceiling-less variant powers the
/// top-down overview render where the interior must be visible from above.
pub fn room_mesh(
    room: &Room,
    theme: DungeonTheme,
    pal: &Palette,
    detail: f32,
    include_ceiling: bool,
) -> Mesh {
    let _ = detail; // boxes need no tessellation; kept for interface parity
    let wall = wall_color(theme, pal);
    let floor_c = floor_color(pal);
    let ceil_c = wall * 0.55;
    let (mn, mx) = (room.min, room.max);
    let cx = (mn.x + mx.x) * 0.5;
    let cz = (mn.z + mx.z) * 0.5;
    let hx = (mx.x - mn.x) * 0.5;
    let hz = (mx.z - mn.z) * 0.5;
    let h = mx.y;

    let mut m = Mesh::new();
    // floor slab (top face at y = 0), overhanging the walls
    m.merge(&cuboid(
        Vec3::new(cx, -T * 0.5, cz),
        Vec3::new(hx + T, T * 0.5, hz + T),
        floor_c,
    ));
    // ceiling slab
    if include_ceiling {
        m.merge(&cuboid(
            Vec3::new(cx, h + T * 0.5, cz),
            Vec3::new(hx + T, T * 0.5, hz + T),
            ceil_c,
        ));
    }
    // four walls, interior faces flush with [mn, mx]
    m.merge(&cuboid(
        Vec3::new(cx, h * 0.5, mn.z - T * 0.5),
        Vec3::new(hx + T, h * 0.5, T * 0.5),
        wall,
    ));
    m.merge(&cuboid(
        Vec3::new(cx, h * 0.5, mx.z + T * 0.5),
        Vec3::new(hx + T, h * 0.5, T * 0.5),
        wall,
    ));
    m.merge(&cuboid(
        Vec3::new(mn.x - T * 0.5, h * 0.5, cz),
        Vec3::new(T * 0.5, h * 0.5, hz),
        wall,
    ));
    m.merge(&cuboid(
        Vec3::new(mx.x + T * 0.5, h * 0.5, cz),
        Vec3::new(T * 0.5, h * 0.5, hz),
        wall,
    ));
    m
}

/// Carve a doorway opening at each door through the room shell. Each cutter is
/// a closed box that straddles the wall (poking out both faces) and stops just
/// short of the floor and ceiling so a sill + header remain — the opening then
/// reads as a real doorway rather than a full-height gap.
pub fn carve_doorways(mut walls: Mesh, doors: &[Door]) -> Mesh {
    for d in doors {
        let cy0 = 0.02;
        let cutter = cuboid(
            Vec3::new(d.pos.x, (cy0 + DOOR_H) * 0.5, d.pos.z),
            Vec3::new(DOOR_W * 0.5, (DOOR_H - cy0) * 0.5, DOOR_W * 0.5),
            Vec3::ONE,
        );
        walls = csg::subtract(&walls, &cutter);
    }
    walls
}

/// A corridor shell: floor + ceiling + two side walls along each straight
/// segment, ends left open so it meets the carved room doorways.
pub fn corridor_mesh(
    c: &Corridor,
    theme: DungeonTheme,
    pal: &Palette,
    detail: f32,
    include_ceiling: bool,
) -> Mesh {
    let _ = detail;
    let wall = wall_color(theme, pal) * 0.92;
    let floor_c = floor_color(pal);
    let ceil_c = wall * 0.5;
    // corridors keep a consistent ceiling, a touch below the room ceiling
    let h = (super::model::DungeonModel::ceiling(theme) - 0.6).max(3.0);
    let mut m = Mesh::new();
    for seg in c.path.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        segment_shell(&mut m, a, b, h, floor_c, wall, ceil_c, include_ceiling);
    }
    m
}

#[allow(clippy::too_many_arguments)]
fn segment_shell(
    m: &mut Mesh,
    a: Vec3,
    b: Vec3,
    h: f32,
    floor_c: Vec3,
    wall: Vec3,
    ceil_c: Vec3,
    include_ceiling: bool,
) {
    let center = (a + b) * 0.5;
    let along_x = (b.x - a.x).abs() >= (b.z - a.z).abs();
    let len = if along_x {
        (b.x - a.x).abs()
    } else {
        (b.z - a.z).abs()
    };
    let half_along = len * 0.5 + CORR_W * 0.5; // overlap ends into rooms/corners
    let hw = CORR_W * 0.5;
    // half extents oriented along the run
    let (half_x, half_z) = if along_x {
        (half_along, hw)
    } else {
        (hw, half_along)
    };
    // floor
    m.merge(&cuboid(
        Vec3::new(center.x, -T * 0.5, center.z),
        Vec3::new(half_x + T, T * 0.5, half_z + T),
        floor_c,
    ));
    // ceiling
    if include_ceiling {
        m.merge(&cuboid(
            Vec3::new(center.x, h + T * 0.5, center.z),
            Vec3::new(half_x + T, T * 0.5, half_z + T),
            ceil_c,
        ));
    }
    // side walls (both sides of the run)
    if along_x {
        for sz in [-1.0f32, 1.0] {
            m.merge(&cuboid(
                Vec3::new(center.x, h * 0.5, center.z + sz * (hw + T * 0.5)),
                Vec3::new(half_x, h * 0.5, T * 0.5),
                wall,
            ));
        }
    } else {
        for sx in [-1.0f32, 1.0] {
            m.merge(&cuboid(
                Vec3::new(center.x + sx * (hw + T * 0.5), h * 0.5, center.z),
                Vec3::new(T * 0.5, h * 0.5, half_z),
                wall,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::DungeonModel;
    use super::*;
    use crate::palette::by_name;
    use crate::recipe::DungeonParams;

    fn params(json: &str) -> DungeonParams {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn room_geometry_has_floor_walls_ceiling_and_carved_door() {
        let p = params(r#"{"kind":"dungeon","type":"crypt","rooms":2}"#);
        let m = DungeonModel::new(&p, &by_name("necrotic")).unwrap();
        let solid = room_mesh(
            &m.rooms[0],
            DungeonTheme::Crypt,
            &by_name("necrotic"),
            1.0,
            true,
        );
        solid.validate().unwrap();
        // room 0's own doors
        let doors: Vec<Door> = m.doors.iter().filter(|d| d.room == 0).cloned().collect();
        assert!(!doors.is_empty(), "connected room 0 must have a doorway");
        let carved = carve_doorways(
            room_mesh(
                &m.rooms[0],
                DungeonTheme::Crypt,
                &by_name("necrotic"),
                1.0,
                true,
            ),
            &doors,
        );
        carved.validate().unwrap();
        assert!(carved.indices.len().is_multiple_of(3));
        assert!(carved.positions.len() > 100);
        // carving actually changed the geometry (an opening exists)
        assert!(carved.positions.len() != solid.positions.len());
    }

    #[test]
    fn corridor_mesh_builds() {
        let p = params(r#"{"kind":"dungeon","type":"mine","size":"small"}"#);
        let m = DungeonModel::new(&p, &by_name("volcanic")).unwrap();
        let c = corridor_mesh(
            &m.corridors[0],
            DungeonTheme::Mine,
            &by_name("volcanic"),
            1.0,
            true,
        );
        c.validate().unwrap();
        assert!(c.triangle_count() > 0);
        // ceiling-less corridor omits the ceiling slab: fewer triangles.
        let open = corridor_mesh(
            &m.corridors[0],
            DungeonTheme::Mine,
            &by_name("volcanic"),
            1.0,
            false,
        );
        open.validate().unwrap();
        assert!(open.triangle_count() < c.triangle_count());
    }
}
