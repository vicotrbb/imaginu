//! Signed-distance-field modeling + surface-nets meshing. Bodies built as
//! ONE smoothly-blended field (capsules, round cones, ellipsoids under
//! polynomial smooth-min) mesh into a single continuous organic surface —
//! no part seams, normals straight from the field gradient. Deterministic:
//! fixed grid, no randomness.

use glam::Vec3;

use crate::mesh::Mesh;

/// Polynomial smooth minimum: C1 blend with fillet radius ~k.
pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = ((k - (a - b).abs()).max(0.0)) / k;
    a.min(b) - h * h * k * 0.25
}

pub fn sd_sphere(p: Vec3, c: Vec3, r: f32) -> f32 {
    (p - c).length() - r
}

/// Exact round cone (capsule with different end radii), after Quilez.
pub fn sd_round_cone(p: Vec3, a: Vec3, b: Vec3, r1: f32, r2: f32) -> f32 {
    let ba = b - a;
    let l2 = ba.dot(ba).max(1e-12);
    let rr = r1 - r2;
    let a2 = l2 - rr * rr;
    let il2 = 1.0 / l2;
    let pa = p - a;
    let y = pa.dot(ba);
    let z = y - l2;
    let x2 = (pa * l2 - ba * y).length_squared();
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = rr.signum() * rr * rr * x2;
    if z.signum() * a2 * z2 > k {
        (x2 + z2).sqrt() * il2 - r2
    } else if y.signum() * a2 * y2 < k {
        (x2 + y2).sqrt() * il2 - r1
    } else {
        ((x2 * a2 * il2).sqrt() + y * rr) * il2 - r1
    }
}

/// Ellipsoid (Quilez approximation — exact enough for blending/meshing).
pub fn sd_ellipsoid(p: Vec3, c: Vec3, r: Vec3) -> f32 {
    let q = p - c;
    let k0 = (q / r).length();
    let k1 = (q / (r * r)).length();
    if k1 < 1e-9 { -r.min_element() } else { k0 * (k0 - 1.0) / k1 }
}

/// Round cone evaluated in a squashed space (elliptical cross-sections):
/// point is scaled about `origin` by 1/scale before evaluation; the result
/// is multiplied by the smallest scale to stay a conservative distance.
pub fn sd_round_cone_scaled(
    p: Vec3,
    a: Vec3,
    b: Vec3,
    r1: f32,
    r2: f32,
    origin: Vec3,
    scale: Vec3,
) -> f32 {
    let q = origin + (p - origin) / scale;
    sd_round_cone(q, a, b, r1, r2) * scale.min_element()
}

