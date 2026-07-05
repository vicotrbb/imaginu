//! Stylized buildings: timber-framed cottage with gabled roof, chimney,
//! door, windows and a stone footing.

use glam::Vec3;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, cuboid, to_flat_shaded};
use crate::palette::{Palette, lerp, srgb, vary};
use crate::recipe::BuildingParams;

use super::{range, rng};

pub fn generate(p: &BuildingParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let w = p.width.clamp(2.0, 20.0);
    let d = w * range(&mut r, 0.7, 0.9);
    let floors = p.floors.clamp(1, 3) as f32;
    let wall_h = w * 0.42 * floors;

    let wall_col = lerp(srgb(232, 222, 202), pal.terrain[1], 0.15);
    let timber = pal.trunk * 0.75;
    let roof_col = vary(pal.accent, 0.1, 0.3) * 0.8;
    let stone = pal.rock[0];

    let mut m = Mesh::new();

    // stone footing
    m.merge(&cuboid(
        Vec3::new(0.0, w * 0.05, 0.0),
        Vec3::new(w / 2.0 + w * 0.03, w * 0.05, d / 2.0 + w * 0.03),
        stone,
    ));
    // walls
    m.merge(&cuboid(
        Vec3::new(0.0, w * 0.1 + wall_h / 2.0, 0.0),
        Vec3::new(w / 2.0, wall_h / 2.0, d / 2.0),
        wall_col,
    ));
    // timber corner posts + horizontal beams
    let post = |x: f32, z: f32| {
        cuboid(
            Vec3::new(x, w * 0.1 + wall_h / 2.0, z),
            Vec3::new(w * 0.025, wall_h / 2.0, w * 0.025),
            timber,
        )
    };
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        m.merge(&post(sx * w / 2.0, sz * d / 2.0));
    }
    for f in 0..floors as u32 + 1 {
        let y = w * 0.1 + wall_h * f as f32 / floors;
        m.merge(&cuboid(
            Vec3::new(0.0, y, 0.0),
            Vec3::new(w / 2.0 + 0.01 * w, w * 0.02, d / 2.0 + 0.01 * w),
            timber,
        ));
    }

    // gabled roof (triangular prism with overhang)
    let roof_h = w * 0.34;
    let over = w * 0.09;
    let y0 = w * 0.1 + wall_h;
    let ridge_y = y0 + roof_h;
    let hw = w / 2.0 + over;
    let hd = d / 2.0 + over;
    let a = Vec3::new(-hw, y0, -hd);
    let b = Vec3::new(hw, y0, -hd);
    let c = Vec3::new(hw, y0, hd);
    let dd = Vec3::new(-hw, y0, hd);
    let r1 = Vec3::new(0.0, ridge_y, -hd);
    let r2 = Vec3::new(0.0, ridge_y, hd);
    let mut roof = Mesh::new();
    roof.add_flat_quad(a, r1, r2, dd, roof_col); // left slope
    roof.add_flat_quad(b, c, r2, r1, roof_col); // right slope
    roof.add_flat_tri(a, b, r1, wall_col); // gable front
    roof.add_flat_tri(c, dd, r2, wall_col); // gable back
    // slope underside (overhang visible from below)
    roof.add_flat_quad(a, dd, r2, r1, roof_col * 0.6);
    roof.add_flat_quad(b, r1, r2, c, roof_col * 0.6);
    m.merge(&roof);

    // door
    let door_w = w * 0.14;
    let door_h = w * 0.28;
    m.merge(&cuboid(
        Vec3::new(0.0, w * 0.1 + door_h / 2.0, d / 2.0 + w * 0.012),
        Vec3::new(door_w, door_h / 2.0, w * 0.015),
        pal.trunk,
    ));
    // windows
    let win = srgb(140, 190, 210);
    let win_col = lerp(win, pal.accent, 0.08);
    for f in 0..floors as u32 {
        let y = w * 0.1 + wall_h * (f as f32 + 0.55) / floors;
        for sx in [-1.0f32, 1.0] {
            m.merge(&cuboid(
                Vec3::new(sx * w * 0.28, y, d / 2.0 + w * 0.012),
                Vec3::new(w * 0.07, w * 0.08, w * 0.012),
                win_col,
            ));
            m.merge(&cuboid(
                Vec3::new(sx * (w / 2.0 + w * 0.012), y, 0.0),
                Vec3::new(w * 0.012, w * 0.08, w * 0.07),
                win_col,
            ));
        }
    }
    // chimney
    let ch_x = w * range(&mut r, 0.18, 0.3);
    let mut chimney = cuboid(
        Vec3::new(ch_x, ridge_y - roof_h * 0.25, -d * 0.15),
        Vec3::new(w * 0.06, roof_h * 0.55, w * 0.06),
        stone * 0.9,
    );
    chimney.merge(&cuboid(
        Vec3::new(ch_x, ridge_y + roof_h * 0.32, -d * 0.15),
        Vec3::new(w * 0.08, w * 0.02, w * 0.08),
        stone * 0.7,
    ));
    m.merge(&chimney);

    let m = to_flat_shaded(&m);
    let half = Vec3::new(hw, (ridge_y) / 2.0, hd);
    Asset::static_mesh(
        "building",
        vec![Part { mesh: m, material: Material { roughness: 0.9, ..Default::default() } }],
        Some(Physics {
            collider: Collider::Box { half_extents: half },
            mass: 0.0,
            friction: 0.8,
            restitution: 0.05,
        }),
    )
}
