//! Loop-style subdivision: each triangle splits into 4 via shared edge
//! midpoints; optional Loop smoothing rounds the result. Attributes
//! (colors, uvs/tangents, skin weights, morph deltas) interpolate through.

use glam::Vec3;
use std::collections::HashMap;

use crate::mesh::Mesh;

fn avg4(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        (a[0] + b[0]) / 2.0,
        (a[1] + b[1]) / 2.0,
        (a[2] + b[2]) / 2.0,
        (a[3] + b[3]) / 2.0,
    ]
}

/// Blend two joint/weight sets: union the influences, halve the weights,
/// keep the 4 strongest, renormalize.
fn blend_skin(ja: [u16; 4], wa: [f32; 4], jb: [u16; 4], wb: [f32; 4]) -> ([u16; 4], [f32; 4]) {
    let mut acc: Vec<(u16, f32)> = Vec::with_capacity(8);
    for (j, w) in ja.iter().zip(wa).chain(jb.iter().zip(wb)) {
        if w <= 0.0 {
            continue;
        }
        match acc.iter_mut().find(|(jj, _)| jj == j) {
            Some((_, ww)) => *ww += w / 2.0,
            None => acc.push((*j, w / 2.0)),
        }
    }
    acc.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap().then(x.0.cmp(&y.0)));
    acc.truncate(4);
    let sum: f32 = acc.iter().map(|(_, w)| w).sum();
    let mut joints = [0u16; 4];
    let mut weights = [0f32; 4];
    for (i, (j, w)) in acc.iter().enumerate() {
        joints[i] = *j;
        weights[i] = w / sum.max(1e-9);
    }
    (joints, weights)
}

/// One round of 1→4 subdivision. `smooth` applies Loop vertex smoothing
/// (interior rule; boundary vertices keep their position).
pub fn subdivide(src: &Mesh, smooth: bool) -> Mesh {
    let mut m = Mesh {
        positions: src.positions.clone(),
        normals: src.normals.clone(),
        colors: src.colors.clone(),
        indices: Vec::with_capacity(src.indices.len() * 4),
        joints: src.joints.clone(),
        weights: src.weights.clone(),
        uvs: src.uvs.clone(),
        tangents: src.tangents.clone(),
        morphs: Vec::new(),
    };
    let mut morphs = src.morphs.clone();

    let mut midpoint_cache: HashMap<(u32, u32), u32> = HashMap::new();
    let mut edge_of: HashMap<(u32, u32), Vec<u32>> = HashMap::new(); // edge -> opposite verts
    for t in src.indices.chunks_exact(3) {
        for k in 0..3 {
            let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            let key = (a.min(b), a.max(b));
            edge_of.entry(key).or_default().push(c);
        }
    }

    let mut mid =
        |a: u32, b: u32, m: &mut Mesh, morphs: &mut Vec<crate::mesh::MorphTarget>| -> u32 {
            let key = (a.min(b), a.max(b));
            if let Some(&i) = midpoint_cache.get(&key) {
                return i;
            }
            let (ia, ib) = (a as usize, b as usize);
            // Loop odd-vertex rule when the edge has two opposite corners
            let opp = edge_of.get(&key).map(|v| v.as_slice()).unwrap_or(&[]);
            let pos = if smooth && opp.len() == 2 {
                (m.positions[ia] + m.positions[ib]) * 0.375
                    + (m.positions[opp[0] as usize] + m.positions[opp[1] as usize]) * 0.125
            } else {
                (m.positions[ia] + m.positions[ib]) / 2.0
            };
            m.positions.push(pos);
            m.normals
                .push(((m.normals[ia] + m.normals[ib]) / 2.0).normalize_or(Vec3::Y));
            m.colors.push((m.colors[ia] + m.colors[ib]) / 2.0);
            if !m.joints.is_empty() {
                let (j, w) = blend_skin(m.joints[ia], m.weights[ia], m.joints[ib], m.weights[ib]);
                m.joints.push(j);
                m.weights.push(w);
            }
            if !m.uvs.is_empty() {
                m.uvs.push((m.uvs[ia] + m.uvs[ib]) / 2.0);
                m.tangents
                    .push(avg4(m.tangents[ia].into(), m.tangents[ib].into()).into());
            }
            for mt in morphs.iter_mut() {
                let d = (mt.deltas[ia] + mt.deltas[ib]) / 2.0;
                mt.deltas.push(d);
            }
            let i = (m.positions.len() - 1) as u32;
            midpoint_cache.insert(key, i);
            i
        };

    for t in src.indices.chunks_exact(3) {
        let (a, b, c) = (t[0], t[1], t[2]);
        let ab = mid(a, b, &mut m, &mut morphs);
        let bc = mid(b, c, &mut m, &mut morphs);
        let ca = mid(c, a, &mut m, &mut morphs);
        m.indices
            .extend_from_slice(&[a, ab, ca, b, bc, ab, c, ca, bc, ab, bc, ca]);
    }

    if smooth {
        // Loop even-vertex rule over original vertices (interior only)
        let n_orig = src.positions.len();
        let mut neighbors: Vec<Vec<u32>> = vec![Vec::new(); n_orig];
        let mut boundary = vec![false; n_orig];
        for (&(a, b), opp) in &edge_of {
            neighbors[a as usize].push(b);
            neighbors[b as usize].push(a);
            if opp.len() != 2 {
                boundary[a as usize] = true;
                boundary[b as usize] = true;
            }
        }
        let mut new_pos = m.positions.clone();
        for (v, nbrs) in neighbors.iter().enumerate() {
            if boundary[v] || nbrs.len() < 3 {
                continue;
            }
            let k = nbrs.len() as f32;
            let beta =
                (5.0 / 8.0 - (0.375 + 0.25 * (core::f32::consts::TAU / k).cos()).powi(2)) / k;
            let sum: Vec3 = nbrs.iter().map(|&i| src.positions[i as usize]).sum();
            new_pos[v] = src.positions[v] * (1.0 - k * beta) + sum * beta;
        }
        m.positions = new_pos;
        m.recompute_smooth_normals();
    }

    m.morphs = morphs;
    m
}

