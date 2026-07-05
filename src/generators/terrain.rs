//! Terrain / map chunk generator: warped-fBm + ridged heightfield, biome
//! coloring by altitude & slope, water plane, optional scattered vegetation.

use glam::{Mat4, Vec3};
use rand::Rng;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, to_flat_shaded};
use crate::noise::Noise2;
use crate::palette::{Palette, ramp, vary};
use crate::recipe::TerrainParams;

use super::{Rand, range, rng};

pub fn generate(p: &TerrainParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let n = Noise2::new(p.seed);
    let res = p.resolution.clamp(16, 512) as usize;
    let size = p.size.max(4.0);
    let amp = size * 0.16 * p.mountainousness.clamp(0.05, 3.0);

    let height = |x: f32, z: f32| -> f32 {
        let (u, v) = (x / size * 3.0, z / size * 3.0);
        let base = n.warped_fbm(u, v, 6, 0.9);
        let ridge = n.ridged(u * 0.8 + 13.7, v * 0.8 + 4.2, 5) - 0.65;
        // ridges dominate where the base terrain is already high
        let mask = ((base + 0.35) * 1.4).clamp(0.0, 1.0);
        (base * 0.6 + ridge * mask * 0.9) * amp
    };

    // heightfield grid
    let mut grid = vec![0.0f32; (res + 1) * (res + 1)];
    let mut h_min = f32::INFINITY;
    let mut h_max = f32::NEG_INFINITY;
    for iz in 0..=res {
        for ix in 0..=res {
            let x = (ix as f32 / res as f32 - 0.5) * size;
            let z = (iz as f32 / res as f32 - 0.5) * size;
            let h = height(x, z);
            grid[iz * (res + 1) + ix] = h;
            h_min = h_min.min(h);
            h_max = h_max.max(h);
        }
    }
    let water_h = h_min + (h_max - h_min) * p.water_level.clamp(0.0, 0.95);

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
            let c = vary(c, 0.10, n.sample(x * 2.1 + 31.0, z * 2.1 + 17.0) * 0.5 + 0.5);
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
    {
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

    let mut parts = vec![Part {
        mesh: ground,
        material: Material { roughness: 0.95, ..Default::default() },
    }];

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

    // scatter vegetation & rocks on gentle, dry ground
    if p.scatter {
        let mut veg = Mesh::new();
        let mut rocks = Mesh::new();
        let count = (size * size * 0.045) as usize;
        for _ in 0..count {
            let x = range(&mut r, -size * 0.48, size * 0.48);
            let z = range(&mut r, -size * 0.48, size * 0.48);
            let h = height(x, z);
            let slope_probe = 0.6;
            let s =
                ((height(x + slope_probe, z) - h).abs() + (height(x, z + slope_probe) - h).abs())
                    / slope_probe;
            let t_alt = ((h - h_min) / (h_max - h_min + 1e-6)).clamp(0.0, 1.0);
            if h < water_h + amp * 0.03 || s > 0.7 || t_alt > 0.75 {
                continue;
            }
            let at = Vec3::new(x, h - 0.05, z);
            if r.gen_bool(0.75) {
                let scale = range(&mut r, 0.5, 1.15) * size * 0.022;
                let mut t = tree_billboardless(&mut r, pal, scale);
                t.transform(Mat4::from_translation(at));
                veg.merge(&t);
            } else {
                let rock_size = range(&mut r, 0.3, 0.9) * size * 0.014;
                let mut rk = crate::generators::rock::rock_mesh(&mut r, pal, rock_size, 0.5);
                rk.transform(Mat4::from_translation(at));
                rocks.merge(&rk);
            }
        }
        if veg.vertex_count() > 0 {
            parts.push(Part {
                mesh: veg,
                material: Material { roughness: 0.9, ..Default::default() },
            });
        }
        if rocks.vertex_count() > 0 {
            parts.push(Part {
                mesh: rocks,
                material: Material { roughness: 0.95, ..Default::default() },
            });
        }
    }

    Asset::static_mesh(
        "terrain",
        parts,
        Some(Physics {
            collider: Collider::Heightfield,
            mass: 0.0,
            friction: 0.9,
            restitution: 0.05,
        }),
    )
}

/// Cheap distant tree used for terrain scattering (cone or blob canopy).
fn tree_billboardless(r: &mut Rand, pal: &Palette, s: f32) -> Mesh {
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
