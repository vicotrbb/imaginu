//! Build one world chunk as a standalone Asset. Heights are sampled with a
//! one-cell ring beyond the chunk so slopes/colors at the border use the
//! exact same world samples as the neighboring chunk — seams stay invisible
//! in color as well as in position.

use glam::{Quat, Vec3};
use rand::Rng;

use crate::gltf::{Asset, Collider, InstancedPart, Material, Part, Physics};
use crate::mesh::Mesh;

use super::model::WorldModel;
use crate::generators::{range, rng};

/// The chunk's height grid with a 1-cell ring (indices -1..=res+1) plus the
/// smooth per-vertex mesh (positions + colors) — the layer where the seam
/// contract holds bit-exactly: shared edge vertices of adjacent chunks have
/// identical positions AND colors (slopes come from the ring, i.e. from the
/// same world samples the neighbor uses).
/// Vertex on the `eff`-resolution edge lattice, computed exactly as the
/// (possibly coarser) neighbor computes it: heights at the shared world
/// coordinates, slope from central differences at `eff` spacing.
fn lattice_vertex(
    m: &WorldModel,
    ox: f32,
    oz: f32,
    cs: f32,
    eff: u32,
    x_edge: bool,
    side: f32,
    j: u32,
) -> (f32, Vec3, Vec3) {
    let cell = cs / eff as f32;
    let along = (j as f32 / eff as f32 - 0.5) * cs;
    let (lx, lz) = if x_edge { (side * cs, along) } else { (along, side * cs) };
    let (wx, wz) = (ox + lx, oz + lz);
    let h = m.height(wx, wz);
    let hx = m.height(wx + cell, wz) - m.height(wx - cell, wz);
    let hz = m.height(wx, wz + cell) - m.height(wx, wz - cell);
    let slope = (hx * hx + hz * hz).sqrt() / (2.0 * cell);
    let nrm = Vec3::new(-hx, 2.0 * cell, -hz).normalize_or(Vec3::Y);
    (h, nrm, m.color(wx, wz, h, slope))
}

pub fn vertex_grid(m: &WorldModel, cx: u32, cz: u32) -> (Vec<f32>, Mesh) {
    let res = m.chunk_res(cx, cz) as usize;
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
    // effective per-edge resolution: min(own, neighbor) — both sides compute
    // the same value (pure function), so stitching is crack-free
    let res32 = res as u32;
    let eff_w = if cx > 0 { m.chunk_res(cx - 1, cz).min(res32) } else { res32 };
    let eff_e = if cx + 1 < m.nx { m.chunk_res(cx + 1, cz).min(res32) } else { res32 };
    let eff_n = if cz > 0 { m.chunk_res(cx, cz - 1).min(res32) } else { res32 };
    let eff_s = if cz + 1 < m.nz { m.chunk_res(cx, cz + 1).min(res32) } else { res32 };
    let stitch = |i_along: usize, eff: u32, x_edge: bool, side: f32| -> (f32, Vec3, Vec3) {
        let ratio = res / eff as usize;
        let j = (i_along / ratio) as u32;
        let fr = (i_along % ratio) as f32 / ratio as f32;
        let (h0, n0, c0) = lattice_vertex(m, ox, oz, cs, eff, x_edge, side, j);
        if fr == 0.0 {
            (h0, n0, c0)
        } else {
            let (h1, n1, c1) = lattice_vertex(m, ox, oz, cs, eff, x_edge, side, j + 1);
            (
                h0 + (h1 - h0) * fr,
                (n0 + (n1 - n0) * fr).normalize_or(Vec3::Y),
                c0 + (c1 - c0) * fr,
            )
        }
    };
    let mut smooth = Mesh::new();
    for iz in 0..=(res as i64) {
        for ix in 0..=(res as i64) {
            let (lx, lz) = (local(ix), local(iz));
            let over = if ix == 0 && eff_w < res32 {
                Some(stitch(iz as usize, eff_w, true, -0.5))
            } else if ix == res as i64 && eff_e < res32 {
                Some(stitch(iz as usize, eff_e, true, 0.5))
            } else if iz == 0 && eff_n < res32 {
                Some(stitch(ix as usize, eff_n, false, -0.5))
            } else if iz == res as i64 && eff_s < res32 {
                Some(stitch(ix as usize, eff_s, false, 0.5))
            } else {
                None
            };
            let (h, nrm, c) = match over {
                Some(hnc) => hnc,
                None => {
                    let h = g(ix, iz);
                    let hx = g(ix + 1, iz) - g(ix - 1, iz);
                    let hz = g(ix, iz + 1) - g(ix, iz - 1);
                    let slope = (hx * hx + hz * hz).sqrt() / (2.0 * cell);
                    let nrm = Vec3::new(-hx, 2.0 * cell, -hz).normalize_or(Vec3::Y);
                    (h, nrm, m.color(lx + ox, lz + oz, h, slope))
                }
            };
            smooth.push_vertex(Vec3::new(lx, h, lz), nrm, c);
        }
    }
    (grid, smooth)
}

