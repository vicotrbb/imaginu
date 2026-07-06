//! Tree generator: oak (branching trunk + noisy blob canopy), pine (cone
//! stack), palm (curved trunk + fronds), dead (bare recursive branches).

use glam::{Mat4, Quat, Vec3};
use rand::Rng;

use crate::gltf::{Asset, Collider, Material, Part, Physics};
use crate::mesh::{Mesh, icosphere, lathe, to_flat_shaded, tube};
use crate::noise::Noise2;
use crate::palette::{Palette, vary};
use crate::recipe::{TreeParams, TreeStyle};

use super::{Rand, range, rng};

pub fn generate(p: &TreeParams, pal: &Palette) -> Asset {
    let mut r = rng(p.seed);
    let n = Noise2::new(p.seed ^ 0xABCD);
    let h = p.height.clamp(1.0, 40.0);

    let (wood, foliage) = build_tree(&mut r, &n, pal, h, p.style);

    let parts = vec![
        Part {
            mesh: wood,
            material: Material {
                roughness: 0.9,
                ..Default::default()
            },
        },
        Part {
            mesh: foliage,
            material: Material {
                roughness: 0.85,
                double_sided: true,
                ..Default::default()
            },
        },
    ];
    Asset::static_mesh(
        "tree",
        parts,
        Some(Physics {
            collider: Collider::Capsule {
                radius: h * 0.08,
                height: h,
            },
            mass: 0.0,
            friction: 0.6,
            restitution: 0.1,
        }),
    )
}

fn build_tree(r: &mut Rand, n: &Noise2, pal: &Palette, h: f32, style: TreeStyle) -> (Mesh, Mesh) {
    match style {
        TreeStyle::Oak => oak(r, n, pal, h, true),
        TreeStyle::Pine => pine(r, pal, h),
        TreeStyle::Palm => palm(r, pal, h),
        TreeStyle::Dead => oak(r, n, pal, h, false),
    }
}

/// Recursive branch: returns path-based tube merged into `wood`, canopy
/// blob positions appended to `tips`.
#[allow(clippy::too_many_arguments)]
fn branch(
    r: &mut Rand,
    wood: &mut Mesh,
    tips: &mut Vec<(Vec3, f32)>,
    base: Vec3,
    dir: Vec3,
    len: f32,
    radius: f32,
    depth: u32,
    trunk_color: Vec3,
) {
    let steps = 4;
    let mut path = Vec::with_capacity(steps + 1);
    let mut p = base;
    let mut d = dir;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        path.push((p, radius * (1.0 - t * 0.55)));
        // wander + gentle upward pull
        let wander = Vec3::new(
            range(r, -1.0, 1.0),
            range(r, -0.3, 0.6),
            range(r, -1.0, 1.0),
        ) * 0.25;
        d = (d + wander + Vec3::Y * 0.14).normalize();
        p += d * (len / steps as f32);
    }
    let segs = if depth == 0 { 8 } else { 6 };
    wood.merge(&tube(&path, segs, |_| trunk_color));

    if depth >= 2 || radius < 0.03 {
        tips.push((path.last().unwrap().0, len * 0.55));
        return;
    }
    let kids = if depth == 0 {
        r.gen_range(3..=4)
    } else {
        r.gen_range(2..=3)
    };
    for _ in 0..kids {
        let spread = Quat::from_axis_angle(
            Vec3::new(
                range(r, -1.0, 1.0),
                range(r, -0.2, 0.4),
                range(r, -1.0, 1.0),
            )
            .normalize_or(Vec3::X),
            range(r, 0.4, 0.9),
        );
        let nd = (spread * d).normalize();
        let t = range(r, 0.55, 1.0);
        let start = path[((steps as f32) * t) as usize].0;
        let child_len = len * range(r, 0.5, 0.7);
        let child_radius = radius * range(r, 0.5, 0.65);
        branch(
            r,
            wood,
            tips,
            start,
            nd,
            child_len,
            child_radius,
            depth + 1,
            trunk_color,
        );
    }
}