/// N rounds of subdivision.
pub fn subdivide_n(src: &Mesh, n: u32, smooth: bool) -> Mesh {
    let mut m = src.clone();
    for _ in 0..n.min(4) {
        m = subdivide(&m, smooth);
    }
    m
}

/// Reduce triangle count to roughly `ratio` of the original via vertex
/// clustering on a uniform grid (deterministic; good enough for LODs).
pub fn decimate(src: &Mesh, ratio: f32) -> Mesh {
    let target = ((src.triangle_count() as f32) * ratio.clamp(0.02, 1.0)) as usize;
    let (lo, hi) = src.bounds();
    let diag = (hi - lo).length().max(1e-6);
    let mut cell = diag / 96.0;
    for _ in 0..24 {
        let m = cluster(src, lo, cell);
        if m.triangle_count() <= target.max(4) {
            return m;
        }
        cell *= 1.35;
    }
    cluster(src, lo, diag / 2.0)
}

fn cluster(src: &Mesh, lo: Vec3, cell: f32) -> Mesh {
    use std::collections::HashMap;
    let key = |p: Vec3| -> (i32, i32, i32) {
        (
            ((p.x - lo.x) / cell).floor() as i32,
            ((p.y - lo.y) / cell).floor() as i32,
            ((p.z - lo.z) / cell).floor() as i32,
        )
    };
    // assign cluster ids in vertex order → deterministic output ordering
    let mut ids: HashMap<(i32, i32, i32), u32> = HashMap::new();
    let mut remap = Vec::with_capacity(src.positions.len());
    let mut count: Vec<u32> = Vec::new();
    let mut m = Mesh::new();
    for (vi, &p) in src.positions.iter().enumerate() {
        let k = key(p);
        let id = *ids.entry(k).or_insert_with(|| {
            m.positions.push(Vec3::ZERO);
            m.normals.push(Vec3::ZERO);
            m.colors.push(Vec3::ZERO);
            if src.is_skinned() {
                m.joints.push(src.joints[vi]);
                m.weights.push(src.weights[vi]);
            }
            if src.has_uvs() {
                m.uvs.push(src.uvs[vi]);
                m.tangents.push(src.tangents[vi]);
            }
            count.push(0);
            (m.positions.len() - 1) as u32
        });
        m.positions[id as usize] += p;
        m.normals[id as usize] += src.normals[vi];
        m.colors[id as usize] += src.colors[vi];
        count[id as usize] += 1;
        remap.push(id);
    }
    for (i, c) in count.iter().enumerate() {
        let inv = 1.0 / *c as f32;
        m.positions[i] *= inv;
        m.colors[i] *= inv;
        m.normals[i] = m.normals[i].normalize_or(Vec3::Y);
    }
    for t in src.indices.chunks_exact(3) {
        let (a, b, c) = (
            remap[t[0] as usize],
            remap[t[1] as usize],
            remap[t[2] as usize],
        );
        if a != b && b != c && a != c {
            m.indices.extend_from_slice(&[a, b, c]);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{cuboid, icosphere};

    #[test]
    fn quadruples_triangles() {
        let m = icosphere(1.0, 1, Vec3::ONE);
        let s = subdivide(&m, false);
        s.validate().unwrap();
        assert_eq!(s.triangle_count(), m.triangle_count() * 4);
    }

    #[test]
    fn smoothing_shrinks_slightly_and_stays_valid() {
        let m = cuboid(Vec3::ZERO, Vec3::ONE, Vec3::ONE);
        let s = subdivide_n(&m, 2, true);
        s.validate().unwrap();
        let (lo, hi) = s.bounds();
        assert!(hi.x <= 1.0 + 1e-5 && lo.x >= -1.0 - 1e-5);
        assert!(hi.x - lo.x > 1.2, "should not collapse: {:?}", hi - lo);
    }

    #[test]
    fn decimate_reduces_and_stays_valid() {
        let m = icosphere(1.0, 4, Vec3::ONE); // 5120 tris
        let d = decimate(&m, 0.2);
        d.validate().unwrap();
        assert!(d.triangle_count() <= m.triangle_count() / 4);
        assert!(d.triangle_count() > 16);
        // deterministic
        let d2 = decimate(&m, 0.2);
        assert_eq!(d.positions, d2.positions);
        assert_eq!(d.indices, d2.indices);
    }

    #[test]
    fn carries_skin_weights() {
        let mut m = icosphere(1.0, 1, Vec3::ONE);
        m.bind_all_to_joint(3);
        let s = subdivide(&m, true);
        assert_eq!(s.joints.len(), s.positions.len());
        assert!(s.joints.iter().all(|j| j[0] == 3));
        assert!(
            s.weights
                .iter()
                .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 1e-4)
        );
    }
}