pub fn build(m: &WorldModel, cx: u32, cz: u32) -> Asset {
    let res = m.chunk_res(cx, cz) as usize;
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
    // smooth-shaded indexed mesh: 6× smaller than flat-shading, normals from
    // world-space central differences (bit-identical at seams)
    let ground = smooth;

    let mut parts = vec![Part {
        mesh: ground,
        material: Material { roughness: 0.95, ..Default::default() },
    }];

    // river water ribbons: world polyline segments clipped to this chunk
    // (world-space, so ribbons continue exactly across borders)
    let (ox, oz) = m.chunk_origin(cx, cz);
    let ribbons = m.network.river_ribbons_in(
        glam::Vec2::new(ox - cs / 2.0, oz - cs / 2.0),
        glam::Vec2::new(ox + cs / 2.0, oz + cs / 2.0),
    );
    if !ribbons.is_empty() {
        let mut ribbon = Mesh::new();
        let wc = m.pal.water;
        for (a, b, w) in &ribbons {
            let surf = m.network.river_depth * 0.55;
            let a = Vec3::new(a.x - ox, a.y + surf, a.z - oz);
            let b = Vec3::new(b.x - ox, b.y + surf, b.z - oz);
            let dir = (b - a).normalize_or(Vec3::X);
            let side = dir.cross(Vec3::Y).normalize_or(Vec3::Z) * (w * 0.75);
            ribbon.add_flat_quad(a - side, b - side, b + side, a + side, wc);
        }
        if ribbon.vertex_count() > 0 {
            parts.push(Part {
                mesh: ribbon,
                material: Material {
                    roughness: 0.12,
                    metallic: 0.1,
                    emissive: if wc.x > wc.z { wc * 0.9 } else { wc * 0.10 },
                    double_sided: true,
                    ..Default::default()
                },
            });
        }
    }

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
            // clear settlements, dungeon mouths, roads and river channels
            let (wxs, wzs) = (x + ox, z + oz);
            if m.pois.iter().any(|p| {
                let rr = p.radius * 1.45;
                (wxs - p.x).powi(2) + (wzs - p.z).powi(2) < rr * rr
            }) || m.network.road_mask(wxs, wzs) > 0.12
                || m.network.river_mask(wxs, wzs) > 0.5
            {
                continue;
            }
            // scree/talus: rocks pile up on steep mountain flanks
            let scree = s > 0.38
                && (zw[crate::world::zones::ZoneKind::Mountains.index()]
                    + zw[crate::world::zones::ZoneKind::Badlands.index()])
                    > 0.3;
            let vi = if is_tree && !scree {
                r.gen_range(0..3usize)
            } else {
                3 + r.gen_range(0..2usize)
            };
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