/// Naive surface nets over a regular grid: one vertex per sign-changing
/// cell (centroid of edge crossings), one quad per sign-changing lattice
/// edge. Shared vertices → genuinely smooth shading; normals come from the
/// field gradient. `color(p)` picks the albedo at a surface point.
pub fn mesh_field(
    lo: Vec3,
    hi: Vec3,
    cell: f32,
    field: &dyn Fn(Vec3) -> f32,
    color: &dyn Fn(Vec3) -> Vec3,
) -> Mesh {
    let n = |a: f32, b: f32| (((b - a) / cell).ceil() as usize).max(2);
    let (nx, ny, nz) = (n(lo.x, hi.x), n(lo.y, hi.y), n(lo.z, hi.z));
    let corner = |i: usize, j: usize, k: usize| {
        lo + Vec3::new(i as f32, j as f32, k as f32) * cell
    };
    // sample corners
    let sx = nx + 1;
    let sy = ny + 1;
    let sz = nz + 1;
    let idx = |i: usize, j: usize, k: usize| (i * sy + j) * sz + k;
    let mut samples = vec![0.0f32; sx * sy * sz];
    for i in 0..sx {
        for j in 0..sy {
            for k in 0..sz {
                samples[idx(i, j, k)] = field(corner(i, j, k));
            }
        }
    }
    let s = |i: usize, j: usize, k: usize| samples[idx(i, j, k)];

    // one vertex per mixed-sign cell: centroid of edge zero-crossings
    let cidx = |i: usize, j: usize, k: usize| (i * ny + j) * nz + k;
    let mut cell_vert = vec![u32::MAX; nx * ny * nz];
    let mut m = Mesh::new();
    const EDGES: [((usize, usize, usize), (usize, usize, usize)); 12] = [
        ((0, 0, 0), (1, 0, 0)),
        ((0, 1, 0), (1, 1, 0)),
        ((0, 0, 1), (1, 0, 1)),
        ((0, 1, 1), (1, 1, 1)),
        ((0, 0, 0), (0, 1, 0)),
        ((1, 0, 0), (1, 1, 0)),
        ((0, 0, 1), (0, 1, 1)),
        ((1, 0, 1), (1, 1, 1)),
        ((0, 0, 0), (0, 0, 1)),
        ((1, 0, 0), (1, 0, 1)),
        ((0, 1, 0), (0, 1, 1)),
        ((1, 1, 0), (1, 1, 1)),
    ];
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let mut sum = Vec3::ZERO;
                let mut cnt = 0;
                for (ea, eb) in EDGES {
                    let va = s(i + ea.0, j + ea.1, k + ea.2);
                    let vb = s(i + eb.0, j + eb.1, k + eb.2);
                    if (va < 0.0) != (vb < 0.0) {
                        let t = va / (va - vb);
                        let pa = corner(i + ea.0, j + ea.1, k + ea.2);
                        let pb = corner(i + eb.0, j + eb.1, k + eb.2);
                        sum += pa + (pb - pa) * t;
                        cnt += 1;
                    }
                }
                if cnt > 0 {
                    let p = sum / cnt as f32;
                    // gradient normal
                    let e = cell * 0.5;
                    let nrm = Vec3::new(
                        field(p + Vec3::X * e) - field(p - Vec3::X * e),
                        field(p + Vec3::Y * e) - field(p - Vec3::Y * e),
                        field(p + Vec3::Z * e) - field(p - Vec3::Z * e),
                    )
                    .normalize_or(Vec3::Y);
                    cell_vert[cidx(i, j, k)] = m.push_vertex(p, nrm, color(p));
                }
            }
        }
    }

    // quads across every sign-changing lattice edge (interior edges only)
    let mut quad = |a: u32, b: u32, c: u32, d: u32, flip: bool| {
        if a == u32::MAX || b == u32::MAX || c == u32::MAX || d == u32::MAX {
            return;
        }
        if flip {
            m.push_tri(a, b, c);
            m.push_tri(a, c, d);
        } else {
            m.push_tri(a, c, b);
            m.push_tri(a, d, c);
        }
    };
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                // +x edge from corner (i+1? ) — use edge starting at corner (i,j,k)
                // x-direction edge: corners (i,j,k)-(i+1,j,k); adjacent cells
                // (i, j-1..j, k-1..k)
                if j > 0 && k > 0 {
                    let (va, vb) = (s(i, j, k), s(i + 1, j, k));
                    if (va < 0.0) != (vb < 0.0) {
                        quad(
                            cell_vert[cidx(i, j - 1, k - 1)],
                            cell_vert[cidx(i, j, k - 1)],
                            cell_vert[cidx(i, j, k)],
                            cell_vert[cidx(i, j - 1, k)],
                            va < 0.0,
                        );
                    }
                }
                // y-direction edge: cells (i-1..i, j, k-1..k)
                if i > 0 && k > 0 {
                    let (va, vb) = (s(i, j, k), s(i, j + 1, k));
                    if (va < 0.0) != (vb < 0.0) {
                        quad(
                            cell_vert[cidx(i - 1, j, k - 1)],
                            cell_vert[cidx(i - 1, j, k)],
                            cell_vert[cidx(i, j, k)],
                            cell_vert[cidx(i, j, k - 1)],
                            va < 0.0,
                        );
                    }
                }
                // z-direction edge: cells (i-1..i, j-1..j, k)
                if i > 0 && j > 0 {
                    let (va, vb) = (s(i, j, k), s(i, j, k + 1));
                    if (va < 0.0) != (vb < 0.0) {
                        quad(
                            cell_vert[cidx(i - 1, j - 1, k)],
                            cell_vert[cidx(i, j - 1, k)],
                            cell_vert[cidx(i, j, k)],
                            cell_vert[cidx(i - 1, j, k)],
                            va < 0.0,
                        );
                    }
                }
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_nets_sphere() {
        let f = |p: Vec3| sd_sphere(p, Vec3::ZERO, 1.0);
        let c = |_: Vec3| Vec3::ONE;
        let m = mesh_field(Vec3::splat(-1.4), Vec3::splat(1.4), 0.1, &f, &c);
        m.validate().unwrap();
        assert!(m.triangle_count() > 500, "{}", m.triangle_count());
        // every vertex near the unit sphere, normals point outward
        for (p, n) in m.positions.iter().zip(&m.normals) {
            assert!((p.length() - 1.0).abs() < 0.08, "vertex off surface: {p}");
            assert!(n.dot(p.normalize()) > 0.8, "normal not outward");
        }
        // deterministic
        let m2 = mesh_field(Vec3::splat(-1.4), Vec3::splat(1.4), 0.1, &f, &c);
        assert_eq!(m.positions, m2.positions);
    }

    #[test]
    fn smin_blends_and_round_cone_tapers() {
        // smin is a lower bound of min, equal far from the junction
        let a = 3.0;
        let b = 0.5;
        assert_eq!(smin(a, b, 0.2), b);
        assert!(smin(0.5, 0.5, 0.2) < 0.5);
        // round cone: near end A the radius is r1, near end B it's r2
        let a_pt = Vec3::ZERO;
        let b_pt = Vec3::new(0.0, 2.0, 0.0);
        let d1 = sd_round_cone(Vec3::new(0.5, 0.0, 0.0), a_pt, b_pt, 0.4, 0.1);
        let d2 = sd_round_cone(Vec3::new(0.5, 2.0, 0.0), a_pt, b_pt, 0.4, 0.1);
        assert!((d1 - 0.1).abs() < 0.02);
        assert!((d2 - 0.4).abs() < 0.02);
    }
}
