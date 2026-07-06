//! Terrain / map chunk generator: warped-fBm + ridged heightfield, biome
//! coloring by altitude & slope, water plane, optional scattered vegetation.

use glam::{Mat4, Vec3};
use rand::Rng;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, to_flat_shaded};
use crate::noise::Noise2;
use crate::palette::{Palette, ramp, vary};
use crate::recipe::{TerrainParams, TerrainShape};

use super::{Rand, range, rng};

pub fn generate(p: &TerrainParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let n = Noise2::new(p.seed);
    let res = p.resolution.clamp(16, 1024) as usize;
    let size = p.size.clamp(4.0, 4096.0);
    let shape = p.shape;
    let amp = size * 0.16
        * p.mountainousness.clamp(0.05, 3.0)
        * if shape == TerrainShape::Mountains { 1.5 } else { 1.0 };
    // noise frequency fixed in world units so adjacent chunks tile seamlessly
    let freq = 3.0 / size.min(96.0).max(24.0);
    let (ox, oz) = (p.offset_x, p.offset_z);

    let height = |x: f32, z: f32| -> f32 {
        let (wx, wz) = (x + ox, z + oz);
        let (u, v) = (wx * freq, wz * freq);
        let base = n.warped_fbm(u, v, 6, 0.9);
        let ridge = n.ridged(u * 0.8 + 13.7, v * 0.8 + 4.2, 5) - 0.65;
        let mask = ((base + 0.35) * 1.4).clamp(0.0, 1.0);
        let ridge_w = match shape {
            TerrainShape::Mountains => 1.4,
            TerrainShape::Dunes => 0.0,
            _ => 0.9,
        };
        let mut h = (base * 0.6 + ridge * mask * ridge_w) * amp;
        if shape == TerrainShape::Dunes {
            // long anisotropic ridges
            let d = 1.0 - (n.sample(u * 0.7 + v * 2.4, v * 0.5).abs());
            h = (base * 0.25 + d * 0.75) * amp * 0.5;
        }
        // macro-shape masks in LOCAL chunk coordinates
        let rx = x / (size * 0.5);
        let rz = z / (size * 0.5);
        let r = (rx * rx + rz * rz).sqrt();
        match shape {
            TerrainShape::Island => {
                let fall = ((r - 0.55) * 2.6).clamp(0.0, 1.0);
                h -= fall * fall * amp * 2.2;
                h += (1.0 - r).clamp(0.0, 1.0) * amp * 0.5;
            }
            TerrainShape::Archipelago => {
                let m = n.fbm((wx + 311.0) * freq * 0.6, (wz + 97.0) * freq * 0.6, 3, 2.0, 0.5);
                h -= amp * 0.9;
                h += (m + 0.25).max(0.0) * amp * 2.4;
            }
            TerrainShape::Canyon => {
                // deep channel wandering along X
                let curve = n.fbm(wx * freq * 0.5 + 41.0, 7.7, 3, 2.0, 0.5) * size * 0.22;
                let d = ((z - curve).abs() / (size * 0.16)).min(1.0);
                let carve = 1.0 - d * d;
                h += amp * 0.9; // raised plateau
                h -= carve * amp * 2.4;
            }
            TerrainShape::Mesa => {
                let m = n.fbm((wx + 87.0) * freq * 0.7, (wz + 13.0) * freq * 0.7, 3, 2.0, 0.5);
                let plate = ((m + 0.15) * 5.0).clamp(0.0, 1.0);
                h = h * 0.25 + plate * amp * 1.15;
            }
            TerrainShape::Crater => {
                let rim = (-((r - 0.62) * (r - 0.62)) / 0.012).exp();
                h += rim * amp * 1.5;
                if r < 0.62 {
                    h -= (1.0 - (r / 0.62).powi(2)) * amp * 1.3;
                }
            }
            TerrainShape::Valley => {
                let d = (rz.abs()).min(1.0);
                h += (d * d) * amp * 1.4 - amp * 0.5;
            }
            _ => {}
        }
        if p.terrace > 0.5 {
            let step = amp * 2.0 / p.terrace;
            let q = (h / step).floor() * step;
            // soften riser edges slightly
            h = q + ((h - q) / step).powf(3.0) * step;
        }
        h
    };

    // heightfield grid
    let mut grid = vec![0.0f32; (res + 1) * (res + 1)];
    for iz in 0..=res {
        for ix in 0..=res {
            let x = (ix as f32 / res as f32 - 0.5) * size;
            let z = (iz as f32 / res as f32 - 0.5) * size;
            grid[iz * (res + 1) + ix] = height(x, z);
        }
    }

    // hydraulic erosion (chunk-local: sculpts gullies and sediment fans)
    if p.erosion > 0.0 {
        erode(&mut grid, res, p.erosion.clamp(0.0, 1.0), p.seed);
    }

    // dirt paths: flatten the terrain along splines, remember a color mask
    let mut path_mask = vec![0.0f32; (res + 1) * (res + 1)];
    for spec in &p.paths {
        flatten_path(&mut grid, &mut path_mask, spec, res, size);
    }

    let mut h_min = grid.iter().cloned().fold(f32::INFINITY, f32::min);
    let mut h_max = grid.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let water_h = h_min + (h_max - h_min) * p.water_level.clamp(0.0, 0.95);

    // rivers: trace springs downhill, carve channels, keep ribbon paths
    let mut rivers: Vec<Vec<Vec3>> = Vec::new();
    for k in 0..p.rivers.min(8) {
        if let Some(ribbon) =
            carve_river(&mut grid, res, size, amp, water_h, p.seed.wrapping_add(k as u64 * 7919))
        {
            rivers.push(ribbon);
        }
    }
    if p.rivers > 0 {
        h_min = grid.iter().cloned().fold(f32::INFINITY, f32::min);
        h_max = grid.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    }

    let mut smooth = Mesh::new();
    for iz in 0..=res {
        for ix in 0..=res {
            let x = (ix as f32 / res as f32 - 0.5) * size;
            let z = (iz as f32 / res as f32 - 0.5) * size;
            let h = grid[iz * (res + 1) + ix];
            // slope from neighbors
            let hx = grid[iz * (res + 1) + (ix + 1).min(res)]
                - grid[iz * (res + 1) + ix.saturating_sub(1)];
            let hz = grid[(iz + 1).min(res) * (res + 1) + ix]
                - grid[iz.saturating_sub(1) * (res + 1) + ix];
            let cell = size / res as f32;
            let slope = (hx * hx + hz * hz).sqrt() / (2.0 * cell);
            let t_alt = ((h - h_min) / (h_max - h_min + 1e-6)).clamp(0.0, 1.0);
            // biome ramp with slope pushing toward rock (index 4)
            let mut c = ramp(&pal.terrain[0..4], t_alt.powf(0.9));
            let rockiness = ((slope - 0.55) * 2.0).clamp(0.0, 1.0);
            c = c * (1.0 - rockiness) + pal.terrain[4] * rockiness;
            // snow caps
            let snow = ((t_alt - 0.78) * 8.0).clamp(0.0, 1.0) * (1.0 - rockiness * 0.6);
            c = c * (1.0 - snow) + pal.terrain[5] * snow;
            // shore sand near the waterline
            let shore = (1.0 - ((h - water_h).abs() / (amp * 0.06 + 1e-3))).clamp(0.0, 1.0);
            c = c * (1.0 - shore * 0.7) + pal.terrain[0] * shore * 0.7;
            let mut c = vary(c, 0.10, n.sample(x * 2.1 + 31.0, z * 2.1 + 17.0) * 0.5 + 0.5);
            // dirt path tint
            let pm = path_mask[iz * (res + 1) + ix];
            if pm > 0.0 {
                c = crate::palette::lerp(c, pal.trunk * 0.85, pm.min(1.0) * 0.8);
            }
            smooth.push_vertex(Vec3::new(x, h, z), Vec3::Y, c);
        }
    }
    for iz in 0..res {
        for ix in 0..res {
            let a = (iz * (res + 1) + ix) as u32;
            let b = a + 1;
            let c = a + (res + 1) as u32;
            let d = c + 1;
            m_push_quad(&mut smooth, a, b, c, d, ix + iz);
        }
    }
    let mut ground = to_flat_shaded(&smooth);
    // diorama skirt: closed earthen sides + bottom so the chunk reads as a
    // hand-crafted slab instead of a floating sheet
    if p.skirt {
        let base_y = h_min - amp * 0.35 - size * 0.02;
        let soil = pal.terrain[4] * 0.45;
        let edge = |i: usize| i as f32 / res as f32 * size - size / 2.0;
        let mut skirt = Mesh::new();
        for i in 0..res {
            let (x0, x1) = (edge(i), edge(i + 1));
            let g = |ix: usize, iz: usize| grid[iz * (res + 1) + ix];
            skirt.add_flat_quad(
                Vec3::new(x0, base_y, -size / 2.0),
                Vec3::new(x1, base_y, -size / 2.0),
                Vec3::new(x1, g(i + 1, 0), -size / 2.0),
                Vec3::new(x0, g(i, 0), -size / 2.0),
                soil,
            );
            skirt.add_flat_quad(
                Vec3::new(x1, base_y, size / 2.0),
                Vec3::new(x0, base_y, size / 2.0),
                Vec3::new(x0, g(i, res), size / 2.0),
                Vec3::new(x1, g(i + 1, res), size / 2.0),
                soil,
            );
            skirt.add_flat_quad(
                Vec3::new(-size / 2.0, base_y, x1),
                Vec3::new(-size / 2.0, base_y, x0),
                Vec3::new(-size / 2.0, g(0, i), x0),
                Vec3::new(-size / 2.0, g(0, i + 1), x1),
                soil,
            );
            skirt.add_flat_quad(
                Vec3::new(size / 2.0, base_y, x0),
                Vec3::new(size / 2.0, base_y, x1),
                Vec3::new(size / 2.0, g(res, i + 1), x1),
                Vec3::new(size / 2.0, g(res, i), x0),
                soil,
            );
        }
        skirt.add_flat_quad(
            Vec3::new(-size / 2.0, base_y, -size / 2.0),
            Vec3::new(-size / 2.0, base_y, size / 2.0),
            Vec3::new(size / 2.0, base_y, size / 2.0),
            Vec3::new(size / 2.0, base_y, -size / 2.0),
            soil * 0.8,
        );
        ground.merge(&skirt);
    }

    // optional baked texture (e.g. rock strata visible on cliffs)
    let ground_tex = match &p.texture {
        Some(spec) => match crate::texture::bake(spec) {
            Ok(t) => {
                crate::uv::box_project(&mut ground, spec.scale.max(0.5));
                Some(std::sync::Arc::new(t))
            }
            Err(_) => None,
        },
        None => None,
    };
    let mut parts = vec![Part {
        mesh: ground,
        material: Material { roughness: 0.95, texture: ground_tex, ..Default::default() },
    }];

    // river water ribbons
    if !rivers.is_empty() {
        let mut ribbon = Mesh::new();
        let wc = pal.water;
        let half_w = size / res as f32 * 1.6;
        for path in &rivers {
            for i in 0..path.len().saturating_sub(1) {
                let (a, b) = (path[i], path[i + 1]);
                let dir = (b - a).normalize_or(Vec3::X);
                let side = dir.cross(Vec3::Y).normalize_or(Vec3::Z) * half_w;
                ribbon.add_flat_quad(a - side, b - side, b + side, a + side, wc);
            }
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

    // water plane
    if p.water_level > 0.0 && water_h > h_min {
        let mut water = Mesh::new();
        let s = size / 2.0 * 0.995;
        let wc = pal.water;
        let a = water.push_vertex(Vec3::new(-s, water_h, -s), Vec3::Y, wc);
        let b = water.push_vertex(Vec3::new(s, water_h, -s), Vec3::Y, wc);
        let c = water.push_vertex(Vec3::new(s, water_h, s), Vec3::Y, wc);
        let d = water.push_vertex(Vec3::new(-s, water_h, s), Vec3::Y, wc);
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

    // scatter vegetation & rocks as GPU instances: a handful of unit-scale
    // variant meshes stamped many times (EXT_mesh_gpu_instancing) — dense
    // coverage at a fraction of the vertex data
    let mut instanced: Vec<crate::gltf::InstancedPart> = Vec::new();
    if p.scatter {
        let mut variants: Vec<(Mesh, f32)> = Vec::new(); // (unit mesh, base scale)
        for _ in 0..3 {
            variants.push((tree_billboardless(&mut r, pal, 1.0), size * 0.022));
        }
        for _ in 0..2 {
            variants.push((
                crate::generators::rock::rock_mesh(&mut r, pal, 1.0, 0.5),
                size * 0.014,
            ));
        }
        let mut placements: Vec<Vec<(Vec3, glam::Quat, Vec3)>> = vec![Vec::new(); variants.len()];
        // sample the (possibly eroded/carved) grid, not the raw height fn
        let sample_grid = |x: f32, z: f32| -> f32 {
            let fx = ((x / size + 0.5) * res as f32).clamp(0.0, res as f32 - 0.001);
            let fz = ((z / size + 0.5) * res as f32).clamp(0.0, res as f32 - 0.001);
            let (ix, iz) = (fx as usize, fz as usize);
            let (dx, dz) = (fx - ix as f32, fz - iz as f32);
            let g = |ix: usize, iz: usize| grid[iz * (res + 1) + ix];
            g(ix, iz) * (1.0 - dx) * (1.0 - dz)
                + g(ix + 1, iz) * dx * (1.0 - dz)
                + g(ix, iz + 1) * (1.0 - dx) * dz
                + g(ix + 1, iz + 1) * dx * dz
        };
        let density = p.scatter_density.clamp(0.1, 8.0);
        let count = (size * size * 0.045 * density) as usize;
        for _ in 0..count {
            let x = range(&mut r, -size * 0.48, size * 0.48);
            let z = range(&mut r, -size * 0.48, size * 0.48);
            let h = sample_grid(x, z);
            let probe = 0.6;
            let s = ((sample_grid(x + probe, z) - h).abs()
                + (sample_grid(x, z + probe) - h).abs())
                / probe;
            let t_alt = ((h - h_min) / (h_max - h_min + 1e-6)).clamp(0.0, 1.0);
            // grid indices for the path mask
            let gx = (((x / size + 0.5) * res as f32) as usize).min(res);
            let gz = (((z / size + 0.5) * res as f32) as usize).min(res);
            let on_path = path_mask[gz * (res + 1) + gx] > 0.15;
            if h < water_h + amp * 0.03 || s > 0.7 || t_alt > 0.75 || on_path {
                continue;
            }
            let vi = if r.gen_bool(0.75) {
                r.gen_range(0..3usize)
            } else {
                3 + r.gen_range(0..2usize)
            };
            let scale = range(&mut r, 0.5, 1.15) * variants[vi].1;
            let yaw = range(&mut r, 0.0, core::f32::consts::TAU);
            placements[vi].push((
                Vec3::new(x, h - 0.05, z),
                glam::Quat::from_rotation_y(yaw),
                Vec3::splat(scale),
            ));
        }
        for (vi, (mesh, _)) in variants.into_iter().enumerate() {
            if placements[vi].is_empty() {
                continue;
            }
            instanced.push(crate::gltf::InstancedPart {
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
        "terrain",
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

/// Deterministic droplet-based hydraulic erosion on the height grid.
fn erode(grid: &mut [f32], res: usize, amount: f32, seed: u64) {
    let mut r = rng(seed ^ 0xE70DE);
    let n = res + 1;
    // the simulation constants assume heights normalized to 0..1
    let h_min = grid.iter().cloned().fold(f32::INFINITY, f32::min);
    let h_max = grid.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let span = (h_max - h_min).max(1e-6);
    for h in grid.iter_mut() {
        *h = (*h - h_min) / span;
    }
    let drops = (((n * n) as f32) * 1.5 * amount) as usize;
    let drops = drops.min(150_000);
    // hard budget: no cell may move more than this from its original height
    // (stops feedback loops from drilling needle pits / stacking spires)
    let orig: Vec<f32> = grid.to_vec();
    let budget = 0.05 + 0.07 * amount;
    let (inertia, capacity_f, min_slope) = (0.05f32, 4.0f32, 0.01f32);
    let (deposit_f, erode_f, evaporate, gravity) = (0.3f32, 0.3f32, 0.02f32, 4.0f32);
    for _ in 0..drops {
        let mut px = range(&mut r, 0.0, (res - 1) as f32);
        let mut pz = range(&mut r, 0.0, (res - 1) as f32);
        let (mut dx, mut dz) = (0.0f32, 0.0f32);
        let mut vel = 1.0f32;
        let mut water = 1.0f32;
        let mut sediment = 0.0f32;
        for _ in 0..40 {
            let (ix, iz) = (px as usize, pz as usize);
            if ix >= res || iz >= res {
                break;
            }
            let (fx, fz) = (px - ix as f32, pz - iz as f32);
            let g = |ix: usize, iz: usize| grid[iz * n + ix];
            let (h00, h10, h01, h11) = (g(ix, iz), g(ix + 1, iz), g(ix, iz + 1), g(ix + 1, iz + 1));
            let grad_x = (h10 - h00) * (1.0 - fz) + (h11 - h01) * fz;
            let grad_z = (h01 - h00) * (1.0 - fx) + (h11 - h10) * fx;
            let h_old = h00 * (1.0 - fx) * (1.0 - fz)
                + h10 * fx * (1.0 - fz)
                + h01 * (1.0 - fx) * fz
                + h11 * fx * fz;
            dx = dx * inertia - grad_x * (1.0 - inertia);
            dz = dz * inertia - grad_z * (1.0 - inertia);
            let len = (dx * dx + dz * dz).sqrt();
            if len < 1e-8 {
                break;
            }
            dx /= len;
            dz /= len;
            px += dx;
            pz += dz;
            if px < 0.0 || pz < 0.0 || px >= (res - 1) as f32 || pz >= (res - 1) as f32 {
                break;
            }
            let (jx, jz) = (px as usize, pz as usize);
            let (gx, gz) = (px - jx as f32, pz - jz as f32);
            let h_new = g(jx, jz) * (1.0 - gx) * (1.0 - gz)
                + g(jx + 1, jz) * gx * (1.0 - gz)
                + g(jx, jz + 1) * (1.0 - gx) * gz
                + g(jx + 1, jz + 1) * gx * gz;
            let dh = h_new - h_old;
            let capacity = (-dh).max(min_slope) * vel * water * capacity_f;
            // 3×3 brush: point splats leave needle spikes on the mesh
            let mut splat = |grid: &mut [f32], amt: f32| {
                for (dxi, dzi, w) in [
                    (0i64, 0i64, 0.40f32),
                    (1, 0, 0.10),
                    (-1, 0, 0.10),
                    (0, 1, 0.10),
                    (0, -1, 0.10),
                    (1, 1, 0.05),
                    (-1, -1, 0.05),
                    (1, -1, 0.05),
                    (-1, 1, 0.05),
                ] {
                    let (gx, gz) = (ix as i64 + dxi, iz as i64 + dzi);
                    if gx >= 0 && gz >= 0 && gx < n as i64 && gz < n as i64 {
                        let idx = gz as usize * n + gx as usize;
                        grid[idx] = (grid[idx] + amt * w)
                            .clamp(orig[idx] - budget, orig[idx] + budget);
                    }
                }
            };
            // cap per-step transfer: unbounded deposits build walls whose
            // growing dh feeds back exponentially
            const MAX_STEP: f32 = 0.004;
            if sediment > capacity || dh > 0.0 {
                let amt = if dh > 0.0 {
                    dh.min(sediment)
                } else {
                    (sediment - capacity) * deposit_f
                }
                .min(MAX_STEP);
                splat(grid, amt);
                sediment -= amt;
            } else {
                let amt = ((capacity - sediment) * erode_f).min(-dh).min(MAX_STEP);
                splat(grid, -amt);
                sediment += amt;
            }
            vel = (vel * vel + (-dh) * gravity).max(0.0).sqrt().min(8.0);
            water *= 1.0 - evaporate;
            if water < 0.01 {
                break;
            }
        }
    }
    // one gentle 3x3 smoothing pass so gullies read as flow lines, not noise
    let snapshot = grid.to_vec();
    for iz in 1..res {
        for ix in 1..res {
            let idx = iz * n + ix;
            let sum = snapshot[idx - 1]
                + snapshot[idx + 1]
                + snapshot[idx - n]
                + snapshot[idx + n];
            grid[idx] = snapshot[idx] * 0.6 + sum * 0.1;
        }
    }
    for h in grid.iter_mut() {
        *h = *h * span + h_min;
    }
}

/// Flatten the terrain along a Catmull-Rom spline and record a dirt mask.
fn flatten_path(
    grid: &mut [f32],
    mask: &mut [f32],
    spec: &crate::recipe::PathSpec,
    res: usize,
    size: f32,
) {
    if spec.points.len() < 2 {
        return;
    }
    let n = res + 1;
    let pts: Vec<Vec3> = spec
        .points
        .iter()
        .map(|p| Vec3::new(p[0].clamp(-size / 2.0, size / 2.0), 0.0, p[1].clamp(-size / 2.0, size / 2.0)))
        .collect();
    let cell = size / res as f32;
    let width = spec.width.max(cell);
    let samples = (pts.len() * 24).max(48);
    // sample the spline, average nearby grid heights per sample for a smooth
    // road profile, then blend grid vertices toward it
    let sample_pt = |t: f32| -> Vec3 {
        // Catmull-Rom (duplicated endpoints)
        let seg = (pts.len() - 1) as f32;
        let x = (t.clamp(0.0, 1.0) * seg).min(seg - 1e-4);
        let i = x as usize;
        let u = x - i as f32;
        let p0 = pts[i.saturating_sub(1)];
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = pts[(i + 2).min(pts.len() - 1)];
        0.5 * ((2.0 * p1)
            + (-p0 + p2) * u
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u * u
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * u * u * u)
    };
    let gidx = |x: f32, z: f32| -> (usize, usize) {
        (
            (((x / size + 0.5) * res as f32).round() as usize).min(res),
            (((z / size + 0.5) * res as f32).round() as usize).min(res),
        )
    };
    // smoothed road height along the spline
    let mut road_h: Vec<f32> = (0..=samples)
        .map(|i| {
            let p = sample_pt(i as f32 / samples as f32);
            let (gx, gz) = gidx(p.x, p.z);
            grid[gz * n + gx]
        })
        .collect();
    for _ in 0..6 {
        let prev = road_h.clone();
        for i in 1..road_h.len() - 1 {
            road_h[i] = (prev[i - 1] + prev[i] * 2.0 + prev[i + 1]) / 4.0;
        }
    }
    let reach = (width / cell).ceil() as i64 + 2;
    for (i, h) in road_h.iter().enumerate() {
        let p = sample_pt(i as f32 / samples as f32);
        let (cx, cz) = gidx(p.x, p.z);
        for dz in -reach..=reach {
            for dxi in -reach..=reach {
                let (gx, gz) = (cx as i64 + dxi, cz as i64 + dz);
                if gx < 0 || gz < 0 || gx > res as i64 || gz > res as i64 {
                    continue;
                }
                let wx = (gx as f32 / res as f32 - 0.5) * size;
                let wz = (gz as f32 / res as f32 - 0.5) * size;
                let d = ((wx - p.x).powi(2) + (wz - p.z).powi(2)).sqrt();
                let t = (1.0 - (d / width).powi(2)).clamp(0.0, 1.0);
                if t <= 0.0 {
                    continue;
                }
                let idx = gz as usize * n + gx as usize;
                grid[idx] = grid[idx] * (1.0 - t * 0.9) + h * t * 0.9;
                mask[idx] = mask[idx].max(t);
            }
        }
    }
}

/// Trace a river downhill from a high spring, carve its bed into the grid,
/// and return the water-ribbon centerline (or None if too short).
fn carve_river(
    grid: &mut [f32],
    res: usize,
    size: f32,
    amp: f32,
    water_h: f32,
    seed: u64,
) -> Option<Vec<Vec3>> {
    let mut r = rng(seed ^ 0x81E5);
    let n = res + 1;
    // spring: highest of 48 random probes
    let mut best = (0usize, 0usize, f32::NEG_INFINITY);
    for _ in 0..48 {
        let ix = r.gen_range(res / 8..res - res / 8);
        let iz = r.gen_range(res / 8..res - res / 8);
        let h = grid[iz * n + ix];
        if h > best.2 {
            best = (ix, iz, h);
        }
    }
    let (mut ix, mut iz, _) = best;
    let mut path: Vec<(usize, usize)> = vec![(ix, iz)];
    for _ in 0..res * 4 {
        let h = grid[iz * n + ix];
        if h <= water_h {
            break;
        }
        // steepest descent over the 8-neighborhood
        let mut next = None;
        let mut lowest = h;
        for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)]
        {
            let (jx, jz) = (ix as i64 + dx, iz as i64 + dz);
            if jx < 0 || jz < 0 || jx > res as i64 || jz > res as i64 {
                continue;
            }
            let hh = grid[jz as usize * n + jx as usize];
            if hh < lowest {
                lowest = hh;
                next = Some((jx as usize, jz as usize));
            }
        }
        match next {
            Some((jx, jz)) => {
                ix = jx;
                iz = jz;
                path.push((ix, iz));
            }
            None => break, // local basin
        }
    }
    if path.len() < res / 8 {
        return None;
    }
    // carve: bed height decreases monotonically downstream
    let cell = size / res as f32;
    let depth = amp * 0.05;
    let radius = 2i64;
    let mut bed = grid[path[0].1 * n + path[0].0];
    let mut ribbon = Vec::with_capacity(path.len());
    for (i, &(cx, cz)) in path.iter().enumerate() {
        let t = i as f32 / path.len() as f32;
        bed = bed.min(grid[cz * n + cx]) - depth * 0.08;
        let target = bed - depth * (0.4 + 0.6 * t);
        for dz in -radius..=radius {
            for dxi in -radius..=radius {
                let (gx, gz) = (cx as i64 + dxi, cz as i64 + dz);
                if gx < 0 || gz < 0 || gx > res as i64 || gz > res as i64 {
                    continue;
                }
                let d2 = (dxi * dxi + dz * dz) as f32;
                let w = (1.0 - d2 / ((radius * radius) as f32 + 1.0)).clamp(0.0, 1.0);
                let idx = gz as usize * n + gx as usize;
                grid[idx] = grid[idx].min(grid[idx] * (1.0 - w) + target * w);
            }
        }
        let wx = (cx as f32 / res as f32 - 0.5) * size;
        let wz = (cz as f32 / res as f32 - 0.5) * size;
        ribbon.push(Vec3::new(wx, target + depth * 0.45, wz));
        let _ = cell;
    }
    Some(ribbon)
}

/// Cheap distant tree used for terrain scattering (cone or blob canopy).
pub(crate) fn tree_billboardless(r: &mut Rand, pal: &Palette, s: f32) -> Mesh {
    let mut m = Mesh::new();
    let trunk_h = s * range(r, 0.9, 1.4);
    let trunk = crate::mesh::tube(
        &[
            (Vec3::ZERO, s * 0.10),
            (Vec3::new(0.0, trunk_h, 0.0), s * 0.06),
        ],
        5,
        |_| pal.trunk,
    );
    m.merge(&mut trunk.clone());
    let f = pal.foliage[r.gen_range(0..pal.foliage.len())];
    if r.gen_bool(0.5) {
        // cone pine
        for (i, (radius, y)) in [(0.55, 0.0), (0.42, 0.45), (0.28, 0.85)].iter().enumerate() {
            let mut cone = crate::mesh::lathe(
                &[
                    (s * radius, trunk_h + s * y * 1.6),
                    (0.0, trunk_h + s * (y * 1.6 + 0.9)),
                ],
                7,
                |_, _| vary(f, 0.12, (i as f32) * 0.37 % 1.0),
            );
            cone = to_flat_shaded(&cone);
            m.merge(&cone);
        }
    } else {
        let mut blob = crate::mesh::icosphere(s * 0.62, 1, f);
        blob = to_flat_shaded(&blob);
        blob.transform(Mat4::from_translation(Vec3::new(0.0, trunk_h + s * 0.5, 0.0)));
        m.merge(&blob);
    }
    m
}

fn m_push_quad(m: &mut Mesh, a: u32, b: u32, c: u32, d: u32, parity: usize) {
    // alternate diagonal for a nicer wireframe pattern
    if parity % 2 == 0 {
        m.push_tri(a, c, b);
        m.push_tri(b, c, d);
    } else {
        m.push_tri(a, c, d);
        m.push_tri(a, d, b);
    }
}
