//! Crystal clusters: emissive faceted prisms rising from a rock base.

use glam::{Mat4, Quat, Vec3};

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::Mesh;
use crate::palette::{Palette, lerp, vary};
use crate::recipe::CrystalParams;

use super::{Rand, range, rng};

// The 6.28 rotation range below is an intentional, determinism-locked magic
// value (not a stand-in for TAU) — changing it would alter existing assets.
#[allow(clippy::approx_constant)]
pub fn generate(p: &CrystalParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let size = p.size.clamp(0.2, 10.0);

    // rocky base
    let base = crate::generators::rock::rock_mesh(&mut r, pal, size * 0.75, 0.6);

    let mut shards = Mesh::new();
    let count = p.count.clamp(3, 24);
    for i in 0..count {
        let big = i == 0;
        let s = if big {
            size
        } else {
            size * range(&mut r, 0.3, 0.7)
        };
        let mut c = prism(&mut r, pal, s);
        let a = i as f32 / count as f32 * core::f32::consts::TAU + range(&mut r, -0.4, 0.4);
        let rad = if big {
            0.0
        } else {
            size * range(&mut r, 0.25, 0.62)
        };
        let tilt = Quat::from_axis_angle(
            Vec3::new(a.cos(), 0.0, a.sin())
                .cross(Vec3::Y)
                .normalize_or(Vec3::X),
            if big {
                range(&mut r, -0.12, 0.12)
            } else {
                range(&mut r, 0.15, 0.55)
            },
        );
        c.transform(Mat4::from_rotation_translation(
            tilt * Quat::from_rotation_y(range(&mut r, 0.0, 6.28)),
            Vec3::new(a.cos() * rad, size * 0.28, a.sin() * rad),
        ));
        shards.merge(&c);
    }

    let glow = pal.accent;
    Asset::static_mesh(
        "crystal",
        vec![
            Part {
                mesh: base,
                material: Material {
                    roughness: 0.95,
                    ..Default::default()
                },
            },
            Part {
                mesh: shards,
                material: Material {
                    roughness: 0.15,
                    metallic: 0.25,
                    emissive: glow * 0.55,
                    ..Default::default()
                },
            },
        ],
        Some(Physics {
            collider: Collider::Sphere { radius: size },
            mass: 0.0,
            friction: 0.7,
            restitution: 0.2,
        }),
    )
}

/// Hexagonal crystal shard: stretched prism with a pointed tip, flat facets,
/// brighter core color near the tip.
fn prism(r: &mut Rand, pal: &Palette, s: f32) -> Mesh {
    let mut m = Mesh::new();
    let sides = 6;
    let radius = s * range(r, 0.14, 0.2);
    let body_h = s * range(r, 0.8, 1.3);
    let tip_h = body_h + s * range(r, 0.25, 0.4);
    let deep = lerp(pal.accent, Vec3::ZERO, 0.55);
    let bright = lerp(pal.accent, Vec3::ONE, 0.35);
    let ring = |y: f32, rad: f32| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let a = i as f32 / sides as f32 * core::f32::consts::TAU;
                Vec3::new(a.cos() * rad, y, a.sin() * rad)
            })
            .collect()
    };
    let bottom = ring(-s * 0.2, radius * 0.8);
    let top = ring(body_h, radius);
    let tip = Vec3::new(0.0, tip_h, 0.0);
    for i in 0..sides {
        let j = (i + 1) % sides;
        let cb = vary(deep, 0.1, i as f32 / sides as f32);
        let ct = vary(bright, 0.1, i as f32 / sides as f32);
        // side quad (flat facet, graded color via two tris with own verts)
        let n = (top[i] - bottom[i])
            .cross(bottom[j] - bottom[i])
            .normalize_or(Vec3::X);
        let v0 = m.push_vertex(bottom[i], n, cb);
        let v1 = m.push_vertex(bottom[j], n, cb);
        let v2 = m.push_vertex(top[j], n, ct);
        let v3 = m.push_vertex(top[i], n, ct);
        m.push_tri(v0, v2, v1);
        m.push_tri(v0, v3, v2);
        // tip facet
        m.add_flat_tri(top[i], tip, top[j], bright);
    }
    m
}
