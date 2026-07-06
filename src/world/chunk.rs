//! Build one world chunk as a standalone Asset. Heights are sampled with a
//! one-cell ring beyond the chunk so slopes/colors at the border use the
//! exact same world samples as the neighboring chunk — seams stay invisible
//! in color as well as in position.

use glam::{Quat, Vec3};
use rand::Rng;

use crate::gltf::{Asset, Collider, InstancedPart, Material, Part, Physics};
use crate::mesh::{Mesh, to_flat_shaded};

use super::model::WorldModel;
use crate::generators::{range, rng};

/// The chunk's height grid with a 1-cell ring (indices -1..=res+1) plus the
/// smooth per-vertex mesh (positions + colors) — the layer where the seam
/// contract holds bit-exactly: shared edge vertices of adjacent chunks have
/// identical positions AND colors (slopes come from the ring, i.e. from the
/// same world samples the neighbor uses).
pub fn vertex_grid(m: &WorldModel, cx: u32, cz: u32) -> (Vec<f32>, Mesh) {
    let res = m.p.chunk_resolution as usize;
    let cs = m.p.chunk_size;
    let (ox, oz) = m.chunk_origin(cx, cz);
    let cell = cs / res as f32;
    let n1 = res + 3;
    let local = |i: i64| (i as f32 / res as f32 - 0.5) * cs;
    let mut grid = vec![0.0f32; n1 * n1];
    for iz in -1..=(res as i64 + 1) {
        for ix in -1..=(res as i64 + 1) {
            grid[(iz + 1) as usize * n1 + (ix + 1) as usize] =
                m.height(local(ix) + ox, local(iz) + oz);
        }
    }
    let g = |ix: i64, iz: i64| grid[(iz + 1) as usize * n1 + (ix + 1) as usize];
    let mut smooth = Mesh::new();
    for iz in 0..=(res as i64) {
        for ix in 0..=(res as i64) {
            let (lx, lz) = (local(ix), local(iz));
            let h = g(ix, iz);
            let hx = g(ix + 1, iz) - g(ix - 1, iz);
            let hz = g(ix, iz + 1) - g(ix, iz - 1);
            let slope = (hx * hx + hz * hz).sqrt() / (2.0 * cell);
            let c = m.color(lx + ox, lz + oz, h, slope);
            smooth.push_vertex(Vec3::new(lx, h, lz), Vec3::Y, c);
        }
    }
    (grid, smooth)
}

