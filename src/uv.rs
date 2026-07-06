//! UV projection + tangent generation for baked textures.
//!
//! `scale` everywhere means "world units per texture tile". Projections fill
//! `Mesh::uvs` and `Mesh::tangents`; they work best on flat-shaded meshes
//! (unique vertices per face, so no seams cut through a face).

use glam::{Vec2, Vec3, Vec4};

use crate::mesh::Mesh;

fn tangent_for(n: Vec3, t: Vec3) -> Vec4 {
    let t = (t - n * t.dot(n)).normalize_or(n.any_orthonormal_vector());
    Vec4::new(t.x, t.y, t.z, 1.0)
}

/// Box (triplanar-style) projection: each vertex projects onto the axis
/// plane most aligned with its normal.
pub fn box_project(m: &mut Mesh, scale: f32) {
    let s = 1.0 / scale.max(1e-4);
    m.uvs = Vec::with_capacity(m.positions.len());
    m.tangents = Vec::with_capacity(m.positions.len());
    for (p, n) in m.positions.iter().zip(&m.normals) {
        let an = n.abs();
        let (uv, tan) = if an.x >= an.y && an.x >= an.z {
            (
                Vec2::new(p.z * n.x.signum(), -p.y),
                Vec3::new(0.0, 0.0, n.x.signum()),
            )
        } else if an.y >= an.z {
            (Vec2::new(p.x, p.z * n.y.signum()), Vec3::X)
        } else {
            (
                Vec2::new(-p.x * n.z.signum(), -p.y),
                Vec3::new(-n.z.signum(), 0.0, 0.0),
            )
        };
        m.uvs.push(uv * s);
        m.tangents.push(tangent_for(*n, tan));
    }
}

/// Cylindrical projection around Y: u = angle, v = height.
pub fn cylindrical_project(m: &mut Mesh, scale: f32) {
    let s = 1.0 / scale.max(1e-4);
    let tau = core::f32::consts::TAU;
    m.uvs = m
        .positions
        .iter()
        .map(|p| {
            let a = p.z.atan2(p.x) / tau + 0.5;
            let r = (p.x * p.x + p.z * p.z).sqrt().max(1e-4);
            Vec2::new(a * (tau * r * s).max(1.0).round(), p.y * s)
        })
        .collect();
    m.tangents = m
        .normals
        .iter()
        .zip(&m.positions)
        .map(|(n, p)| {
            let t = Vec3::new(-p.z, 0.0, p.x).normalize_or(Vec3::X);
            tangent_for(*n, t)
        })
        .collect();
    fix_wrap_seams(m);
}

/// Planar top-down projection (terrains, floors).
pub fn planar_project(m: &mut Mesh, scale: f32) {
    let s = 1.0 / scale.max(1e-4);
    m.uvs = m
        .positions
        .iter()
        .map(|p| Vec2::new(p.x, p.z) * s)
        .collect();
    m.tangents = m.normals.iter().map(|n| tangent_for(*n, Vec3::X)).collect();
}

/// On meshes with per-face vertices, unwrap triangles that straddle the
/// cylindrical wrap seam so no face spans the whole texture.
fn fix_wrap_seams(m: &mut Mesh) {
    // only safe when no vertex is shared between faces
    if m.positions.len() != m.indices.len() {
        return;
    }
    for t in m.indices.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let us = [m.uvs[a].x, m.uvs[b].x, m.uvs[c].x];
        let max = us.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        for &i in &[a, b, c] {
            if max - m.uvs[i].x > 0.5 {
                m.uvs[i].x += (max - m.uvs[i].x + 0.5).floor();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::cuboid;

    #[test]
    fn box_projection_fills_attrs() {
        let mut m = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        box_project(&mut m, 2.0);
        assert_eq!(m.uvs.len(), m.positions.len());
        assert_eq!(m.tangents.len(), m.positions.len());
        m.validate().unwrap();
        // a 2-unit cube with 2-unit tiles spans exactly one tile
        for uv in &m.uvs {
            assert!(uv.x.abs() <= 1.01 && uv.y.abs() <= 1.01, "{uv:?}");
        }
        // tangents orthogonal to normals
        for (t, n) in m.tangents.iter().zip(&m.normals) {
            assert!(Vec3::new(t.x, t.y, t.z).dot(*n).abs() < 1e-4);
        }
    }

    #[test]
    fn merge_pads_uvs() {
        let mut a = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        box_project(&mut a, 1.0);
        let b = cuboid(Vec3::ONE, Vec3::ONE, Vec3::ONE);
        a.merge(&b);
        a.validate().unwrap();
        assert_eq!(a.uvs.len(), a.positions.len());
    }
}
