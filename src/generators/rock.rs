//! Rocks & boulders: noise-displaced icospheres, flattened base, faceted
//! shading, slope-aware coloring (moss on top).

use glam::Vec3;
use rand::{Rng, RngCore};

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, icosphere, to_flat_shaded};
use crate::noise::Noise2;
use crate::palette::{Palette, lerp, vary};
use crate::recipe::RockParams;

use super::{Rand, range, rng};

pub fn generate(p: &RockParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let size = p.size.clamp(0.1, 20.0);
    let mut m = Mesh::new();
    // main boulder + a couple of satellites for composition
    let main = rock_mesh(&mut r, pal, size, p.jaggedness);
    m.merge(&main);
    let satellites = r.gen_range(2..=4);
    for _ in 0..satellites {
        let s = size * range(&mut r, 0.2, 0.45);
        let mut sat = rock_mesh(&mut r, pal, s, p.jaggedness * 1.2);
        let a = range(&mut r, 0.0, core::f32::consts::TAU);
        sat.translate(Vec3::new(a.cos(), 0.0, a.sin()) * size * range(&mut r, 1.15, 1.6));
        m.merge(&sat);
    }
    Asset::static_mesh(
        "rock",
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.97,
                ..Default::default()
            },
        }],
        Some(Physics {
            collider: Collider::Sphere { radius: size },
            mass: 0.0,
            friction: 0.85,
            restitution: 0.1,
        }),
    )
}

/// A single displaced-icosphere boulder sitting on y=0.
pub fn rock_mesh(r: &mut Rand, pal: &Palette, size: f32, jaggedness: f32) -> Mesh {
    let n = Noise2::new(r.next_u64());
    let j = jaggedness.clamp(0.0, 1.5);
    let mut m = icosphere(size, 2, pal.rock[0]);
    let squash = Vec3::new(
        range(r, 0.85, 1.2),
        range(r, 0.6, 0.85),
        range(r, 0.85, 1.2),
    );
    // random anisotropy axis: rocks look hewn, not blobby
    let axis = Vec3::new(
        range(r, -1.0, 1.0),
        range(r, -0.2, 0.2),
        range(r, -1.0, 1.0),
    )
    .normalize_or(Vec3::X);
    for p in m.positions.iter_mut() {
        let d = n.fbm(
            p.x / size * 2.2 + 3.1,
            (p.z + p.y * 0.7) / size * 2.2,
            4,
            2.2,
            0.55,
        );
        let sharp = n.sample(p.x / size * 5.0, p.z / size * 5.0);
        // faceted chisel cuts: quantize the sharp component
        let chisel = (sharp * 2.0).round() / 2.0;
        let stretch = 1.0 + p.normalize_or(Vec3::Y).dot(axis).abs() * 0.35;
        let disp = (1.0 + d * 0.7 * j + chisel * 0.3 * j) * stretch;
        *p = *p * disp * squash;
    }
    // flatten & sink base
    let (lo, _) = m.bounds();
    for p in m.positions.iter_mut() {
        p.y -= lo.y * 0.25;
        if p.y < 0.0 {
            p.y *= 0.25;
        }
    }
    m.recompute_smooth_normals();
    // color: strata + moss on upward faces
    let moss = lerp(pal.foliage[0], pal.rock[0], 0.2);
    for i in 0..m.positions.len() {
        let up = m.normals[i].y.max(0.0).powf(3.0);
        let strata = (n.sample(m.positions[i].y / size * 3.0 + 9.0, m.positions[i].x / size) * 0.5
            + 0.5)
            .clamp(0.0, 1.0);
        let base = lerp(pal.rock[0] * 1.1, pal.rock[1] * 0.75, strata);
        m.colors[i] = vary(lerp(base, moss, up * 0.5), 0.12, strata);
    }
    to_flat_shaded(&m)
}
