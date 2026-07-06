//! Map-level presentation: a stitched downsampled world asset for oblique
//! full-map beauty shots, and corridor assets (real chunks merged in world
//! space) for flyover showcases.

use glam::{Vec2, Vec3};

use crate::gltf::{Asset, Material, Part};
use crate::mesh::Mesh;

use super::model::WorldModel;

/// Downsampled whole-world asset: ground mesh + sea + rivers + real POI
/// geometry placed at world positions. `grid` ≈ vertices per edge.
pub fn world_asset(m: &WorldModel, grid: usize) -> Asset {
    let n = grid.clamp(64, 900);
    let size = m.size_x;
    let step = size / n as f32;
    let mut heights = vec![0.0f32; (n + 1) * (n + 1)];
    for jz in 0..=n {
        for jx in 0..=n {
            heights[jz * (n + 1) + jx] = m.height(
                jx as f32 * step - size * 0.5,
                jz as f32 * step - size * 0.5,
            );
        }
    }
    let at = |jx: usize, jz: usize| heights[jz * (n + 1) + jx];
    let mut ground = Mesh::new();
    for jz in 0..=n {
        for jx in 0..=n {
            let (wx, wz) = (jx as f32 * step - size * 0.5, jz as f32 * step - size * 0.5);
            let h = at(jx, jz);
            let hx = at((jx + 1).min(n), jz) - at(jx.saturating_sub(1), jz);
            let hz = at(jx, (jz + 1).min(n)) - at(jx, jz.saturating_sub(1));
            let slope = (hx * hx + hz * hz).sqrt() / (2.0 * step);
            let nrm = Vec3::new(-hx, 2.0 * step, -hz).normalize_or(Vec3::Y);
            let c = m.color(wx, wz, h, slope);
            ground.push_vertex(Vec3::new(wx, h, wz), nrm, c);
        }
    }
    for jz in 0..n {
        for jx in 0..n {
            let a = (jz * (n + 1) + jx) as u32;
            let b = a + 1;
            let c = a + (n + 1) as u32;
            let d = c + 1;
            ground.push_tri(a, c, b);
            ground.push_tri(b, c, d);
        }
    }
    let mut parts = vec![Part {
        mesh: ground,
        material: Material { roughness: 0.95, ..Default::default() },
    }];
    // sea plane
    let sea = m.p.sea_level;
    let mut water = Mesh::new();
    let s = size / 2.0;
    let wc = m.pal.water;
    let a = water.push_vertex(Vec3::new(-s, sea, -s), Vec3::Y, wc);
    let b = water.push_vertex(Vec3::new(s, sea, -s), Vec3::Y, wc);
    let c = water.push_vertex(Vec3::new(s, sea, s), Vec3::Y, wc);
    let d = water.push_vertex(Vec3::new(-s, sea, s), Vec3::Y, wc);
    water.push_tri(a, c, b);
    water.push_tri(a, d, c);
    parts.push(Part {
        mesh: water,
        material: Material {
            roughness: 0.12,
            metallic: 0.1,
            emissive: wc * 0.10,
            ..Default::default()
        },
    });
    // river ribbons across the whole map
    let ribbons = m.network.river_ribbons_in(Vec2::new(-s, -s), Vec2::new(s, s));
    if !ribbons.is_empty() {
        let mut ribbon = Mesh::new();
        for (p, q, w) in &ribbons {
            let surf = m.network.river_depth * 0.55;
            let p = *p + Vec3::Y * surf;
            let q = *q + Vec3::Y * surf;
            let dir = (q - p).normalize_or(Vec3::X);
            let side = dir.cross(Vec3::Y).normalize_or(Vec3::Z) * (*w * 0.9);
            ribbon.add_flat_quad(p - side, q - side, q + side, p + side, wc);
        }
        parts.push(Part {
            mesh: ribbon,
            material: Material {
                roughness: 0.12,
                metallic: 0.1,
                emissive: wc * 0.10,
                double_sided: true,
                ..Default::default()
            },
        });
    }
    // real POI + bridge geometry at world positions
    add_poi_parts(m, &mut parts, None);
    Asset::static_mesh("world_overview", parts, None)
}

/// Real chunks near the segment [a, b] merged into one world-space asset
/// (for flyover renders).
pub fn corridor_asset(m: &WorldModel, a: Vec2, b: Vec2, radius: f32) -> Asset {
    let cs = m.p.chunk_size;
    let mut ground = Mesh::new();
    let mut waters = Mesh::new();
    let mut parts: Vec<Part> = Vec::new();
    let mut instanced: Vec<crate::gltf::InstancedPart> = Vec::new();
    for cz in 0..m.nz {
        for cx in 0..m.nx {
            let (ox, oz) = m.chunk_origin(cx, cz);
            let p = Vec2::new(ox, oz);
            let ab = b - a;
            let t = if ab.length_squared() > 1e-6 {
                ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let d = (p - (a + ab * t)).length();
            if d > radius + cs * 0.75 {
                continue;
            }
            let asset = super::chunk::build(m, cx, cz);
            let shift = Vec3::new(ox, 0.0, oz);
            for (i, part) in asset.parts.into_iter().enumerate() {
                let mut mesh = part.mesh;
                mesh.translate(shift);
                if i == 0 {
                    ground.merge(&mesh);
                } else {
                    waters.merge(&mesh);
                }
            }
            for mut ip in asset.instanced.into_iter() {
                for tr in ip.transforms.iter_mut() {
                    tr.0 += shift;
                }
                instanced.push(ip);
            }
        }
    }
    parts.push(Part { mesh: ground, material: Material { roughness: 0.95, ..Default::default() } });
    if waters.vertex_count() > 0 {
        let wc = m.pal.water;
        parts.push(Part {
            mesh: waters,
            material: Material {
                roughness: 0.12,
                metallic: 0.1,
                emissive: wc * 0.10,
                double_sided: true,
                ..Default::default()
            },
        });
    }
    add_poi_parts(m, &mut parts, Some((a, b, radius + cs)));
    let mut asset = Asset::static_mesh("world_corridor", parts, None);
    asset.instanced = instanced;
    asset
}

fn add_poi_parts(m: &WorldModel, parts: &mut Vec<Part>, near: Option<(Vec2, Vec2, f32)>) {
    let keep = |x: f32, z: f32| -> bool {
        match near {
            None => true,
            Some((a, b, r)) => {
                let p = Vec2::new(x, z);
                let ab = b - a;
                let t = if ab.length_squared() > 1e-6 {
                    ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (p - (a + ab * t)).length() < r
            }
        }
    };
    for site in &m.pois {
        if !keep(site.x, site.z) {
            continue;
        }
        let asset = super::poi::build_asset(site, &m.pal);
        let shift = Vec3::new(site.x, site.ground, site.z);
        for part in &asset.parts {
            let mut mesh = part.mesh.clone();
            mesh.translate(shift);
            parts.push(Part { mesh, material: part.material.clone() });
        }
    }
    for bge in &m.network.bridges {
        if !keep(bge.pos.x, bge.pos.y) {
            continue;
        }
        let asset = super::poi::bridge_asset(bge, &m.pal);
        for part in &asset.parts {
            let mut mesh = part.mesh.clone();
            mesh.translate(Vec3::new(bge.pos.x, bge.deck, bge.pos.y));
            parts.push(Part { mesh, material: part.material.clone() });
        }
    }
}
