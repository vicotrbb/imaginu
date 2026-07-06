//! Small props: barrel, crate, lantern (emissive), campfire.

use glam::Vec3;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, cuboid, icosphere, lathe, to_flat_shaded, tube};
use crate::palette::{Palette, lerp, srgb, vary};
use crate::recipe::{PropKind, PropParams};

use super::{range, rng};

pub fn generate(p: &PropParams, pal: &Palette) -> Asset {
    match p.prop {
        PropKind::Barrel => barrel(p, pal),
        PropKind::Crate => krate(p, pal),
        PropKind::Lantern => lantern(p, pal),
        PropKind::Campfire => campfire(p, pal),
    }
}

fn barrel(p: &PropParams, pal: &Palette) -> Asset {
    let s = p.size.clamp(0.2, 4.0);
    let wood = pal.trunk;
    let profile = [
        (s * 0.30, 0.0),
        (s * 0.38, s * 0.18),
        (s * 0.42, s * 0.45),
        (s * 0.38, s * 0.72),
        (s * 0.30, s * 0.9),
        (0.0, s * 0.9),
    ];
    let body = lathe(&profile, 12, |ri, a| {
        // vertical stave stripes via angular banding
        let stripe = ((a * 12.0 / core::f32::consts::TAU).floor() as i32 % 2) as f32;
        vary(wood * (0.9 + stripe * 0.16), 0.03, ri as f32 * 0.3)
    });
    let mut m = to_flat_shaded(&body);
    // iron hoops
    let iron = srgb(70, 74, 80);
    for y in [s * 0.16, s * 0.74] {
        let hoop = lathe(
            &[
                (s * 0.385, y - s * 0.03),
                (s * 0.415, y),
                (s * 0.385, y + s * 0.03),
            ],
            12,
            |_, _| iron,
        );
        m.merge(&to_flat_shaded(&hoop));
    }
    Asset::static_mesh(
        "barrel",
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.8,
                metallic: 0.15,
                ..Default::default()
            },
        }],
        Some(Physics {
            collider: Collider::Capsule {
                radius: s * 0.42,
                height: s * 0.9,
            },
            mass: 30.0 * s,
            friction: 0.6,
            restitution: 0.3,
        }),
    )
}

fn krate(p: &PropParams, pal: &Palette) -> Asset {
    let s = p.size.clamp(0.2, 4.0) * 0.5;
    let wood = lerp(pal.trunk, srgb(190, 160, 110), 0.45);
    let frame = pal.trunk * 0.8;
    let mut m = cuboid(Vec3::new(0.0, s, 0.0), Vec3::splat(s * 0.94), wood);
    let e = s * 0.12;
    // edge frame beams
    for &(x, y, z, hx, hy, hz) in &[
        // 4 vertical
        (-1.0, 0.0, -1.0, e, s, e),
        (1.0, 0.0, -1.0, e, s, e),
        (-1.0, 0.0, 1.0, e, s, e),
        (1.0, 0.0, 1.0, e, s, e),
        // 4 top + 4 bottom
        (0.0, 1.0, -1.0, s, e, e),
        (0.0, 1.0, 1.0, s, e, e),
        (-1.0, 1.0, 0.0, e, e, s),
        (1.0, 1.0, 0.0, e, e, s),
        (0.0, -1.0, -1.0, s, e, e),
        (0.0, -1.0, 1.0, s, e, e),
        (-1.0, -1.0, 0.0, e, e, s),
        (1.0, -1.0, 0.0, e, e, s),
    ] {
        m.merge(&cuboid(
            Vec3::new(x * s, s + y * s, z * s),
            Vec3::new(hx, hy, hz),
            frame,
        ));
    }
    Asset::static_mesh(
        "crate",
        vec![Part {
            mesh: m,
            material: Material {
                roughness: 0.85,
                ..Default::default()
            },
        }],
        Some(Physics {
            collider: Collider::Box {
                half_extents: Vec3::splat(s),
            },
            mass: 20.0 * s,
            friction: 0.7,
            restitution: 0.2,
        }),
    )
}