pub fn build(m: &WorldModel, cx: u32, cz: u32) -> Asset {
    let res = m.p.chunk_resolution as usize;
    let cs = m.p.chunk_size;
    let sea = m.p.sea_level;
    let n1 = res + 3;
    let (grid, mut smooth) = vertex_grid(m, cx, cz);
    let g = |ix: i64, iz: i64| grid[(iz + 1) as usize * n1 + (ix + 1) as usize];
    for iz in 0..res {
        for ix in 0..res {
            let a = (iz * (res + 1) + ix) as u32;
            let b = a + 1;
            let c = a + (res + 1) as u32;
            let d = c + 1;
            // alternate the diagonal in a world-consistent checkerboard
            if (ix + iz) % 2 == 0 {
                smooth.push_tri(a, c, b);
                smooth.push_tri(b, c, d);
            } else {
                smooth.push_tri(a, c, d);
                smooth.push_tri(a, d, b);
            }
        }
    }
    let ground = to_flat_shaded(&smooth);

    let mut parts = vec![Part {
        mesh: ground,
        material: Material { roughness: 0.95, ..Default::default() },
    }];

    // sea: one continuous world-level plane, clipped to this chunk
    let h_lo = grid.iter().cloned().fold(f32::INFINITY, f32::min);
    if h_lo < sea {
        let mut water = Mesh::new();
        let s = cs / 2.0;
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
                emissive: if wc.x > wc.z { wc * 0.9 } else { wc * 0.12 },
                ..Default::default()
            },
        });
    }

    // scatter: deterministic per-chunk (seeded from world seed + chunk
    // coords), placed on the sampled grid
    let mut instanced: Vec<InstancedPart> = Vec::new();
    if m.p.scatter {
        let mut r = rng(m.chunk_seed(cx, cz));
        let mut variants: Vec<(Mesh, f32)> = Vec::new();
        for _ in 0..3 {
            variants.push((
                crate::generators::terrain::tree_billboardless(&mut r, &m.pal, 1.0),
                5.5,
            ));
        }
        for _ in 0..2 {
            variants.push((crate::generators::rock::rock_mesh(&mut r, &m.pal, 1.0, 0.5), 2.4));
        }
        let sample = |x: f32, z: f32| -> f32 {
            let fx = ((x / cs + 0.5) * res as f32).clamp(0.0, res as f32 - 0.001);
            let fz = ((z / cs + 0.5) * res as f32).clamp(0.0, res as f32 - 0.001);
            let (ix, iz) = (fx as i64, fz as i64);
            let (dx, dz) = (fx - ix as f32, fz - iz as f32);
            g(ix, iz) * (1.0 - dx) * (1.0 - dz)
                + g(ix + 1, iz) * dx * (1.0 - dz)
                + g(ix, iz + 1) * (1.0 - dx) * dz
                + g(ix + 1, iz + 1) * dx * dz
        };
        let density = m.p.scatter_density.clamp(0.1, 8.0);
        let count = (cs * cs * 0.010 * density) as usize;
        let (ox, oz) = m.chunk_origin(cx, cz);
        let mut placements: Vec<Vec<(Vec3, Quat, Vec3)>> = vec![Vec::new(); variants.len()];
        for _ in 0..count {
            let x = range(&mut r, -cs * 0.49, cs * 0.49);
            let z = range(&mut r, -cs * 0.49, cs * 0.49);
            // zone-driven density + mix: smooth across borders (weights), so
            // forests thin out into plains instead of stopping at a line
            let zw = m.zones.weights(x + ox, z + oz);
            let mut dens = 0.0f32;
            let mut best = 0usize;
            for i in 0..crate::world::zones::NK {
                dens += zw[i] * crate::world::zones::scatter_profile(crate::world::zones::KINDS[i]).0;
                if zw[i] > zw[best] {
                    best = i;
                }
            }
            let keep = (dens / 2.4).clamp(0.0, 1.0) as f64;
            let roll = r.gen_bool(keep.max(1e-9));
            let h = sample(x, z);
            let probe = 1.2;
            let s = ((sample(x + probe, z) - h).abs() + (sample(x, z + probe) - h).abs()) / probe;
            let t_alt = ((h - sea) / (m.amp * 3.5)).clamp(0.0, 1.0).powf(0.5);
            let tree_frac =
                crate::world::zones::scatter_profile(crate::world::zones::KINDS[best]).1;
            let is_tree = r.gen_bool(tree_frac);
            // treeline drops in mountain zones: bare rocky heights
            let treeline = 0.58 - zw[crate::world::zones::ZoneKind::Mountains.index()] * 0.22;
            if !roll || h < sea + 0.6 || s > 0.7 || t_alt > treeline {
                continue;
            }
            // clear settlements and dungeon mouths
            let (wxs, wzs) = (x + ox, z + oz);
            if m.pois.iter().any(|p| {
                let rr = p.radius * 1.45;
                (wxs - p.x).powi(2) + (wzs - p.z).powi(2) < rr * rr
            }) {
                continue;
            }
            let vi = if is_tree { r.gen_range(0..3usize) } else { 3 + r.gen_range(0..2usize) };
            let scale = range(&mut r, 0.5, 1.15) * variants[vi].1;
            let yaw = range(&mut r, 0.0, core::f32::consts::TAU);
            placements[vi].push((
                Vec3::new(x, h - 0.05, z),
                Quat::from_rotation_y(yaw),
                Vec3::splat(scale),
            ));
        }
        for (vi, (mesh, _)) in variants.into_iter().enumerate() {
            if placements[vi].is_empty() {
                continue;
            }
            instanced.push(InstancedPart {
                part: Part {
                    mesh,
                    material: Material {
                        roughness: if vi < 3 { 0.9 } else { 0.95 },
                        ..Default::default()
                    },
                },
                transforms: std::mem::take(&mut placements[vi]),
            });
        }
    }

    let mut asset = Asset::static_mesh(
        &format!("chunk_{cx}_{cz}"),
        parts,
        Some(Physics {
            collider: Collider::Heightfield,
            mass: 0.0,
            friction: 0.9,
            restitution: 0.05,
        }),
    );
    asset.instanced = instanced;
    asset
}