fn oak(r: &mut Rand, n: &Noise2, pal: &Palette, h: f32, leafy: bool) -> (Mesh, Mesh) {
    let mut wood = Mesh::new();
    let mut tips = Vec::new();
    let lean = Vec3::new(range(r, -0.12, 0.12), 1.0, range(r, -0.12, 0.12)).normalize();
    let bark = if leafy {
        pal.trunk
    } else {
        // sun-bleached dead wood
        pal.trunk * 0.9 + Vec3::splat(0.09)
    };
    branch(
        r,
        &mut wood,
        &mut tips,
        Vec3::ZERO,
        lean,
        h * 0.55,
        h * 0.055,
        0,
        bark,
    );
    // root flare
    let flare = lathe(
        &[(h * 0.10, 0.0), (h * 0.065, h * 0.06), (h * 0.05, h * 0.14)],
        8,
        |_, _| bark * 0.85,
    );
    wood.merge(&flare);

    let mut foliage = Mesh::new();
    if leafy {
        for (i, &(tip, s)) in tips.iter().enumerate() {
            let base_col = pal.foliage[i % pal.foliage.len()];
            let mut blob = icosphere(s.max(h * 0.10) * 1.15, 2, base_col);
            // noise-displace along normals for organic clumps
            for (vi, p) in blob.positions.iter_mut().enumerate() {
                let d = n.fbm(p.x * 1.3 + i as f32 * 7.0, p.z * 1.3 + p.y, 4, 2.0, 0.5);
                let nr = blob.normals[vi];
                *p += nr * d * s * 0.45;
                *p += Vec3::new(0.0, d.abs() * s * 0.1, 0.0);
            }
            blob.recompute_smooth_normals();
            for (vi, c) in blob.colors.iter_mut().enumerate() {
                // upper faces lighter — fake sky light baked in
                let up = blob.normals[vi].y * 0.5 + 0.5;
                *c = vary(*c * (0.75 + up * 0.5), 0.10, (vi as f32 * 0.61) % 1.0);
            }
            let mut blob = to_flat_shaded(&blob);
            blob.translate(tip + Vec3::new(0.0, s * 0.25, 0.0));
            foliage.merge(&blob);
        }
    } else {
        // dead tree: extend every tip with 2-3 attached, tapering twigs
        let twig_bark = pal.trunk * 0.9 + Vec3::splat(0.11);
        for &(tip, s) in tips.iter() {
            let twigs = r.gen_range(2..=3);
            for _ in 0..twigs {
                let d = Vec3::new(range(r, -1.0, 1.0), range(r, 0.5, 1.2), range(r, -1.0, 1.0))
                    .normalize();
                let mid = tip + d * s * 0.4;
                let d2 = (d + Vec3::new(range(r, -0.5, 0.5), 0.3, range(r, -0.5, 0.5))).normalize();
                wood.merge(&tube(
                    &[
                        (tip, s * 0.09),
                        (mid, s * 0.05),
                        (mid + d2 * s * 0.45, s * 0.012),
                    ],
                    4,
                    |_| twig_bark,
                ));
            }
        }
    }
    (wood, foliage)
}

fn pine(r: &mut Rand, pal: &Palette, h: f32) -> (Mesh, Mesh) {
    let mut wood = Mesh::new();
    let trunk_h = h * 0.32;
    wood.merge(&tube(
        &[
            (Vec3::ZERO, h * 0.045),
            (Vec3::new(0.0, trunk_h, 0.0), h * 0.028),
        ],
        7,
        |_| pal.trunk,
    ));
    let mut foliage = Mesh::new();
    let layers = 5 + (h as usize % 3);
    let f = pal.foliage[r.gen_range(0..3)];
    for i in 0..layers {
        let t = i as f32 / layers as f32;
        let y = trunk_h * 0.8 + t * (h - trunk_h * 0.8);
        let radius = h * 0.24 * (1.0 - t * 0.8) * range(r, 0.9, 1.1);
        let cone_h = (h - y) * 0.5 + h * 0.08;
        let col = vary(f * (0.85 + t * 0.35), 0.08, t);
        let mut cone = lathe(
            &[
                (radius * 0.55, y - cone_h * 0.12),
                (radius, y),
                (radius * 0.35, y + cone_h * 0.55),
                (0.0, y + cone_h),
            ],
            9,
            |_, _| col,
        );
        cone = to_flat_shaded(&cone);
        // droop jitter
        cone.transform(Mat4::from_rotation_y(range(r, 0.0, 1.0)));
        foliage.merge(&cone);
    }
    (wood, foliage)
}