fn lantern(p: &PropParams, pal: &Palette) -> Asset {
    let s = p.size.clamp(0.2, 6.0);
    let iron = srgb(52, 54, 60);
    let mut post = tube(
        &[
            (Vec3::ZERO, s * 0.055),
            (Vec3::new(0.0, s * 1.55, 0.0), s * 0.04),
            (Vec3::new(s * 0.22, s * 1.72, 0.0), s * 0.03),
            (Vec3::new(s * 0.4, s * 1.68, 0.0), s * 0.025),
        ],
        7,
        |_| iron,
    );
    post.merge(&to_flat_shaded(&cuboid(
        Vec3::new(0.0, s * 0.04, 0.0),
        Vec3::new(s * 0.14, s * 0.04, s * 0.14),
        iron * 1.2,
    )));
    // cage
    let cage_c = Vec3::new(s * 0.4, s * 1.5, 0.0);
    post.merge(&cuboid(
        cage_c + Vec3::Y * s * 0.13,
        Vec3::new(s * 0.09, s * 0.015, s * 0.09),
        iron,
    ));
    post.merge(&cuboid(
        cage_c - Vec3::Y * s * 0.13,
        Vec3::new(s * 0.09, s * 0.015, s * 0.09),
        iron,
    ));
    for (sx, sz) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        post.merge(&cuboid(
            cage_c + Vec3::new(sx * s * 0.08, 0.0, sz * s * 0.08),
            Vec3::new(s * 0.012, s * 0.13, s * 0.012),
            iron,
        ));
    }
    // glowing core
    let glow = lerp(pal.accent, srgb(255, 214, 130), 0.6);
    let mut core = icosphere(s * 0.088, 1, glow);
    core.translate(cage_c);
    Asset::static_mesh(
        "lantern",
        vec![
            Part {
                mesh: post,
                material: Material {
                    roughness: 0.6,
                    metallic: 0.7,
                    ..Default::default()
                },
            },
            Part {
                mesh: core,
                material: Material {
                    roughness: 0.4,
                    emissive: glow * 2.4,
                    ..Default::default()
                },
            },
        ],
        Some(Physics {
            collider: Collider::Capsule {
                radius: s * 0.1,
                height: s * 1.7,
            },
            mass: 0.0,
            friction: 0.5,
            restitution: 0.1,
        }),
    )
}

fn campfire(p: &PropParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let s = p.size.clamp(0.3, 4.0);
    let mut wood = Mesh::new();
    // stone ring
    for i in 0..8 {
        let a = i as f32 / 8.0 * core::f32::consts::TAU;
        let mut st = icosphere(s * range(&mut r, 0.09, 0.13), 1, pal.rock[i % 2]);
        for v in st.positions.iter_mut() {
            v.y *= 0.7;
        }
        st.recompute_smooth_normals();
        let st = to_flat_shaded(&st);
        let mut st2 = st;
        st2.translate(Vec3::new(a.cos() * s * 0.42, s * 0.05, a.sin() * s * 0.42));
        wood.merge(&st2);
    }
    // crossed logs
    for i in 0..4 {
        let a = i as f32 / 4.0 * core::f32::consts::TAU + 0.4;
        let dir = Vec3::new(a.cos(), 0.85, a.sin()).normalize();
        let log = tube(
            &[
                (
                    Vec3::new(a.cos(), 0.0, a.sin()) * s * 0.3 + Vec3::Y * s * 0.05,
                    s * 0.05,
                ),
                (dir * s * 0.1 + Vec3::Y * s * 0.32, s * 0.035),
            ],
            6,
            |_| pal.trunk * 0.7,
        );
        wood.merge(&to_flat_shaded(&log));
    }
    // flame: stacked emissive blobs
    let mut flame = Mesh::new();
    let fire_deep = srgb(255, 96, 18);
    let fire_bright = srgb(255, 208, 90);
    for (i, (radius, y)) in [(0.16, 0.18), (0.11, 0.34), (0.06, 0.48)]
        .iter()
        .enumerate()
    {
        let col = lerp(fire_deep, fire_bright, i as f32 / 2.0);
        let mut b = icosphere(s * radius, 1, col);
        for v in b.positions.iter_mut() {
            v.y *= 1.6;
        }
        b.recompute_smooth_normals();
        let b = to_flat_shaded(&b);
        let mut b2 = b;
        b2.translate(Vec3::new(0.0, s * y + s * 0.05, 0.0));
        flame.merge(&b2);
    }
    Asset::static_mesh(
        "campfire",
        vec![
            Part {
                mesh: wood,
                material: Material {
                    roughness: 0.95,
                    ..Default::default()
                },
            },
            Part {
                mesh: flame,
                material: Material {
                    roughness: 0.5,
                    emissive: srgb(255, 140, 40) * 1.8,
                    ..Default::default()
                },
            },
        ],
        Some(Physics {
            collider: Collider::Sphere { radius: s * 0.5 },
            mass: 0.0,
            friction: 0.8,
            restitution: 0.0,
        }),
    )
}