fn palm(r: &mut Rand, pal: &Palette, h: f32) -> (Mesh, Mesh) {
    let mut wood = Mesh::new();
    // curved trunk with ring bulges
    let bend = Vec3::new(range(r, -1.0, 1.0), 0.0, range(r, -1.0, 1.0)).normalize()
        * h
        * range(r, 0.16, 0.28);
    let steps = 9;
    let mut path = Vec::new();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let p = Vec3::new(bend.x * t * t, h * t, bend.z * t * t);
        let ring_bulge = if i % 2 == 0 { 1.18 } else { 1.0 };
        path.push((p, h * 0.045 * (1.0 - t * 0.45) * ring_bulge));
    }
    let top = path.last().unwrap().0;
    wood.merge(&to_flat_shaded(&tube(&path, 8, |i| {
        if i % 2 == 0 {
            pal.trunk
        } else {
            pal.trunk * 0.78
        }
    })));

    let mut foliage = Mesh::new();
    let fronds = r.gen_range(9..=11);
    for i in 0..fronds {
        let a = i as f32 / fronds as f32 * core::f32::consts::TAU + range(r, -0.15, 0.15);
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        // alternate long low fronds and short high ones for a full crown
        let long = i % 2 == 0;
        let len = h * if long {
            range(r, 0.5, 0.62)
        } else {
            range(r, 0.34, 0.42)
        };
        let lift = if long { 0.30 } else { 0.55 };
        let droop = if long { 0.85 } else { 0.55 };
        let col = vary(pal.foliage[i % 3], 0.14, i as f32 * 0.37 % 1.0);
        let side = out.cross(Vec3::Y).normalize();
        let mut m = Mesh::new();
        let steps = 7;
        let pt = |t: f32| top + out * (len * t) + Vec3::Y * (len * (lift * t - droop * t * t));
        for sgm in 0..steps {
            let t0 = sgm as f32 / steps as f32;
            let t1 = (sgm + 1) as f32 / steps as f32;
            // leaflet width: widest a third of the way out
            let wdt = |t: f32| len * 0.30 * ((t * 3.2).min(1.0)) * (1.0 - t * 0.92).max(0.04);
            let (p0, p1) = (pt(t0), pt(t1));
            // center spine kink: fold the two halves downward like real fronds
            let sag0 = Vec3::Y * wdt(t0) * 0.55;
            let sag1 = Vec3::Y * wdt(t1) * 0.55;
            let shade0 = 0.72 + t0 * 0.5;
            m.add_flat_quad(
                p0 - side * wdt(t0) - sag0,
                p0,
                p1,
                p1 - side * wdt(t1) - sag1,
                col * shade0,
            );
            m.add_flat_quad(
                p0,
                p0 + side * wdt(t0) + sag0 * 0.0 - sag0,
                p1 + side * wdt(t1) - sag1,
                p1,
                col * (shade0 * 1.08),
            );
        }
        foliage.merge(&m);
    }
    // coconut cluster tucked under the crown
    for k in 0..4 {
        let a = k as f32 * 1.7 + 0.4;
        let mut c = icosphere(h * 0.055, 1, pal.trunk * 0.55);
        c.translate(top + Vec3::new(a.cos(), -0.9, a.sin()).normalize() * h * 0.075);
        foliage.merge(&to_flat_shaded(&c));
    }
    (wood, foliage)
}
